//! Yog inter-mod communication — proc-macros for cross-mod function calls.
//!
//! # Export (manual registration, auto-generated binding symbol)
//!
//! ```ignore
//! use yog_api::{yog_export, Registry};
//!
//! #[yog_export]
//! pub fn register_pipe(api: *const yog_api::YogApi, json: *const std::ffi::c_char) {
//!     // ...
//! }
//!
//! fn register(registry: &mut Registry) {
//!     // Manual export registration (loader scanning coming later)
//!     registry.interop().export("register_pipe", register_pipe as *const std::ffi::c_void);
//! }
//! ```
//!
//! # Import (auto-wired by loader)
//!
//! ```ignore
//! yog_api::import! {
//!     from "yog-pipes" {
//!         fn register_pipe(api: *const yog_api::YogApi, json: *const std::ffi::c_char);
//!     }
//! }
//!
//! // Use like a normal function — loader wires it at init time
//! register_pipe(api, json_ptr);
//! ```

use proc_macro::TokenStream;
use quote::{format_ident, quote};

// ── #[yog_export] ─────────────────────────────────────────────────────────

/// Universal export macro — works on both structs and functions.
///
/// **On structs**: auto-derives `rkyv::Archive`, `rkyv::Serialize`,
/// `rkyv::Deserialize`. Equivalent to:
/// ```ignore
/// #[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
/// #[archive(check_bytes)]
/// struct MyStruct { ... }
/// ```
///
/// **On functions**: generates a C-ABI wrapper using rkyv zero-copy
/// serialization + auto-registers the export via `export_mod!`.
///
/// Supported fn signatures:
/// - `fn(InputType) -> OutputType`  (single arg)
/// - `fn() -> OutputType`          (no arg)
/// - `fn(InputType)`               (no return)
/// For multi-arg: `fn((A, B)) -> C`.
///
/// No manual `registry.interop().export()` needed.
#[proc_macro_attribute]
pub fn yog_export(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Try parsing as a struct or enum first (more common for types)
    if let Ok(s) = syn::parse2::<syn::ItemStruct>(proc_macro2::TokenStream::from(item.clone())) {
        let vis = &s.vis;
        let ident = &s.ident;
        let generics = &s.generics;
        let fields = &s.fields;
        let attrs = &s.attrs;
        return TokenStream::from(quote! {
            #[derive(::yog_api::rkyv::Archive, ::yog_api::rkyv::Serialize, ::yog_api::rkyv::Deserialize)]
            #(#attrs)*
            #vis struct #ident #generics #fields
        });
    }
    if let Ok(e) = syn::parse2::<syn::ItemEnum>(proc_macro2::TokenStream::from(item.clone())) {
        let vis = &e.vis;
        let ident = &e.ident;
        let generics = &e.generics;
        let variants = &e.variants;
        let attrs = &e.attrs;
        return TokenStream::from(quote! {
            #[derive(::yog_api::rkyv::Archive, ::yog_api::rkyv::Serialize, ::yog_api::rkyv::Deserialize)]
            #(#attrs)*
            #vis enum #ident #generics { #variants }
        });
    }

    // Fall through to function handling
    let func = syn::parse_macro_input!(item as syn::ItemFn);
    let name = &func.sig.ident;
    let vis = &func.vis;
    let sig = &func.sig;
    let block = &func.block;
    let name_str = name.to_string();
    let wrap_name = format_ident!("__yog_wrap_{}", name);
    let _init_name = format_ident!("__yog_export_init_{}", name);

    // Extract input and output types
    let inputs = &func.sig.inputs;
    let output = match &func.sig.output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };

    // Build the wrapper body — handles both arg and no-arg cases
    let call_fn = if inputs.is_empty() {
        quote! { #name() }
    } else {
        // Multi-arg: serialize as tuple, then deserialize and destructure
        let arg_names: Vec<_> = inputs.iter().map(|arg| {
            match arg {
                syn::FnArg::Typed(pat_type) => &pat_type.pat,
                syn::FnArg::Receiver(_) => unreachable!(),
            }
        }).collect();
        quote! { #name(#(#arg_names),*) }
    };

    // Extract the single input type (or unit for no-arg fns)
    let input_type = if inputs.is_empty() {
        quote! { () }
    } else {
        // Take the first (and should be only) argument type
        match inputs.first().unwrap() {
            syn::FnArg::Typed(pat_type) => {
                let ty = &pat_type.ty;
                quote! { #ty }
            }
            _ => quote! { () },
        }
    };

    let expanded = quote! {
        #vis #sig #block

        // ctor — runs before main(), pushes entry to export registry
        #[doc(hidden)]
        #[ctor::ctor]
        fn __yog_export_ctor_{name}() {
            ::yog_api::__yog_export_registry()
                .lock()
                .unwrap()
                .push(::yog_api::YogExportEntry {
                    name: #name_str,
                    ptr: #wrap_name as usize,
                });
        }

        // C-ABI wrapper — deserializes input via rkyv, calls fn, serializes output.
        #[doc(hidden)]
        #[no_mangle]
        pub unsafe extern "C" fn #wrap_name(
            input_ptr: *const u8,
            input_len: u32,
            out_data: *mut *mut u8,
            out_len: *mut u32,
            out_cap: *mut u32,
        ) {
            let input_slice = std::slice::from_raw_parts(input_ptr, input_len as usize);
            let args: #input_type = ::yog_api::rkyv::from_bytes::<_, ::yog_api::rkyv::rancor::Error>(input_slice)
                .expect("yog: deserialization failed in export wrapper");
            let result: #output = #call_fn;
            let aligned = ::yog_api::rkyv::to_bytes::<_, 256>(&result)
                .unwrap_or_default();
            let bytes: Vec<u8> = aligned.to_vec();
            *out_data = bytes.as_ptr() as *mut u8;
            *out_len = bytes.len() as u32;
            *out_cap = bytes.capacity() as u32;
            std::mem::forget(bytes); // caller frees via __yog_free_buffer
        }
    };

    TokenStream::from(expanded)
}

// ── import! macro ────────────────────────────────────────────────────────

/// Declare imported functions from another mod. Generates public wrapper
/// functions and `__yog_bind_NAME` entry points for the loader.
///
/// ```ignore
/// yog_api::import! {
///     from "yog-pipes" {
///         fn register_pipe(api: *const YogApi, json: *const c_char) -> bool;
///     }
/// }
/// register_pipe(api, json_ptr); // auto-wired
/// ```
#[proc_macro]
pub fn import(input: TokenStream) -> TokenStream {
    let import_block = syn::parse_macro_input!(input as ImportBlock);
    let mod_name = &import_block.mod_name;
    let mut out = proc_macro2::TokenStream::new();

    for func in &import_block.funcs {
        let name = &func.sig.ident;
        let name_str = name.to_string();
        let slot_name = format_ident!("__yog_slot_{}", name);
        let bind_name = format_ident!("__yog_bind_{}", name);
        let fn_sig = &func.sig;
        let vis = &func.vis;
        let inputs = &fn_sig.inputs;
        let output = &fn_sig.output;

        // Extract input type (single arg or unit)
        let _input_type = if inputs.is_empty() {
            quote! { () }
        } else {
            match inputs.first().unwrap() {
                syn::FnArg::Typed(pat_type) => {
                    let ty = &pat_type.ty;
                    quote! { #ty }
                }
                _ => quote! { () },
            }
        };

        let output_type = match output {
            syn::ReturnType::Default => quote! { () },
            syn::ReturnType::Type(_, ty) => quote! { #ty },
        };

        // Build arg forwarding for the public function
        let arg_names: Vec<_> = inputs.iter().map(|arg| {
            match arg {
                syn::FnArg::Typed(pat_type) => &pat_type.pat,
                syn::FnArg::Receiver(_) => unreachable!("self not allowed in imports"),
            }
        }).collect();

        // Serialize the arg (or nothing for no-arg fns)
        let serialize_block = if inputs.is_empty() {
            quote! { let input_bytes = Vec::new(); }
        } else {
            quote! {
                let aligned = ::yog_api::rkyv::to_bytes::<_, 256>(&(#(#arg_names),*))
                    .unwrap_or_default();
                let input_bytes: Vec<u8> = aligned.to_vec();
            }
        };

        // C-ABI wrapper type — shared by all exports
        let wrapper_ty = quote! {
            unsafe extern "C" fn(
                input_ptr: *const u8, input_len: u32,
                out_data: *mut *mut u8, out_len: *mut u32, out_cap: *mut u32,
            )
        };

        let _init_name = format_ident!("__yog_import_init_{}", name);

        let wrapper = quote! {
            // ctor — runs before main(), pushes entry to import registry
            #[doc(hidden)]
            #[ctor::ctor]
            fn __yog_import_ctor_{name}() {
                ::yog_api::__yog_import_registry()
                    .lock()
                    .unwrap()
                    .push(::yog_api::YogImportEntry {
                        mod_id: #mod_name,
                        symbol: #name_str,
                        bind_fn: #bind_name as usize,
                    });
            }

            #[allow(non_snake_case)]
            #vis fn #name(#inputs) #output {
                #serialize_block
                let f = #slot_name.get()
                    .expect(concat!("yog: import '", #mod_name, ":", #name_str, "' not bound — is mod '", #mod_name, "' loaded?"));
                let mut out_data: *mut u8 = std::ptr::null_mut();
                let mut out_len: u32 = 0;
                let mut out_cap: u32 = 0;
                unsafe {
                    f(
                        input_bytes.as_ptr(), input_bytes.len() as u32,
                        &mut out_data, &mut out_len, &mut out_cap,
                    );
                    let output_slice = std::slice::from_raw_parts(out_data, out_len as usize);
                    let result: #output_type = ::yog_api::rkyv::from_bytes::<_, ::yog_api::rkyv::rancor::Error>(output_slice)
                        .expect(concat!("yog: deserialization failed in import '", #mod_name, ":", #name_str, "'"));
                    // Free the buffer allocated by the export wrapper
                    let _ = Vec::from_raw_parts(out_data, out_len as usize, out_cap as usize);
                    result
                }
            }

            static #slot_name: ::std::sync::OnceLock<#wrapper_ty>
                = ::std::sync::OnceLock::new();

            #[doc(hidden)]
            #[no_mangle]
            pub unsafe extern "C" fn #bind_name(ptr: *const ::std::os::raw::c_void) {
                let f: #wrapper_ty = ::std::mem::transmute(ptr);
                #slot_name.set(f).ok();
            }
        };

        out.extend(wrapper);
    }

    TokenStream::from(out)
}

// ── Parse helpers ────────────────────────────────────────────────────────

struct ImportBlock {
    mod_name: String,
    funcs: Vec<syn::ItemFn>,
}

impl syn::parse::Parse for ImportBlock {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let from_kw: syn::Ident = input.parse()?;
        if from_kw != "from" {
            return Err(syn::Error::new(from_kw.span(), "expected `from`"));
        }
        let lit: syn::LitStr = input.parse()?;
        let mod_name = lit.value();

        let content;
        syn::braced!(content in input);
        let mut funcs = Vec::new();
        while !content.is_empty() {
            funcs.push(content.parse::<syn::ItemFn>()?);
            let _ = content.parse::<syn::Token![;]>();
        }

        Ok(ImportBlock { mod_name, funcs })
    }
}

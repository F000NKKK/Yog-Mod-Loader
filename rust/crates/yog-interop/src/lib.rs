//! Proc-macros for Yog inter-mod communication.
//!
//! `#[yog_export]` on fn: generates C-ABI wrapper. Register manually:
//! ```ignore
//! registry.interop().export("fname", __yog_wrap_fname as *const c_void);
//! ```
//!
//! `import!` macro: generates Rust wrapper + binding slot.

use proc_macro::TokenStream;
use quote::{format_ident, quote};

// ── #[yog_export] ─────────────────────────────────────────────────────────

/// Marks a function for inter-mod export. Generates a C-ABI wrapper.
/// Auto-registration TBD (ctor/linkme debugging in progress).
/// For now, register manually in `Mod::register()`:
/// ```ignore
/// registry.interop().export("fname", __yog_wrap_fname as *const c_void);
/// ```
#[proc_macro_attribute]
pub fn yog_export(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Try parsing as a struct or enum first (auto rkyv derives)
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

    let func = syn::parse_macro_input!(item as syn::ItemFn);
    let name = &func.sig.ident;
    let vis = &func.vis;
    let sig = &func.sig;
    let block = &func.block;
    let name_str = name.to_string();
    let wrap_name = format_ident!("__yog_wrap_{}", name);

    let inputs = &func.sig.inputs;
    let output = match &func.sig.output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };

    let call_fn = if inputs.is_empty() {
        quote! { #name() }
    } else {
        let arg_names: Vec<_> = inputs.iter().map(|arg| {
            match arg {
                syn::FnArg::Typed(pat_type) => &pat_type.pat,
                syn::FnArg::Receiver(_) => unreachable!(),
            }
        }).collect();
        quote! { #name(#(#arg_names),*) }
    };

    let input_type = if inputs.is_empty() {
        quote! { () }
    } else {
        match inputs.first().unwrap() {
            syn::FnArg::Typed(pat_type) => { let ty = &pat_type.ty; quote! { #ty } }
            _ => quote! { () },
        }
    };

    let expanded = quote! {
        #vis #sig #block

        // Auto-register via linker section (no ctor dependency needed)
        #[doc(hidden)]
        #[used]
        #[cfg_attr(target_os = "linux", link_section = ".init_array")]
        #[cfg_attr(target_vendor = "apple", link_section = "__DATA,__mod_init_func,mod_init_funcs")]
        #[cfg_attr(all(target_os = "windows", any(target_env = "gnu", target_env = "msvc")), link_section = ".CRT$XCU")]
        static __yog_export_ctor_{name}: extern "C" fn() = {{
            extern "C" fn init() {{
                ::yog_api::__yog_export_registry()
                    .lock()
                    .unwrap()
                    .push(::yog_api::YogExportEntry {{
                        name: #name_str,
                        ptr: #wrap_name as usize,
                    }});
            }}
            init
        }};
        
        #[doc(hidden)]
        #[no_mangle]
        pub unsafe extern "C" fn #wrap_name(
            input_ptr: *const u8, input_len: u32,
            out_data: *mut *mut u8, out_len: *mut u32, out_cap: *mut u32,
        ) {
            let input_slice = std::slice::from_raw_parts(input_ptr, input_len as usize);
            let args: #input_type = ::yog_api::rkyv::from_bytes::<_, ::yog_api::rkyv::rancor::Error>(input_slice)
                .expect("yog: deser failed");
            let result: #output = #call_fn;
            let aligned = ::yog_api::rkyv::to_bytes::<::yog_api::rkyv::rancor::Error>(&result).unwrap_or_default();
            let bytes: Vec<u8> = aligned.to_vec();
            *out_data = bytes.as_ptr() as *mut u8;
            *out_len = bytes.len() as u32;
            *out_cap = bytes.capacity() as u32;
            std::mem::forget(bytes);
        }
    };

    TokenStream::from(expanded)
}

// ── import! macro ────────────────────────────────────────────────────────

#[proc_macro]
pub fn import(input: TokenStream) -> TokenStream {
    let import_block = syn::parse_macro_input!(input as ImportBlock);
    let mod_name = &import_block.mod_name;
    let mod_ident = format_ident!("{}", mod_name.replace('-', "_"));
    let mut items = proc_macro2::TokenStream::new();

    for item in &import_block.items {
        match item {
            ImportItem::Func(func) => { items.extend(generate_import_fn(func, mod_name)); }
            ImportItem::Struct(s) => {
                let vis = &s.vis; let ident = &s.ident; let generics = &s.generics;
                let fields = &s.fields; let attrs = &s.attrs;
                items.extend(quote! {
                    #[derive(::yog_api::rkyv::Archive, ::yog_api::rkyv::Serialize, ::yog_api::rkyv::Deserialize)]
                    #(#attrs)* #vis struct #ident #generics #fields
                });
            }
        }
    }

    let out = quote! { pub mod #mod_ident { #items } };
    TokenStream::from(out)
}

fn generate_import_fn(func: &syn::ItemFn, mod_name: &str) -> proc_macro2::TokenStream {
    let name = &func.sig.ident;
    let name_str = name.to_string();
    let slot_name = format_ident!("__yog_slot_{}", name);
    let bind_name = format_ident!("__yog_bind_{}", name);
    let fn_sig = &func.sig;
    let vis = &func.vis;
    let inputs = &fn_sig.inputs;
    let output = &fn_sig.output;

    let output_type = match output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };

    let arg_names: Vec<_> = inputs.iter().map(|arg| {
        match arg { syn::FnArg::Typed(pat_type) => &pat_type.pat, _ => unreachable!() }
    }).collect();

    let serialize_block = if inputs.is_empty() {
        quote! { let input_bytes = Vec::new(); }
    } else {
        quote! {
            let aligned = ::yog_api::rkyv::to_bytes::<::yog_api::rkyv::rancor::Error>(&(#(#arg_names),*)).unwrap_or_default();
            let input_bytes: Vec<u8> = aligned.to_vec();
        }
    };

    let wrapper_ty = quote! {
        unsafe extern "C" fn(input_ptr: *const u8, input_len: u32,
            out_data: *mut *mut u8, out_len: *mut u32, out_cap: *mut u32)
    };

    quote! {
        // Auto-register via linker section — equivalent to #[ctor::ctor(unsafe)]
        #[doc(hidden)]
        #[used]
        #[cfg_attr(target_os = "linux", link_section = ".init_array")]
        #[cfg_attr(target_vendor = "apple", link_section = "__DATA,__mod_init_func,mod_init_funcs")]
        #[cfg_attr(all(target_os = "windows", any(target_env = "gnu", target_env = "msvc")), link_section = ".CRT$XCU")]
        static __yog_import_ctor_{name}: extern "C" fn() = {{
            extern "C" fn init() {{
                ::yog_api::__yog_import_registry()
                    .lock()
                    .unwrap()
                    .push(::yog_api::YogImportEntry {{
                        mod_id: #mod_name,
                        symbol: #name_str,
                        bind_fn: #bind_name as usize,
                    }});
            }}
            init
        }};

        #[allow(non_snake_case)]
        #vis fn #name(#inputs) #output {
            #serialize_block
            let f = #slot_name.get()
                .expect(concat!("yog: import '", #mod_name, ":", #name_str, "' not bound"));
            let mut out_data: *mut u8 = std::ptr::null_mut();
            let mut out_len: u32 = 0; let mut out_cap: u32 = 0;
            unsafe {
                f(input_bytes.as_ptr(), input_bytes.len() as u32,
                  &mut out_data, &mut out_len, &mut out_cap);
                let output_slice = std::slice::from_raw_parts(out_data, out_len as usize);
                let result: #output_type = ::yog_api::rkyv::from_bytes::<_, ::yog_api::rkyv::rancor::Error>(output_slice)
                    .expect(concat!("yog: deser failed in import '", #mod_name, ":", #name_str, "'"));
                let _ = Vec::from_raw_parts(out_data, out_len as usize, out_cap as usize);
                result
            }
        }

        static #slot_name: ::std::sync::OnceLock<#wrapper_ty> = ::std::sync::OnceLock::new();

        #[doc(hidden)] #[no_mangle]
        pub unsafe extern "C" fn #bind_name(ptr: *const ::std::os::raw::c_void) {
            let f: #wrapper_ty = ::std::mem::transmute(ptr);
            #slot_name.set(f).ok();
        }
    }
}

// ── Parse helpers ────────────────────────────────────────────────────────

struct ImportBlock { mod_name: String, items: Vec<ImportItem> }
enum ImportItem { Func(syn::ItemFn), Struct(syn::ItemStruct) }

impl syn::parse::Parse for ImportBlock {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let from_kw: syn::Ident = input.parse()?;
        if from_kw != "from" { return Err(syn::Error::new(from_kw.span(), "expected `from`")); }
        let lit: syn::LitStr = input.parse()?;
        let mod_name = lit.value();
        let content; syn::braced!(content in input);
        let mut items = Vec::new();
        while !content.is_empty() {
            if let Ok(f) = content.parse::<syn::ItemFn>() { items.push(ImportItem::Func(f)); let _ = content.parse::<syn::Token![;]>(); }
            else if let Ok(s) = content.parse::<syn::ItemStruct>() { items.push(ImportItem::Struct(s)); let _ = content.parse::<syn::Token![;]>(); }
            else { break; }
        }
        Ok(ImportBlock { mod_name, items })
    }
}

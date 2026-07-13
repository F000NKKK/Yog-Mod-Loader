//! Proc-macros for Yog inter-mod communication.
//!
//! `#[yog_export]` on fn: generates a C-ABI wrapper and a `ctor` constructor
//! that registers it into `yog_api`'s auto-export registry when the mod's
//! `cdylib` is loaded — `export_mod!` flushes that registry into
//! `registry.interop().export(...)` automatically, no manual call needed.
//!
//! `import!` macro: generates Rust wrapper + binding slot.

use proc_macro::TokenStream;
use quote::{format_ident, quote};

// ── #[yog_export] ─────────────────────────────────────────────────────────

/// Marks a function for inter-mod export. Generates a C-ABI wrapper plus a
/// `ctor` constructor that self-registers into `yog_api`'s auto-export
/// registry — `export_mod!` picks it up automatically, no manual
/// `registry.interop().export(...)` call needed.
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
    let ctor_name = format_ident!("__yog_ctor_register_{}", name);

    let output = match &func.sig.output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };

    // A leading `&mut Registry`-shaped parameter is never serialized across
    // the interop boundary — every mod is handed the same global `YogApi`
    // table, so the exporting mod reconstructs its own `Registry` from the
    // API pointer it captured during its own `yog_mod_register`, instead of
    // requiring callers to thread an `api_ptr` field through by hand.
    fn is_registry_ty(ty: &syn::Type) -> bool {
        let ty = match ty {
            syn::Type::Reference(r) => &*r.elem,
            other => other,
        };
        quote! { #ty }.to_string().replace(' ', "").ends_with("Registry")
    }

    let mut data_params: Vec<&syn::PatType> = func
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pt) => Some(pt),
            syn::FnArg::Receiver(_) => None,
        })
        .collect();

    let has_registry_param = data_params
        .first()
        .map(|pt| is_registry_ty(&pt.ty))
        .unwrap_or(false);
    if has_registry_param {
        data_params.remove(0);
    }

    let data_types: Vec<&syn::Type> = data_params.iter().map(|pt| &*pt.ty).collect();
    let arg_idents: Vec<syn::Ident> = (0..data_types.len())
        .map(|i| format_ident!("__yog_arg{}", i))
        .collect();

    let data_type = match data_types.len() {
        0 => quote! { () },
        1 => {
            let t = data_types[0];
            quote! { #t }
        }
        _ => quote! { (#(#data_types),*) },
    };

    let destructure = match arg_idents.len() {
        0 => quote! {},
        1 => {
            let a = &arg_idents[0];
            quote! { let #a = __yog_data; }
        }
        _ => quote! { let (#(#arg_idents),*) = __yog_data; },
    };

    let registry_setup = if has_registry_param {
        quote! {
            let mut __yog_registry = unsafe {
                ::yog_api::Registry::from_raw(::yog_api::__current_api_ptr() as *const ::yog_api::YogApi)
            };
        }
    } else {
        quote! {}
    };

    let mut call_args: Vec<proc_macro2::TokenStream> = Vec::new();
    if has_registry_param {
        call_args.push(quote! { &mut __yog_registry });
    }
    call_args.extend(arg_idents.iter().map(|a| quote! { #a }));
    let call_fn = quote! { #name(#(#call_args),*) };

    let expanded = quote! {
        #vis #sig #block

        #[doc(hidden)]
        #[no_mangle]
        pub unsafe extern "C" fn #wrap_name(
            input_ptr: *const u8, input_len: u32,
            out_data: *mut *mut u8, out_len: *mut u32, out_cap: *mut u32,
        ) {
            let input_slice = std::slice::from_raw_parts(input_ptr, input_len as usize);
            let __yog_data: #data_type = ::yog_api::rkyv::from_bytes::<_, ::yog_api::rkyv::rancor::Error>(input_slice)
                .expect("yog: deser failed");
            #destructure
            #registry_setup
            let result: #output = #call_fn;
            let aligned = ::yog_api::rkyv::to_bytes::<::yog_api::rkyv::rancor::Error>(&result).unwrap_or_default();
            let bytes: Vec<u8> = aligned.to_vec();
            *out_data = bytes.as_ptr() as *mut u8;
            *out_len = bytes.len() as u32;
            *out_cap = bytes.capacity() as u32;
            std::mem::forget(bytes);
        }

        // Runs when this mod's `cdylib` is loaded, before the runtime calls
        // `yog_mod_register` — populates the auto-export registry so mod
        // authors don't need to call `registry.interop().export(...)` by hand.
        #[::yog_api::ctor::ctor(unsafe, crate_path = ::yog_api::ctor)]
        #[doc(hidden)]
        fn #ctor_name() {
            ::yog_api::__yog_export_registry().lock().unwrap().push(
                ::yog_api::YogExportEntry { name: #name_str, ptr: #wrap_name as usize }
            );
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
            ImportItem::Func(func) => {
                items.extend(generate_import_fn(func, mod_name));
            }
            ImportItem::Struct(s) => {
                let vis = &s.vis;
                let ident = &s.ident;
                let generics = &s.generics;
                let fields = &s.fields;
                let attrs = &s.attrs;
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
    // Statics use SCREAMING_SNAKE_CASE per Rust convention; the bind fn stays
    // snake_case since it's a function, not a static.
    let slot_name = format_ident!("__YOG_SLOT_{}", name_str.to_uppercase());
    let bind_name = format_ident!("__yog_bind_{}", name);
    let fn_sig = &func.sig;
    let vis = &func.vis;
    let inputs = &fn_sig.inputs;
    let output = &fn_sig.output;

    let output_type = match output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };

    let arg_names: Vec<_> = inputs
        .iter()
        .map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => &pat_type.pat,
            _ => unreachable!(),
        })
        .collect();

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

struct ImportBlock {
    mod_name: String,
    items: Vec<ImportItem>,
}
enum ImportItem {
    Func(syn::ItemFn),
    Struct(syn::ItemStruct),
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
        let mut items = Vec::new();
        while !content.is_empty() {
            if let Ok(f) = content.parse::<syn::ItemFn>() {
                items.push(ImportItem::Func(f));
                let _ = content.parse::<syn::Token![;]>();
            } else if let Ok(s) = content.parse::<syn::ItemStruct>() {
                items.push(ImportItem::Struct(s));
                let _ = content.parse::<syn::Token![;]>();
            } else {
                break;
            }
        }
        Ok(ImportBlock { mod_name, items })
    }
}

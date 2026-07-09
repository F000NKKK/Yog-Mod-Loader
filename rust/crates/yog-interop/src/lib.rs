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

/// Marks a function for inter-mod export.
///
/// Generates a `__yog_export_get_NAME` symbol that a future loader scanner
/// can use to auto-discover exports. For now, the mod must also call
/// `registry.interop().export("name", fn_ptr)` during `register()`.
#[proc_macro_attribute]
pub fn yog_export(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = syn::parse_macro_input!(item as syn::ItemFn);
    let name = &func.sig.ident;
    let vis = &func.vis;
    let sig = &func.sig;
    let block = &func.block;
    let name_str = name.to_string();
    let getter_name = format_ident!("__yog_export_get_{}", name);

    let expanded = quote! {
        #vis #sig #block

        #[doc(hidden)]
        #[no_mangle]
        pub unsafe extern "C" fn #getter_name(
            out_name: *mut *const ::std::os::raw::c_char,
            out_ptr: *mut *const ::std::os::raw::c_void,
        ) {
            if !out_name.is_null() {
                *out_name = concat!(#name_str, "\0").as_ptr() as *const ::std::os::raw::c_char;
            }
            if !out_ptr.is_null() {
                *out_ptr = #name as *const ::std::os::raw::c_void;
            }
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

        let arg_names: Vec<_> = inputs.iter().map(|arg| {
            match arg {
                syn::FnArg::Typed(pat_type) => &pat_type.pat,
                syn::FnArg::Receiver(_) => unreachable!("self not allowed in imports"),
            }
        }).collect();

        let wrapper = quote! {
            #[allow(non_snake_case)]
            #vis fn #name(#inputs) #output {
                let f = #slot_name.get()
                    .expect(concat!("yog: import '", #mod_name, ":", #name_str, "' not bound — is mod '", #mod_name, "' loaded?"));
                unsafe { f(#(#arg_names),*) }
            }

            static #slot_name: ::std::sync::OnceLock<unsafe extern "C" fn(#inputs) #output>
                = ::std::sync::OnceLock::new();

            #[doc(hidden)]
            #[no_mangle]
            pub unsafe extern "C" fn #bind_name(ptr: *const ::std::os::raw::c_void) {
                let f: unsafe extern "C" fn(#inputs) #output = ::std::mem::transmute(ptr);
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

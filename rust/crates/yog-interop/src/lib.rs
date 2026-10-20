//! Proc-macros for mods' exports (`docs/design/bus.md`, stage 4).
//!
//! `#[yog_export]` on a fn makes it a bus service at `mod_id:name` (its
//! `ctor` registers it when the mod's library loads; `export_mod!`
//! provides it); on a struct or enum it derives the Yog Data schema the
//! calls carry it with. `import!` writes the calling side.

use proc_macro::TokenStream;
use quote::{format_ident, quote};

// ── #[yog_export] ─────────────────────────────────────────────────────────

/// The derives an exported type carries: its Yog Data schema, through
/// `yog_api`'s re-export (a mod depends on `yog_api` only).
fn schema_derive() -> proc_macro2::TokenStream {
    quote! {
        #[derive(::yog_api::yog_data::Schema)]
        #[data(crate = "::yog_api::yog_data")]
    }
}

/// Whether `ty` is a `Registry` (a leading `&mut Registry` is the
/// exporting mod's own, never part of a call).
fn is_registry_ty(ty: &syn::Type) -> bool {
    let ty = match ty {
        syn::Type::Reference(r) => &*r.elem,
        other => other,
    };
    quote! { #ty }
        .to_string()
        .replace(' ', "")
        .ends_with("Registry")
}

/// A function's call parameters (a leading `Registry` left out), each a
/// plain name — `__yog_argN` for a pattern that isn't one.
fn call_params(sig: &syn::Signature) -> (bool, Vec<(syn::Ident, syn::Type)>) {
    let mut typed: Vec<&syn::PatType> = sig
        .inputs
        .iter()
        .filter_map(|a| match a {
            syn::FnArg::Typed(pt) => Some(pt),
            syn::FnArg::Receiver(_) => None,
        })
        .collect();
    let registry = typed.first().is_some_and(|pt| is_registry_ty(&pt.ty));
    if registry {
        typed.remove(0);
    }
    let params = typed
        .iter()
        .enumerate()
        .map(|(i, pt)| {
            let name = match &*pt.pat {
                syn::Pat::Ident(p) => p.ident.clone(),
                _ => format_ident!("__yog_arg{}", i),
            };
            (name, (*pt.ty).clone())
        })
        .collect();
    (registry, params)
}

/// The struct a call's arguments cross as: one field per parameter, named like it.
fn args_struct(
    name: &syn::Ident,
    params: &[(syn::Ident, syn::Type)],
) -> (syn::Ident, proc_macro2::TokenStream) {
    let args = format_ident!("__YogArgs_{}", name);
    let fields = params.iter().map(|(n, t)| quote! { #n: #t });
    let derive = schema_derive();
    (
        args.clone(),
        quote! {
            #[doc(hidden)]
            // Named after the function (`__YogArgs_give_item`), which is snake case.
            #[allow(non_camel_case_types)]
            #derive
            struct #args { #(#fields),* }
        },
    )
}

fn output_type(sig: &syn::Signature) -> proc_macro2::TokenStream {
    match &sig.output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    }
}

/// Marks a function, struct or enum for other mods: a function becomes the
/// service `mod_id:name`, a type gets the schema its values cross with.
/// A function's doc comment, its lines joined.
fn doc_of(attrs: &[syn::Attribute]) -> String {
    let lines: Vec<String> = attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| match &a.meta {
            syn::Meta::NameValue(nv) => match &nv.value {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) => Some(s.value().trim().to_owned()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    lines.join(" ")
}

/// The interfaces an export says it implements: `implements = "a", implements = "b"`.
fn implements_of(attr: TokenStream) -> syn::Result<Vec<String>> {
    let mut out = Vec::new();
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("implements") {
            out.push(meta.value()?.parse::<syn::LitStr>()?.value());
            Ok(())
        } else {
            Err(meta.error("unknown `yog_export` argument (implements)"))
        }
    });
    syn::parse::Parser::parse(parser, attr)?;
    Ok(out)
}

#[proc_macro_attribute]
pub fn yog_export(attr: TokenStream, item: TokenStream) -> TokenStream {
    let implements = match implements_of(attr) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error().into(),
    };
    let tokens = proc_macro2::TokenStream::from(item.clone());
    if syn::parse2::<syn::ItemStruct>(tokens.clone()).is_ok()
        || syn::parse2::<syn::ItemEnum>(tokens.clone()).is_ok()
    {
        let derive = schema_derive();
        return TokenStream::from(quote! { #derive #tokens });
    }
    let func = syn::parse_macro_input!(item as syn::ItemFn);
    let name = &func.sig.ident;
    let name_str = name.to_string();
    let serve_name = format_ident!("__yog_serve_{}", name);
    let ctor_name = format_ident!("__yog_ctor_register_{}", name);
    let (registry, params) = call_params(&func.sig);
    let (args, args_def) = args_struct(name, &params);
    let output = output_type(&func.sig);
    let names: Vec<&syn::Ident> = params.iter().map(|(n, _)| n).collect();
    let doc = doc_of(&func.attrs);
    // The exporting mod's own `Registry`, from the API it captured at load.
    let (setup, registry_arg) = if registry {
        (
            quote! { let mut __yog_registry = unsafe { ::yog_api::Registry::from_raw(::yog_api::__current_api_ptr() as *const ::yog_api::YogApi) }; },
            quote! { &mut __yog_registry, },
        )
    } else {
        (quote! {}, quote! {})
    };
    TokenStream::from(quote! {
        #func

        #args_def

        #[doc(hidden)]
        unsafe extern "C" fn #serve_name(
            _ud: *mut ::std::os::raw::c_void,
            payload: ::yog_api::YogBytes,
            reply: *const ::yog_api::YogReply,
        ) -> ::yog_api::YogStatus {
            unsafe {
                ::yog_api::__yog_serve::<#args, #output>(payload, reply, |a| {
                    #setup
                    #name(#registry_arg #(a.#names),*)
                })
            }
        }

        // Runs when this mod's library loads, before `yog_mod_register`.
        #[::yog_api::ctor::ctor(unsafe, crate_path = ::yog_api::ctor)]
        #[doc(hidden)]
        fn #ctor_name() {
            ::yog_api::__yog_export_registry().lock().unwrap_or_else(|e| e.into_inner()).push(
                ::yog_api::YogExportEntry {
                    name: #name_str,
                    handler: #serve_name,
                    doc: #doc,
                    shapes: || (<#args as ::yog_api::yog_data::Schema>::shape(), <#output as ::yog_api::yog_data::Schema>::shape()),
                    implements: &[#(#implements),*],
                }
            );
        }
    })
}

// ── #[bench] ─────────────────────────────────────────────────────────────

/// Marks a function in a mod's `benches/*.rs` as a benchmark `yog bench`
/// runs: `fn name(b: &mut criterion::Bencher)` for one benchmark, or
/// `fn name(c: &mut criterion::Criterion)` to set up groups. yog generates
/// the file's `main`; this only makes the function reachable from it.
#[proc_macro_attribute]
pub fn bench(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut f = syn::parse_macro_input!(item as syn::ItemFn);
    if matches!(f.vis, syn::Visibility::Inherited) {
        f.vis = syn::parse_quote!(pub(crate));
    }
    TokenStream::from(quote! { #f })
}

// ── import! macro ────────────────────────────────────────────────────────

/// Another mod's exports, callable here: `import! { from "mod_id" { pub fn
/// name(a: A) -> R; pub struct T { .. } } }` — each fn calls `mod_id:name`
/// on the bus and answers `Option<R>` (`None`: the mod isn't loaded, or it
/// failed); each type gets the schema its values cross with.
#[proc_macro]
pub fn import(input: TokenStream) -> TokenStream {
    let import_block = syn::parse_macro_input!(input as ImportBlock);
    let mod_name = &import_block.mod_name;
    let mod_ident = format_ident!("{}", mod_name.replace('-', "_"));
    let mut items = proc_macro2::TokenStream::new();
    for item in &import_block.items {
        match item {
            ImportItem::Func(func) => items.extend(generate_import_fn(func, mod_name)),
            ImportItem::Type(t) => {
                let derive = schema_derive();
                items.extend(quote! { #derive #t });
            }
        }
    }
    TokenStream::from(quote! { pub mod #mod_ident { use super::*; #items } })
}

/// An imported function: `pub fn name(args) -> Out;` (a body, if written, is ignored).
struct ImportFn {
    vis: syn::Visibility,
    sig: syn::Signature,
}

fn generate_import_fn(func: &ImportFn, mod_name: &str) -> proc_macro2::TokenStream {
    let name = &func.sig.ident;
    let name_str = name.to_string();
    let vis = &func.vis;
    let (_, params) = call_params(&func.sig);
    let (args, args_def) = args_struct(name, &params);
    let output = output_type(&func.sig);
    let names: Vec<&syn::Ident> = params.iter().map(|(n, _)| n).collect();
    let types: Vec<&syn::Type> = params.iter().map(|(_, t)| t).collect();
    quote! {
        #args_def

        /// The other mod's answer; `None` when it isn't loaded or failed (see its log).
        #vis fn #name(#(#names: #types),*) -> ::std::option::Option<#output> {
            ::yog_api::call_export::<#args, #output>(#mod_name, #name_str, &#args { #(#names),* })
        }
    }
}

// ── Parse helpers ────────────────────────────────────────────────────────

struct ImportBlock {
    mod_name: String,
    items: Vec<ImportItem>,
}
enum ImportItem {
    Func(Box<ImportFn>),
    /// A struct or enum, as written.
    Type(proc_macro2::TokenStream),
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
            let attrs = content.call(syn::Attribute::parse_outer)?;
            let vis: syn::Visibility = content.parse()?;
            if content.peek(syn::Token![fn]) {
                let sig: syn::Signature = content.parse()?;
                if content.peek(syn::token::Brace) {
                    let _body: syn::Block = content.parse()?;
                } else {
                    content.parse::<syn::Token![;]>()?;
                }
                items.push(ImportItem::Func(Box::new(ImportFn { vis, sig })));
            } else if content.peek(syn::Token![enum]) {
                let mut e: syn::ItemEnum = content.parse()?;
                e.attrs = attrs;
                e.vis = vis;
                items.push(ImportItem::Type(quote! { #e }));
            } else {
                let mut s: syn::ItemStruct = content.parse()?;
                s.attrs = attrs;
                s.vis = vis;
                items.push(ImportItem::Type(quote! { #s }));
                let _ = content.parse::<syn::Token![;]>();
            }
        }
        Ok(ImportBlock { mod_name, items })
    }
}

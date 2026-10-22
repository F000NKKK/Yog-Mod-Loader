//! `#[nanite]` (`docs/design/nanites.md`): a Java class reached by a mod.
//!
//! On a unit struct, `#[nanite("net.minecraft.world.entity.LivingEntity")]`
//! makes it hold the object (`yog_api::nanite::Handle`). On its `impl`,
//! `#[nanite]` gives each body-less `fn` its body: the member resolved once
//! (by its name and the descriptor its Rust types make), then called by its
//! handle — `&self` an instance's, none a static one's; `#[field]` reads a
//! field (and makes `set_<name>`), `#[new]` calls a constructor.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};

/// The member attributes.
const FIELD: &str = "field";
const NEW: &str = "new";

fn api() -> TokenStream {
    quote! { ::yog_api::nanite }
}

/// `#[nanite("class")] struct X;` → `struct X(Handle);` and `impl Nanite`.
pub(crate) fn class(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let class: syn::LitStr = syn::parse2(attr)?;
    let mut s: syn::ItemStruct = syn::parse2(item)?;
    if !matches!(s.fields, syn::Fields::Unit) {
        return Err(syn::Error::new_spanned(
            &s.ident,
            "a nanite class is a unit struct: it holds the object",
        ));
    }
    let n = api();
    s.fields = syn::Fields::Unnamed(syn::parse_quote! { (#n::Handle) });
    s.semi_token = Some(Default::default());
    let ident = &s.ident;
    Ok(quote! {
        #s

        impl #n::Nanite for #ident {
            const CLASS: &'static str = #class;

            fn from_handle(handle: #n::Handle) -> Self {
                #ident(handle)
            }

            fn handle(&self) -> &#n::Handle {
                &self.0
            }
        }
    })
}

/// A member's parts: its parameters (their types for the descriptor, the
/// expressions passing them), its return type.
struct Sig {
    params: Vec<(syn::Type, TokenStream)>,
    ret: syn::Type,
    receiver: bool,
}

fn sig_of(f: &syn::ForeignItemFn) -> syn::Result<Sig> {
    let mut params = Vec::new();
    let mut receiver = false;
    for input in &f.sig.inputs {
        match input {
            syn::FnArg::Receiver(_) => receiver = true,
            syn::FnArg::Typed(t) => {
                let syn::Pat::Ident(name) = &*t.pat else {
                    return Err(syn::Error::new_spanned(
                        &t.pat,
                        "a nanite's parameters are named",
                    ));
                };
                let name = &name.ident;
                match &*t.ty {
                    syn::Type::Reference(r) => params.push(((*r.elem).clone(), quote! { #name })),
                    ty => params.push((ty.clone(), quote! { &#name })),
                }
            }
        }
    }
    let ret = match &f.sig.output {
        syn::ReturnType::Default => syn::parse_quote! { () },
        syn::ReturnType::Type(_, ty) => (**ty).clone(),
    };
    Ok(Sig {
        params,
        ret,
        receiver,
    })
}

fn has(f: &syn::ForeignItemFn, attr: &str) -> bool {
    f.attrs.iter().any(|a| a.path().is_ident(attr))
}

/// `#[nanite] impl X { fn m(&self, …) -> R; }` → each `fn` given its body,
/// its member a static beside the impl.
pub(crate) fn members(item: TokenStream) -> syn::Result<TokenStream> {
    let imp: syn::ItemImpl = syn::parse2(item)?;
    let owner = &imp.self_ty;
    let (mut fns, mut statics) = (Vec::new(), Vec::new());
    for it in &imp.items {
        let tokens = match it {
            syn::ImplItem::Verbatim(t) => t.clone(),
            other => {
                fns.push(quote! { #other });
                continue;
            }
        };
        let f: syn::ForeignItemFn = syn::parse2(tokens)?;
        let (def, member) = member(owner, &f)?;
        fns.push(def);
        statics.push(member);
    }
    let attrs = &imp.attrs;
    Ok(quote! {
        #(#attrs)*
        impl #owner {
            #(#fns)*
        }
        #(#statics)*
    })
}

/// One member: its function(s) and its static.
fn member(owner: &syn::Type, f: &syn::ForeignItemFn) -> syn::Result<(TokenStream, TokenStream)> {
    let n = api();
    let sig = sig_of(f)?;
    let name = &f.sig.ident;
    let owner_name = quote! { #owner }
        .to_string()
        .replace(' ', "")
        .to_uppercase();
    let stat = format_ident!(
        "__YOG_NANITE_{}_{}",
        owner_name,
        name.to_string().to_uppercase()
    );
    let (java_name, desc) = java_name_and_desc(f, &sig);
    let static_def = quote! {
        #[doc(hidden)]
        static #stat: #n::Member = #n::Member::new(<#owner as #n::Nanite>::CLASS, #java_name, || #desc);
    };
    let body = body(f, &sig, &stat);
    let (vis, handle) = (&f.vis, format_ident!("{}", name.to_string().to_uppercase()));
    let doc = format!("Member `{name}`, resolved once (what `#[inject]` names).");
    let body = quote! {
        #[doc = #doc]
        #vis const #handle: &'static #n::Member = &#stat;

        #body
    };
    Ok((body, static_def))
}

/// The member's Java name and the expression making its descriptor.
fn java_name_and_desc(f: &syn::ForeignItemFn, sig: &Sig) -> (TokenStream, TokenStream) {
    let n = api();
    let tys: Vec<&syn::Type> = sig.params.iter().map(|(t, _)| t).collect();
    let ret = &sig.ret;
    let java_name = syn::LitStr::new(&f.sig.ident.to_string(), Span::call_site());
    if has(f, FIELD) {
        (quote! { #java_name }, quote! { <#ret as #n::Java>::desc() })
    } else if has(f, NEW) {
        let void = quote! { <() as #n::Reply>::desc() };
        (
            quote! { #n::CONSTRUCTOR },
            quote! { #n::method_desc(&[#(<#tys as #n::Java>::desc()),*], &#void) },
        )
    } else {
        (
            quote! { #java_name },
            quote! { #n::method_desc(&[#(<#tys as #n::Java>::desc()),*], &<#ret as #n::Reply>::desc()) },
        )
    }
}

/// The function(s): a call by the member's handle, or a field's getter and setter.
fn body(f: &syn::ForeignItemFn, sig: &Sig, stat: &syn::Ident) -> TokenStream {
    let n = api();
    let args: Vec<&TokenStream> = sig.params.iter().map(|(_, a)| a).collect();
    let ret = &sig.ret;
    let target = if sig.receiver {
        quote! { #n::Nanite::handle(self).to_value() }
    } else {
        quote! { ::yog_api::yog_data::Value::Null }
    };
    let (vis, fsig) = (&f.vis, &f.sig);
    let attrs: Vec<&syn::Attribute> = f
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident(FIELD) && !a.path().is_ident(NEW))
        .collect();
    if !has(f, FIELD) {
        return quote! {
            #(#attrs)*
            #vis #fsig {
                <#ret as #n::Reply>::from_reply(#stat.call(#target, vec![#(#n::Java::to_value(#args)),*]))
            }
        };
    }
    let setter = format_ident!("set_{}", f.sig.ident);
    let me = if sig.receiver {
        quote! { &self, }
    } else {
        quote! {}
    };
    quote! {
        #(#attrs)*
        #vis #fsig {
            <#ret as #n::Reply>::from_reply(#stat.get(#target))
        }

        /// Sets the field; whether it was.
        #vis fn #setter(#me value: #ret) -> bool {
            #stat.set(#target, #n::Java::to_value(&value))
        }
    }
}

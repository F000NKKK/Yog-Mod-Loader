//! `#[derive(Schema)]` — see `yog_data::Schema`.
//!
//! - A struct with named fields is an object. Field attributes:
//!   `#[data(rename = "name")]`, `#[data(default)]` /
//!   `#[data(default = "path::to_fn")]`, `#[data(skip)]` (never read or
//!   written; `Default`). `Option` fields may be left out.
//! - A struct with one unnamed field is that field's format.
//! - An enum of unit variants is a string: the variant's name in
//!   snake_case, or `#[data(rename = "...")]`.
//! - `#[data(tag = "type")]` on an enum: an object whose `type` names the
//!   variant; a variant is a unit, has named fields, or wraps one struct
//!   whose fields sit next to the tag.
//!
//! Doc comments become the shape's docs (Yog-IDLE's hover text and the
//! docs' field tables).

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident, LitStr, Type, parse_macro_input};

#[proc_macro_derive(Schema, attributes(data))]
pub fn derive_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[derive(Default)]
struct Attrs {
    rename: Option<String>,
    default: Option<Option<syn::Path>>,
    skip: bool,
    tag: Option<String>,
}

fn attrs(list: &[syn::Attribute]) -> syn::Result<Attrs> {
    let mut a = Attrs::default();
    for attr in list.iter().filter(|a| a.path().is_ident("data")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                a.rename = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("tag") {
                a.tag = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("default") {
                a.default = Some(if meta.input.peek(syn::Token![=]) {
                    Some(meta.value()?.parse::<LitStr>()?.parse()?)
                } else {
                    None
                });
            } else if meta.path.is_ident("skip") {
                a.skip = true;
            } else {
                return Err(meta.error("unknown `data` attribute (rename, default, skip, tag)"));
            }
            Ok(())
        })?;
    }
    Ok(a)
}

fn docs(list: &[syn::Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in list.iter().filter(|a| a.path().is_ident("doc")) {
        if let syn::Meta::NameValue(nv) = &attr.meta
            && let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
        {
            lines.push(s.value().trim().to_string());
        }
    }
    // Paragraphs keep their breaks; lines of one paragraph join with spaces.
    let mut out = String::new();
    for line in lines {
        if line.is_empty() {
            if !out.ends_with("\n\n") && !out.is_empty() {
                out.push_str("\n\n");
            }
        } else {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push(' ');
            }
            out.push_str(&line);
        }
    }
    out.trim_end().to_string()
}

fn snake(ident: &Ident) -> String {
    let mut out = String::new();
    for (i, c) in ident.to_string().chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

struct NamedField {
    ident: Ident,
    ty: Type,
    key: String,
    doc: String,
    default: Option<Option<syn::Path>>,
    skip: bool,
}

fn named_fields(fields: &syn::FieldsNamed) -> syn::Result<Vec<NamedField>> {
    fields
        .named
        .iter()
        .map(|f| {
            let a = attrs(&f.attrs)?;
            let ident = f.ident.clone().unwrap();
            let key = a
                .rename
                .unwrap_or_else(|| ident.to_string().trim_start_matches("r#").to_string());
            Ok(NamedField {
                ident,
                ty: f.ty.clone(),
                key,
                doc: docs(&f.attrs),
                default: a.default,
                skip: a.skip,
            })
        })
        .collect()
}

/// `Vec<Field>` describing `fields`.
fn field_shapes(fields: &[NamedField]) -> Tokens {
    let items = fields.iter().filter(|f| !f.skip).map(|f| {
        let (key, doc, ty) = (&f.key, &f.doc, &f.ty);
        let has_default = f.default.is_some();
        quote! {
            ::yog_data::schema::Field {
                name: #key,
                doc: #doc,
                shape: <#ty as ::yog_data::Schema>::shape,
                required: !#has_default && <#ty as ::yog_data::Schema>::absent().is_none(),
            }
        }
    });
    quote! { ::std::vec![#(#items),*] }
}

/// Reads `fields` out of `map` (in scope) into `ctor { ... }`.
fn read_fields(fields: &[NamedField], ctor: Tokens) -> Tokens {
    let keys = fields.iter().filter(|f| !f.skip).map(|f| &f.key);
    let lets = fields.iter().map(|f| {
        let (ident, ty, key) = (&f.ident, &f.ty, &f.key);
        if f.skip {
            return quote! { let #ident: ::std::option::Option<#ty> = ::std::option::Option::Some(::std::default::Default::default()); };
        }
        match &f.default {
            None => quote! { let #ident = cx.field::<#ty>(map, #key); },
            Some(None) => quote! { let #ident = cx.field_or::<#ty>(map, #key, ::std::default::Default::default); },
            Some(Some(path)) => quote! { let #ident = cx.field_or::<#ty>(map, #key, #path); },
        }
    });
    let inits = fields.iter().map(|f| {
        let ident = &f.ident;
        quote! { #ident: #ident? }
    });
    quote! {
        cx.unknown_keys(map, &[#(#keys),*]);
        cx.own_keys(|cx| {
            #(#lets)*
            ::std::option::Option::Some(#ctor { #(#inits),* })
        })
    }
}

/// Inserts `fields` (bound by name) into `m` (in scope).
fn write_fields(fields: &[NamedField]) -> Tokens {
    let puts = fields.iter().filter(|f| !f.skip).map(|f| {
        let (ident, key) = (&f.ident, &f.key);
        quote! {
            if !::yog_data::Schema::skip(#ident) {
                m.insert(#key, ::yog_data::Schema::write(#ident));
            }
        }
    });
    quote! { #(#puts)* }
}

fn expand(input: &DeriveInput) -> syn::Result<Tokens> {
    let name = &input.ident;
    let (impl_g, ty_g, where_g) = input.generics.split_for_impl();
    let top = attrs(&input.attrs)?;
    let shape_name = top.rename.clone().unwrap_or_else(|| name.to_string());
    let doc = docs(&input.attrs);
    let body = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(named) => {
                let fields = named_fields(named)?;
                let shapes = field_shapes(&fields);
                let read = read_fields(&fields, quote! { Self });
                let idents: Vec<_> = fields.iter().map(|f| &f.ident).collect();
                let write = write_fields(&fields);
                quote! {
                    fn shape() -> ::yog_data::Shape {
                        ::yog_data::Shape::Struct(::yog_data::schema::StructShape { name: #shape_name, doc: #doc, fields: #shapes })
                    }
                    fn read(v: &::yog_data::Value, cx: &mut ::yog_data::Reader) -> ::std::option::Option<Self> {
                        let map = cx.map(v)?;
                        #read
                    }
                    #[allow(unused_variables)]
                    fn write(&self) -> ::yog_data::Value {
                        let Self { #(#idents),* } = self;
                        let mut m = ::yog_data::Map::new();
                        #write
                        ::yog_data::Value::Map(m)
                    }
                }
            }
            Fields::Unnamed(u) if u.unnamed.len() == 1 => {
                let ty = &u.unnamed[0].ty;
                quote! {
                    fn shape() -> ::yog_data::Shape { <#ty as ::yog_data::Schema>::shape() }
                    fn read(v: &::yog_data::Value, cx: &mut ::yog_data::Reader) -> ::std::option::Option<Self> {
                        <#ty as ::yog_data::Schema>::read(v, cx).map(Self)
                    }
                    fn write(&self) -> ::yog_data::Value { ::yog_data::Schema::write(&self.0) }
                    fn absent() -> ::std::option::Option<Self> { <#ty as ::yog_data::Schema>::absent().map(Self) }
                    fn skip(&self) -> bool { ::yog_data::Schema::skip(&self.0) }
                }
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    name,
                    "Schema: a struct needs named fields or exactly one unnamed field",
                ));
            }
        },
        Data::Enum(e) => expand_enum(e, &top, &shape_name, &doc)?,
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                name,
                "Schema: unions aren't supported",
            ));
        }
    };
    Ok(quote! {
        impl #impl_g ::yog_data::Schema for #name #ty_g #where_g {
            #body
        }
    })
}

fn expand_enum(e: &syn::DataEnum, top: &Attrs, shape_name: &str, doc: &str) -> syn::Result<Tokens> {
    struct V {
        ident: Ident,
        key: String,
        doc: String,
        kind: VKind,
    }
    enum VKind {
        Unit,
        Named(Vec<NamedField>),
        Wrap(Box<Type>),
    }
    let mut vs = Vec::new();
    for v in &e.variants {
        let a = attrs(&v.attrs)?;
        let kind = match &v.fields {
            Fields::Unit => VKind::Unit,
            Fields::Named(n) => VKind::Named(named_fields(n)?),
            Fields::Unnamed(u) if u.unnamed.len() == 1 => {
                VKind::Wrap(Box::new(u.unnamed[0].ty.clone()))
            }
            Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    &v.ident,
                    "Schema: a variant wraps one struct at most",
                ));
            }
        };
        if top.tag.is_none() && !matches!(kind, VKind::Unit) {
            return Err(syn::Error::new_spanned(
                &v.ident,
                "Schema: variants with fields need `#[data(tag = \"type\")]` on the enum",
            ));
        }
        vs.push(V {
            key: a.rename.unwrap_or_else(|| snake(&v.ident)),
            ident: v.ident.clone(),
            doc: docs(&v.attrs),
            kind,
        });
    }
    let keys: Vec<&String> = vs.iter().map(|v| &v.key).collect();
    let variant_shapes = vs.iter().map(|v| {
        let (key, doc) = (&v.key, &v.doc);
        let fields = match &v.kind {
            VKind::Unit => quote! { ::std::vec::Vec::new() },
            VKind::Named(f) => field_shapes(f),
            VKind::Wrap(ty) => quote! {
                match <#ty as ::yog_data::Schema>::shape() {
                    ::yog_data::Shape::Struct(s) => s.fields,
                    _ => ::std::vec::Vec::new(),
                }
            },
        };
        quote! { ::yog_data::schema::Variant { name: #key, doc: #doc, fields: #fields } }
    });
    let tag_tokens = match &top.tag {
        Some(t) => quote! { ::std::option::Option::Some(#t) },
        None => quote! { ::std::option::Option::None },
    };
    let shape = quote! {
        fn shape() -> ::yog_data::Shape {
            ::yog_data::Shape::Enum(::yog_data::schema::EnumShape {
                name: #shape_name,
                doc: #doc,
                tag: #tag_tokens,
                variants: ::std::vec![#(#variant_shapes),*],
            })
        }
    };

    let Some(tag) = &top.tag else {
        let reads = vs.iter().map(|v| {
            let (ident, key) = (&v.ident, &v.key);
            quote! { #key => ::std::option::Option::Some(Self::#ident), }
        });
        let writes = vs.iter().map(|v| {
            let (ident, key) = (&v.ident, &v.key);
            quote! { Self::#ident => #key, }
        });
        return Ok(quote! {
            #shape
            fn read(v: &::yog_data::Value, cx: &mut ::yog_data::Reader) -> ::std::option::Option<Self> {
                let ::yog_data::Value::Str(s) = v else {
                    cx.expected("a name", v);
                    return ::std::option::Option::None;
                };
                match s.as_str() {
                    #(#reads)*
                    other => {
                        cx.unknown_name(other, &[#(#keys),*]);
                        ::std::option::Option::None
                    }
                }
            }
            fn write(&self) -> ::yog_data::Value {
                ::yog_data::Value::Str(::std::string::String::from(match self { #(#writes)* }))
            }
        });
    };

    let reads = vs.iter().map(|v| {
        let (ident, key) = (&v.ident, &v.key);
        let body = match &v.kind {
            VKind::Unit => quote! {
                cx.with_allowed(#tag, |cx| cx.unknown_keys(map, &[]));
                ::std::option::Option::Some(Self::#ident)
            },
            VKind::Named(fields) => {
                let read = read_fields(fields, quote! { Self::#ident });
                quote! { cx.with_allowed(#tag, |cx| { #read }) }
            }
            VKind::Wrap(ty) => quote! {
                cx.with_allowed(#tag, |cx| <#ty as ::yog_data::Schema>::read(v, cx)).map(Self::#ident)
            },
        };
        quote! { #key => { #body } }
    });
    let writes = vs.iter().map(|v| {
        let (ident, key) = (&v.ident, &v.key);
        match &v.kind {
            VKind::Unit => quote! {
                Self::#ident => {
                    m.insert(#tag, ::yog_data::Value::from(#key));
                }
            },
            VKind::Named(fields) => {
                let idents: Vec<_> = fields.iter().map(|f| &f.ident).collect();
                let puts = write_fields(fields);
                quote! {
                    #[allow(unused_variables)]
                    Self::#ident { #(#idents),* } => {
                        m.insert(#tag, ::yog_data::Value::from(#key));
                        #puts
                    }
                }
            }
            VKind::Wrap(_) => quote! {
                Self::#ident(inner) => {
                    m.insert(#tag, ::yog_data::Value::from(#key));
                    if let ::yog_data::Value::Map(fields) = ::yog_data::Schema::write(inner) {
                        for (k, v) in fields {
                            m.insert(k, v);
                        }
                    }
                }
            },
        }
    });
    Ok(quote! {
        #shape
        fn read(v: &::yog_data::Value, cx: &mut ::yog_data::Reader) -> ::std::option::Option<Self> {
            let map = cx.map(v)?;
            let ::std::option::Option::Some(kind) = map.get(#tag) else {
                cx.problem(::std::format!("missing `{}` ({})", #tag, [#(#keys),*].map(|k| ::std::format!("`{k}`")).join(", ")));
                return ::std::option::Option::None;
            };
            let ::yog_data::Value::Str(kind) = kind else {
                cx.at(::yog_data::Seg::Key(::std::string::String::from(#tag)), |cx| cx.expected("a name", kind));
                return ::std::option::Option::None;
            };
            match kind.as_str() {
                #(#reads)*
                other => {
                    cx.at(::yog_data::Seg::Key(::std::string::String::from(#tag)), |cx| cx.unknown_name(other, &[#(#keys),*]));
                    ::std::option::Option::None
                }
            }
        }
        fn write(&self) -> ::yog_data::Value {
            let mut m = ::yog_data::Map::new();
            match self { #(#writes)* }
            ::yog_data::Value::Map(m)
        }
    })
}

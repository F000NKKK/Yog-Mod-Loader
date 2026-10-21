//! Enums: a tagged one (`#[data(tag = "type")]`, its variants by name
//! under the tag, fields or a wrapped struct alongside) or one of names.

use quote::quote;
use syn::{Fields, Ident, Type};

use crate::{
    Attrs, NamedField, Tokens, attrs, docs, field_shapes, fields_as_written, krate, named_fields,
    read_fields, variant_key, write_fields,
};

/// An enum of unit variants (`names`: each one's ident and key): a string.
pub(crate) fn expand_unit_enum(names: &[(&Ident, &String)], shape: Tokens) -> Tokens {
    let krate = krate();
    let keys = names.iter().map(|(_, key)| key);
    let reads: Vec<_> = names
        .iter()
        .map(|(ident, key)| quote! { #key => ::std::option::Option::Some(Self::#ident), })
        .collect();
    let writes: Vec<_> = names
        .iter()
        .map(|(ident, key)| quote! { Self::#ident => #key, })
        .collect();
    // Packed by position: variants are only ever appended.
    let indices: Vec<_> = (0..names.len() as u64).collect();
    let numbers = names
        .iter()
        .zip(&indices)
        .map(|((ident, _), i)| quote! { Self::#ident => #i, });
    let by_number = names
        .iter()
        .zip(&indices)
        .map(|((ident, _), i)| quote! { #i => ::std::option::Option::Some(Self::#ident), });
    quote! {
        #shape
        fn pack(&self, p: &mut #krate::frame::Packer) {
            p.put_u(match self { #(#numbers)* });
        }
        fn unpack(u: &mut #krate::frame::Unpacker<'_>) -> ::std::option::Option<Self> {
            match u.get_u()? {
                #(#by_number)*
                _ => ::std::option::Option::None,
            }
        }
        fn read(v: &#krate::ValueRef<'_>, cx: &mut #krate::Reader) -> ::std::option::Option<Self> {
            let #krate::ValueRef::Str(s) = v else {
                cx.expected("a name", v);
                return ::std::option::Option::None;
            };
            match *s {
                #(#reads)*
                other => {
                    cx.unknown_name(other, &[#(#keys),*]);
                    ::std::option::Option::None
                }
            }
        }
        fn write(&self) -> #krate::Value {
            #krate::Value::Str(::std::string::String::from(match self { #(#writes)* }))
        }
        fn encode(&self, w: &mut #krate::binary::Writer) {
            w.str(match self { #(#writes)* });
        }
        fn decode(d: &mut #krate::binary::Decoder<'_>) -> ::std::option::Option<Self> {
            match d.str()? {
                #(#reads)*
                _ => ::std::option::Option::None,
            }
        }
    }
}

/// A variant as the derive sees it.
struct Variant {
    ident: Ident,
    key: String,
    doc: String,
    kind: VariantKind,
    other: bool,
}

enum VariantKind {
    Unit,
    Named(Vec<NamedField>),
    Wrap(Box<Type>),
}

pub(crate) fn expand_enum(
    e: &syn::DataEnum,
    top: &Attrs,
    shape_name: &str,
    doc: &str,
) -> syn::Result<Tokens> {
    let mut vs = variants(e, top)?;
    // The catch-all variant, if any: not a name of its own.
    let other = vs.iter().position(|v| v.other).map(|i| vs.remove(i));
    let shape = enum_shape(&vs, top, shape_name, doc);
    let Some(tag) = &top.tag else {
        let names: Vec<(&Ident, &String)> = vs.iter().map(|v| (&v.ident, &v.key)).collect();
        return Ok(expand_unit_enum(&names, shape));
    };
    Ok(tagged(&vs, other.as_ref(), tag, shape))
}

/// The enum's variants, checked: fields need a tag, `other` wraps a type.
fn variants(e: &syn::DataEnum, top: &Attrs) -> syn::Result<Vec<Variant>> {
    let mut vs = Vec::new();
    for v in &e.variants {
        let a = attrs(&v.attrs)?;
        let kind = match &v.fields {
            Fields::Unit => VariantKind::Unit,
            Fields::Named(n) => VariantKind::Named(fields_as_written(|| named_fields(n))?),
            Fields::Unnamed(u) if u.unnamed.len() == 1 => {
                VariantKind::Wrap(Box::new(u.unnamed[0].ty.clone()))
            }
            Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    &v.ident,
                    "Schema: a variant wraps one struct at most",
                ));
            }
        };
        if top.tag.is_none() && !matches!(kind, VariantKind::Unit) {
            return Err(syn::Error::new_spanned(
                &v.ident,
                "Schema: variants with fields need `#[data(tag = \"type\")]` on the enum",
            ));
        }
        if a.other && (top.tag.is_none() || !matches!(kind, VariantKind::Wrap(_))) {
            return Err(syn::Error::new_spanned(
                &v.ident,
                "Schema: `other` is a tagged enum's variant wrapping a type that reads the whole object, tag included",
            ));
        }
        vs.push(Variant {
            key: a.rename.unwrap_or_else(|| variant_key(&v.ident)),
            ident: v.ident.clone(),
            doc: docs(&v.attrs),
            kind,
            other: a.other,
        });
    }
    Ok(vs)
}

/// The enum's `shape()`.
fn enum_shape(vs: &[Variant], top: &Attrs, shape_name: &str, doc: &str) -> Tokens {
    let krate = krate();
    let variant_shapes = vs.iter().map(|v| {
        let (key, doc) = (&v.key, &v.doc);
        let fields = match &v.kind {
            VariantKind::Unit => quote! { ::std::vec::Vec::new() },
            VariantKind::Named(f) => field_shapes(f),
            VariantKind::Wrap(ty) => quote! {
                match <#ty as #krate::Schema>::shape() {
                    #krate::Shape::Struct(s) => s.fields,
                    _ => ::std::vec::Vec::new(),
                }
            },
        };
        quote! { #krate::schema::Variant { name: #key, doc: #doc, fields: #fields } }
    });
    let tag_tokens = match &top.tag {
        Some(t) => quote! { ::std::option::Option::Some(#t) },
        None => quote! { ::std::option::Option::None },
    };
    quote! {
        fn shape() -> #krate::Shape {
            #krate::Shape::Enum(#krate::schema::EnumShape {
                name: #shape_name,
                doc: #doc,
                tag: #tag_tokens,
                variants: ::std::vec![#(#variant_shapes),*],
            })
        }
    }
}

/// A tagged enum's `read` and `write`: the tag names the variant.
fn tagged(vs: &[Variant], other: Option<&Variant>, tag: &str, shape: Tokens) -> Tokens {
    let krate = krate();
    let keys: Vec<&String> = vs.iter().map(|v| &v.key).collect();
    let reads = vs.iter().map(|v| tagged_read(v, tag));
    let writes = vs.iter().map(|v| tagged_write(v, tag));
    let (fallback, other_write) = match other {
        Some(Variant {
            ident,
            kind: VariantKind::Wrap(ty),
            ..
        }) => (
            quote! { _ => <#ty as #krate::Schema>::read(v, cx).map(Self::#ident), },
            quote! { Self::#ident(inner) => { m = match #krate::Schema::write(inner) { #krate::Value::Map(fields) => fields, _ => m }; } },
        ),
        _ => (
            quote! {
                other => {
                    cx.at(#krate::Seg::Key(::std::string::String::from(#tag)), |cx| cx.unknown_name(other, &[#(#keys),*]));
                    ::std::option::Option::None
                }
            },
            quote! {},
        ),
    };
    quote! {
        #shape
        fn read(v: &#krate::ValueRef<'_>, cx: &mut #krate::Reader) -> ::std::option::Option<Self> {
            let map = cx.map(v)?;
            let ::std::option::Option::Some(kind) = map.get(#tag) else {
                cx.problem(::std::format!("missing `{}` ({})", #tag, [#(#keys),*].map(|k| ::std::format!("`{k}`")).join(", ")));
                return ::std::option::Option::None;
            };
            let #krate::ValueRef::Str(kind) = kind else {
                cx.at(#krate::Seg::Key(::std::string::String::from(#tag)), |cx| cx.expected("a name", kind));
                return ::std::option::Option::None;
            };
            match *kind {
                #(#reads)*
                #fallback
            }
        }
        fn write(&self) -> #krate::Value {
            let mut m = #krate::Map::new();
            match self { #(#writes)* #other_write }
            #krate::Value::Map(m)
        }
    }
}

/// A variant read under its tag's name.
fn tagged_read(v: &Variant, tag: &str) -> Tokens {
    let krate = krate();
    let (ident, key) = (&v.ident, &v.key);
    let body = match &v.kind {
        VariantKind::Unit => quote! {
            cx.with_allowed(#tag, |cx| cx.unknown_keys(map, &[]));
            ::std::option::Option::Some(Self::#ident)
        },
        VariantKind::Named(fields) => {
            let read = read_fields(fields, quote! { Self::#ident });
            quote! { cx.with_allowed(#tag, |cx| { #read }) }
        }
        VariantKind::Wrap(ty) => quote! {
            cx.with_allowed(#tag, |cx| <#ty as #krate::Schema>::read(v, cx)).map(Self::#ident)
        },
    };
    quote! { #key => { #body } }
}

/// A variant written with its tag's name.
fn tagged_write(v: &Variant, tag: &str) -> Tokens {
    let krate = krate();
    let (ident, key) = (&v.ident, &v.key);
    match &v.kind {
        VariantKind::Unit => quote! {
            Self::#ident => {
                m.insert(#tag, #krate::Value::from(#key));
            }
        },
        VariantKind::Named(fields) => {
            let idents: Vec<_> = fields.iter().map(|f| &f.ident).collect();
            let puts = write_fields(fields);
            quote! {
                // A skipped field is bound and not written.
                #[allow(unused_variables)]
                Self::#ident { #(#idents),* } => {
                    m.insert(#tag, #krate::Value::from(#key));
                    #puts
                }
            }
        }
        VariantKind::Wrap(_) => quote! {
            Self::#ident(inner) => {
                m.insert(#tag, #krate::Value::from(#key));
                if let #krate::Value::Map(fields) = #krate::Schema::write(inner) {
                    for (k, v) in fields {
                        m.insert(k, v);
                    }
                }
            }
        },
    }
}

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
//!   variant; a variant is a unit, has named fields, or wraps one value —
//!   a struct's fields sit next to the tag, anything else under `value`
//!   (`yog_data::schema::WRAPPED`).
//!
//! - `#[data(view)]` on a struct: `NameView<'a>` too, the struct read out
//!   of a frame without copying (`yog_data::frame::View`).
//!
//! Every type also packs itself into frames (`pack` / `unpack`: fields by
//! position, unit enums by index) and writes the binary form straight
//! (`encode` / `decode`).
//!
//! Doc comments become the shape's docs (Yog-IDLE's hover text and the
//! docs' field tables).

mod attr_name;
mod enums;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident, LitStr, Type, parse_macro_input};

thread_local! {
    /// The path of `yog_data` for the derive running now: `::yog_data`, or
    /// the container's `#[data(crate = "...")]` (a crate that re-exports it).
    static KRATE: std::cell::RefCell<Tokens> = std::cell::RefCell::new(quote!(::yog_data));
    /// Whether the container says `#[data(rename_all = "camelCase")]`.
    static CAMEL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The path of `yog_data` the generated code uses.
fn krate() -> Tokens {
    KRATE.with(|k| k.borrow().clone())
}

/// Where `yog_data` is for the crate deriving: its own dependency (or the
/// crate itself), else yog-api's re-export — a mod's only dependency.
fn default_path() -> syn::Path {
    use proc_macro_crate::{FoundCrate, crate_name};
    use yog_ids::rust_crate;
    let own = quote::format_ident!("{}", rust_crate::DATA_RUST_NAME);
    match crate_name(rust_crate::DATA) {
        Ok(FoundCrate::Name(name)) => {
            let name = quote::format_ident!("{name}");
            return syn::parse_quote!(::#name);
        }
        Ok(FoundCrate::Itself) => return syn::parse_quote!(::#own),
        Err(_) => {}
    }
    match crate_name(rust_crate::API) {
        Ok(FoundCrate::Name(api)) => {
            let api = quote::format_ident!("{api}");
            syn::parse_quote!(::#api::#own)
        }
        _ => syn::parse_quote!(::#own),
    }
}

#[proc_macro_derive(Schema, attributes(data))]
pub fn derive_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let mut container = match attrs(&input.attrs) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };
    // An enum with data in its variants is tagged, by `type` unless it says.
    if let syn::Data::Enum(e) = &input.data
        && container.tag.is_none()
        && e.variants
            .iter()
            .any(|v| !matches!(v.fields, syn::Fields::Unit))
    {
        container.tag = Some(yog_ids::word::TYPE.to_owned());
    }
    let camel = match container.rename_all.as_deref() {
        None => false,
        Some(attr_name::CAMEL_CASE) => true,
        Some(_) => {
            return syn::Error::new_spanned(
                &input.ident,
                "rename_all = \"camelCase\" is the one case",
            )
            .to_compile_error()
            .into();
        }
    };
    CAMEL.with(|c| c.set(camel));
    let path = container.krate.take().unwrap_or_else(default_path);
    KRATE.with(|k| *k.borrow_mut() = quote!(#path));
    match expand(&input, &container) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[derive(Default)]
struct Attrs {
    rename: Option<String>,
    /// The container's case for its fields' or variants' names.
    rename_all: Option<String>,
    default: Option<Option<syn::Path>>,
    skip: bool,
    tag: Option<String>,
    with: Option<Type>,
    range: Option<(f64, f64)>,
    /// A tagged enum's variant for every tag the others don't take.
    other: bool,
    /// The container's path of `yog_data` (a crate re-exporting it).
    krate: Option<syn::Path>,
    /// A struct gets a view of frames (`yog_data::frame::View`).
    view: bool,
}

fn attrs(list: &[syn::Attribute]) -> syn::Result<Attrs> {
    let mut a = Attrs::default();
    for attr in list.iter().filter(|a| a.path().is_ident(attr_name::DATA)) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(attr_name::RENAME) {
                a.rename = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident(attr_name::RENAME_ALL) {
                a.rename_all = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident(attr_name::TAG) {
                a.tag = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident(attr_name::DEFAULT) {
                a.default = Some(if meta.input.peek(syn::Token![=]) {
                    Some(meta.value()?.parse::<LitStr>()?.parse()?)
                } else {
                    None
                });
            } else if meta.path.is_ident(attr_name::RANGE) {
                let lit = meta.value()?.parse::<LitStr>()?;
                let text = lit.value();
                let parsed = text.split_once("..=").and_then(|(lo, hi)| {
                    Some((
                        lo.trim().parse::<f64>().ok()?,
                        hi.trim().parse::<f64>().ok()?,
                    ))
                });
                a.range = Some(parsed.ok_or_else(|| {
                    syn::Error::new_spanned(&lit, "range = \"lo..=hi\" (numbers)")
                })?);
            } else if meta.path.is_ident(attr_name::WITH) {
                a.with = Some(meta.value()?.parse::<LitStr>()?.parse()?);
            } else if meta.path.is_ident(attr_name::SKIP) {
                a.skip = true;
            } else if meta.path.is_ident(attr_name::VIEW) {
                a.view = true;
            } else if meta.path.is_ident(attr_name::OTHER) {
                a.other = true;
            } else if meta.path.is_ident(attr_name::CRATE) {
                a.krate = Some(meta.value()?.parse::<LitStr>()?.parse()?);
            } else {
                return Err(meta
                    .error("unknown `data` attribute (rename, rename_all, default, skip, tag, with, range, other, crate, view)"));
            }
            Ok(())
        })?;
    }
    Ok(a)
}

fn docs(list: &[syn::Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in list.iter().filter(|a| a.path().is_ident(attr_name::DOC)) {
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

/// A field's name as written: as in Rust, or in camel case (`rename_all`).
fn field_key(ident: &Ident) -> String {
    let name = ident.to_string().trim_start_matches("r#").to_string();
    if !CAMEL.with(std::cell::Cell::get) {
        return name;
    }
    let mut out = String::new();
    let mut upper = false;
    for c in name.chars() {
        match c {
            '_' => upper = !out.is_empty(),
            c if upper => {
                out.push(c.to_ascii_uppercase());
                upper = false;
            }
            c => out.push(c),
        }
    }
    out
}

/// `f` with fields named as in Rust: an enum's `rename_all` names its
/// variants, not their fields.
fn fields_as_written<T>(f: impl FnOnce() -> T) -> T {
    let camel = CAMEL.with(|c| c.replace(false));
    let out = f();
    CAMEL.with(|c| c.set(camel));
    out
}

/// A variant's name as written: in snake case, or camel case (`rename_all`).
fn variant_key(ident: &Ident) -> String {
    let name = ident.to_string();
    if CAMEL.with(std::cell::Cell::get) {
        yog_codegen::camel(&name)
    } else {
        yog_codegen::snake(&name)
    }
}

struct NamedField {
    ident: Ident,
    ty: Type,
    key: String,
    doc: String,
    default: Option<Option<syn::Path>>,
    skip: bool,
    /// The codec: `with = "…"`, else `Plain` (the type's own schema).
    with: Type,
    /// Whether `with` names a codec (else it's the type's own schema).
    codec: bool,
    range: Option<(f64, f64)>,
}

fn named_fields(fields: &syn::FieldsNamed) -> syn::Result<Vec<NamedField>> {
    let krate = krate();
    fields
        .named
        .iter()
        .map(|f| {
            let a = attrs(&f.attrs)?;
            let Some(ident) = f.ident.clone() else {
                return Err(syn::Error::new_spanned(
                    f,
                    "Schema: a named field without a name",
                ));
            };
            let key = a.rename.unwrap_or_else(|| field_key(&ident));
            Ok(NamedField {
                ident,
                ty: f.ty.clone(),
                key,
                doc: docs(&f.attrs),
                default: a.default,
                skip: a.skip,
                codec: a.with.is_some(),
                with: a
                    .with
                    .unwrap_or_else(|| syn::parse_quote!(#krate::with::Plain)),
                range: a.range,
            })
        })
        .collect()
}

/// `Vec<Field>` describing `fields`.
fn field_shapes(fields: &[NamedField]) -> Tokens {
    let krate = krate();
    let items = fields.iter().filter(|f| !f.skip).map(|f| {
        let (key, doc, ty, w) = (&f.key, &f.doc, &f.ty, &f.with);
        let has_default = f.default.is_some();
        let range = match f.range {
            Some((lo, hi)) => quote! { ::std::option::Option::Some((#lo, #hi)) },
            None => quote! { ::std::option::Option::None },
        };
        quote! {
            #krate::schema::Field {
                name: #key,
                doc: #doc,
                shape: <#w as #krate::with::With<#ty>>::shape,
                required: !#has_default && <#w as #krate::with::With<#ty>>::absent().is_none(),
                range: #range,
            }
        }
    });
    quote! { ::std::vec![#(#items),*] }
}

/// Reads `fields` out of `map` (in scope) into `ctor { ... }`.
fn read_fields(fields: &[NamedField], ctor: Tokens) -> Tokens {
    let keys = fields.iter().filter(|f| !f.skip).map(|f| &f.key);
    let lets = fields.iter().map(|f| {
        let (ident, ty, key, w) = (&f.ident, &f.ty, &f.key, &f.with);
        if f.skip {
            return quote! { let #ident: ::std::option::Option<#ty> = ::std::option::Option::Some(::std::default::Default::default()); };
        }
        match &f.default {
            None => quote! { let #ident = cx.field_with::<#w, #ty>(map, #key); },
            Some(None) => quote! { let #ident = cx.field_or_with::<#w, #ty>(map, #key, ::std::default::Default::default); },
            Some(Some(path)) => quote! { let #ident = cx.field_or_with::<#w, #ty>(map, #key, #path); },
        }
    });
    let ranges = fields.iter().filter_map(|f| {
        let (ident, key) = (&f.ident, &f.key);
        let (lo, hi) = f.range?;
        Some(quote! { let #ident = cx.in_range(#key, #ident, #lo, #hi); })
    });
    let inits = fields.iter().map(|f| {
        let ident = &f.ident;
        quote! { #ident: #ident? }
    });
    quote! {
        cx.unknown_keys(map, &[#(#keys),*]);
        cx.own_keys(|cx| {
            #(#lets)*
            #(#ranges)*
            ::std::option::Option::Some(#ctor { #(#inits),* })
        })
    }
}

/// Inserts `fields` (bound by name) into `m` (in scope).
fn write_fields(fields: &[NamedField]) -> Tokens {
    let krate = krate();
    let puts = fields.iter().filter(|f| !f.skip).map(|f| {
        let (ident, key, ty, w) = (&f.ident, &f.key, &f.ty, &f.with);
        quote! {
            if !<#w as #krate::with::With<#ty>>::skip(#ident) {
                m.insert(#key, <#w as #krate::with::With<#ty>>::write(#ident));
            }
        }
    });
    quote! { #(#puts)* }
}

/// `fields` (bound by name) written straight as a map.
fn encode_fields(fields: &[NamedField]) -> Tokens {
    let krate = krate();
    let written: Vec<_> = fields.iter().filter(|f| !f.skip).collect();
    let counts = written.iter().map(|f| {
        let (ident, ty, w) = (&f.ident, &f.ty, &f.with);
        quote! { + usize::from(!<#w as #krate::with::With<#ty>>::skip(#ident)) }
    });
    let puts = written.iter().map(|f| {
        let (ident, key, ty, w) = (&f.ident, &f.key, &f.ty, &f.with);
        quote! {
            if !<#w as #krate::with::With<#ty>>::skip(#ident) {
                __yog_w.key(#key);
                <#w as #krate::with::With<#ty>>::encode(#ident, __yog_w);
            }
        }
    });
    quote! {
        __yog_w.map(0 #(#counts)*);
        #(#puts)*
    }
}

/// `ctor { ... }` read straight from a map — `None` (the slow way) for a
/// key it doesn't know, or a field with a range to check.
fn decode_fields(fields: &[NamedField], ctor: Tokens) -> Option<Tokens> {
    let krate = krate();
    if fields.iter().any(|f| f.range.is_some()) {
        return None;
    }
    let read: Vec<_> = fields.iter().filter(|f| !f.skip).collect();
    // Each field's slot, named apart from anything a default may call.
    let slot = |f: &NamedField| quote::format_ident!("__yog_field_{}", f.ident);
    let slots = read.iter().map(|f| {
        let (s, ty) = (slot(f), &f.ty);
        quote! { let mut #s: ::std::option::Option<#ty> = ::std::option::Option::None; }
    });
    let arms = read.iter().map(|f| {
        let (s, key, ty, w) = (slot(f), &f.key, &f.ty, &f.with);
        quote! { #key => #s = ::std::option::Option::Some(<#w as #krate::with::With<#ty>>::decode(__yog_d)?), }
    });
    let inits = fields.iter().map(|f| {
        let (ident, ty, w) = (&f.ident, &f.ty, &f.with);
        let value = if f.skip {
            quote! { ::std::default::Default::default() }
        } else {
            let missing = match &f.default {
                None => quote! { <#w as #krate::with::With<#ty>>::absent()? },
                Some(None) => quote! { ::std::default::Default::default() },
                Some(Some(path)) => quote! { #path() },
            };
            let s = slot(f);
            quote! { match #s { ::std::option::Option::Some(__yog_v) => __yog_v, ::std::option::Option::None => #missing } }
        };
        quote! { #ident: #value }
    });
    Some(quote! {
        let __yog_n = __yog_d.map()?;
        #(#slots)*
        for _ in 0..__yog_n {
            match __yog_d.key()? {
                #(#arms)*
                _ => return ::std::option::Option::None,
            }
        }
        __yog_d.leave();
        ::std::option::Option::Some(#ctor { #(#inits),* })
    })
}

/// `fields` (bound by name) packed into a frame: a record, every field in order.
fn pack_fields(fields: &[NamedField]) -> Tokens {
    let krate = krate();
    let puts = fields.iter().filter(|f| !f.skip).map(|f| {
        let (ident, ty, w) = (&f.ident, &f.ty, &f.with);
        quote! { <#w as #krate::with::With<#ty>>::pack(#ident, __yog_p); }
    });
    quote! {
        let __yog_at = __yog_p.begin();
        #(#puts)*
        __yog_p.end(__yog_at);
    }
}

/// `ctor { ... }` out of a frame's record: the fields it has in order, the
/// rest their defaults; fields it has that this type doesn't know skipped.
/// A frame comes from a writer of the same schema: ranges aren't checked again.
fn unpack_fields(fields: &[NamedField], ctor: Tokens) -> Tokens {
    let krate = krate();
    let slot = |f: &NamedField| quote::format_ident!("__yog_field_{}", f.ident);
    let gets = fields.iter().filter(|f| !f.skip).map(|f| {
        let (s, ty, w) = (slot(f), &f.ty, &f.with);
        quote! {
            let #s: ::std::option::Option<#ty> = if __yog_u.more(__yog_end) {
                ::std::option::Option::Some(<#w as #krate::with::With<#ty>>::unpack(__yog_u)?)
            } else {
                ::std::option::Option::None
            };
        }
    });
    let inits = fields.iter().map(|f| {
        let (ident, ty, w) = (&f.ident, &f.ty, &f.with);
        let value = if f.skip {
            quote! { ::std::default::Default::default() }
        } else {
            let s = slot(f);
            let missing = match &f.default {
                None => quote! { <#w as #krate::with::With<#ty>>::absent()? },
                Some(None) => quote! { ::std::default::Default::default() },
                Some(Some(path)) => quote! { #path() },
            };
            quote! { match #s { ::std::option::Option::Some(__yog_v) => __yog_v, ::std::option::Option::None => #missing } }
        };
        quote! { #ident: #value }
    });
    quote! {
        let __yog_end = __yog_u.begin()?;
        #(#gets)*
        __yog_u.leave(__yog_end)?;
        ::std::option::Option::Some(#ctor { #(#inits),* })
    }
}

/// A struct with named fields: an object.
fn expand_named(named: &syn::FieldsNamed, shape_name: &str, doc: &str) -> syn::Result<Tokens> {
    let krate = krate();
    let fields = named_fields(named)?;
    let shapes = field_shapes(&fields);
    let read = read_fields(&fields, quote! { Self });
    let idents: Vec<_> = fields.iter().map(|f| &f.ident).collect();
    let write = write_fields(&fields);
    let encode = encode_fields(&fields);
    let decode = decode_fields(&fields, quote! { Self }).map(|body| {
        quote! {
            fn decode(__yog_d: &mut #krate::binary::Decoder<'_>) -> ::std::option::Option<Self> {
                #body
            }
        }
    });
    let pack = pack_fields(&fields);
    let unpack = unpack_fields(&fields, quote! { Self });
    Ok(quote! {
        // A skipped field is bound and not encoded.
        #[allow(unused_variables)]
        fn encode(&self, __yog_w: &mut #krate::binary::Writer) {
            let Self { #(#idents),* } = self;
            #encode
        }
        #decode
        // A skipped field is bound and not packed.
        #[allow(unused_variables)]
        fn pack(&self, __yog_p: &mut #krate::frame::Packer) {
            let Self { #(#idents),* } = self;
            #pack
        }
        fn unpack(__yog_u: &mut #krate::frame::Unpacker<'_>) -> ::std::option::Option<Self> {
            #unpack
        }
        fn shape() -> #krate::Shape {
            #krate::Shape::Struct(#krate::schema::StructShape { name: #shape_name, doc: #doc, fields: #shapes })
        }
        fn read(v: &#krate::ValueRef<'_>, cx: &mut #krate::Reader) -> ::std::option::Option<Self> {
            let map = cx.map(v)?;
            #read
        }
        // A skipped field is bound and not written.
        #[allow(unused_variables)]
        fn write(&self) -> #krate::Value {
            let Self { #(#idents),* } = self;
            let mut m = #krate::Map::new();
            #write
            #krate::Value::Map(m)
        }
    })
}

/// `NameView<'a>` and `Name`'s `View` (`#[data(view)]`): every field
/// viewed (a codec's field read whole), a field an older writer didn't
/// have making the view `None` — read the owned way then.
fn view_items(input: &DeriveInput, fields: &[NamedField]) -> syn::Result<Tokens> {
    let krate = krate();
    let (name, vis) = (&input.ident, &input.vis);
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            name,
            "Schema: `view` is for a struct without generics",
        ));
    }
    let view = quote::format_ident!("{}View", name);
    let shown: Vec<&NamedField> = fields.iter().filter(|f| !f.skip).collect();
    let types = shown.iter().map(|f| {
        let (ident, ty) = (&f.ident, &f.ty);
        if f.codec {
            quote! { pub #ident: #ty, }
        } else {
            quote! { pub #ident: <#ty as #krate::frame::View<'a>>::View, }
        }
    });
    let reads = shown.iter().map(|f| {
        let (ident, ty, w) = (&f.ident, &f.ty, &f.with);
        let read = if f.codec {
            quote! { <#w as #krate::with::With<#ty>>::unpack(__yog_u)? }
        } else {
            quote! { <#ty as #krate::frame::View<'a>>::unpack_view(__yog_u)? }
        };
        quote! { let #ident = if __yog_u.more(__yog_end) { #read } else { return ::std::option::Option::None }; }
    });
    let idents = shown.iter().map(|f| &f.ident);
    let doc =
        format!("[`{name}`] viewed in a frame: strings borrowed, packed numbers read in place.");
    Ok(quote! {
        #[doc = #doc]
        #vis struct #view<'a> {
            #(#types)*
            _frame: ::std::marker::PhantomData<&'a ()>,
        }

        impl<'a> #krate::frame::View<'a> for #name {
            type View = #view<'a>;
            type ListView = #krate::frame::List<'a, Self>;
            fn unpack_view(__yog_u: &mut #krate::frame::Unpacker<'a>) -> ::std::option::Option<#view<'a>> {
                let __yog_end = __yog_u.begin()?;
                #(#reads)*
                __yog_u.leave(__yog_end)?;
                ::std::option::Option::Some(#view { #(#idents,)* _frame: ::std::marker::PhantomData })
            }
            fn unpack_list_view(__yog_u: &mut #krate::frame::Unpacker<'a>) -> ::std::option::Option<Self::ListView> {
                #krate::frame::list(__yog_u)
            }
        }
    })
}

fn expand(input: &DeriveInput, top: &Attrs) -> syn::Result<Tokens> {
    let krate = krate();
    let name = &input.ident;
    let (impl_g, ty_g, where_g) = input.generics.split_for_impl();
    let shape_name = top.rename.clone().unwrap_or_else(|| name.to_string());
    let doc = docs(&input.attrs);
    let body = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(named) => expand_named(named, &shape_name, &doc)?,
            Fields::Unnamed(u) if u.unnamed.len() == 1 => expand_newtype(&u.unnamed[0].ty),
            _ => {
                return Err(syn::Error::new_spanned(
                    name,
                    "Schema: a struct needs named fields or exactly one unnamed field",
                ));
            }
        },
        Data::Enum(e) => enums::expand_enum(e, top, &shape_name, &doc)?,
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                name,
                "Schema: unions aren't supported",
            ));
        }
    };
    let view = view_of(input, top)?;
    let fields = fields_of(input)?;
    Ok(quote! {
        impl #impl_g #krate::Schema for #name #ty_g #where_g {
            #body
        }
        #view
        #fields
    })
}

/// A struct's fields by their Rust names, as files write them (`field!`).
fn fields_of(input: &DeriveInput) -> syn::Result<Tokens> {
    let krate = krate();
    let name = &input.ident;
    let (impl_g, ty_g, where_g) = input.generics.split_for_impl();
    let Data::Struct(syn::DataStruct {
        fields: Fields::Named(named),
        ..
    }) = &input.data
    else {
        return Ok(quote! {});
    };
    let pairs = named_fields(named)?.into_iter().map(|f| {
        let ident = f.ident.to_string();
        let ident = ident.trim_start_matches("r#");
        let key = &f.key;
        quote! { (#ident, #key) }
    });
    Ok(quote! {
        impl #impl_g #krate::schema::Fields for #name #ty_g #where_g {
            const FIELDS: &'static [(&'static str, &'static str)] = &[#(#pairs),*];
        }
    })
}

/// A struct of one unnamed field: its field's schema throughout.
fn expand_newtype(ty: &Type) -> Tokens {
    let krate = krate();
    quote! {
        fn shape() -> #krate::Shape { <#ty as #krate::Schema>::shape() }
        fn read(v: &#krate::ValueRef<'_>, cx: &mut #krate::Reader) -> ::std::option::Option<Self> {
            <#ty as #krate::Schema>::read(v, cx).map(Self)
        }
        fn write(&self) -> #krate::Value { #krate::Schema::write(&self.0) }
        fn encode(&self, w: &mut #krate::binary::Writer) { #krate::Schema::encode(&self.0, w) }
        fn decode(d: &mut #krate::binary::Decoder<'_>) -> ::std::option::Option<Self> {
            <#ty as #krate::Schema>::decode(d).map(Self)
        }
        fn pack(&self, p: &mut #krate::frame::Packer) { #krate::Schema::pack(&self.0, p) }
        fn unpack(u: &mut #krate::frame::Unpacker<'_>) -> ::std::option::Option<Self> {
            <#ty as #krate::Schema>::unpack(u).map(Self)
        }
        fn absent() -> ::std::option::Option<Self> { <#ty as #krate::Schema>::absent().map(Self) }
        fn skip(&self) -> bool { #krate::Schema::skip(&self.0) }
    }
}

/// The type's view of frames: a struct's (`#[data(view)]`), an enum of names' (itself).
fn view_of(input: &DeriveInput, top: &Attrs) -> syn::Result<Tokens> {
    let krate = krate();
    let name = &input.ident;
    Ok(match (&input.data, top.view) {
        (
            Data::Struct(syn::DataStruct {
                fields: Fields::Named(named),
                ..
            }),
            true,
        ) => view_items(input, &named_fields(named)?)?,
        (_, true) => {
            return Err(syn::Error::new_spanned(
                name,
                "Schema: `view` is for a struct with named fields",
            ));
        }
        (Data::Enum(e), false)
            if top.tag.is_none()
                && input.generics.params.is_empty()
                && e.variants.iter().all(|v| matches!(v.fields, Fields::Unit)) =>
        {
            quote! {
                impl<'a> #krate::frame::View<'a> for #name {
                    type View = Self;
                    type ListView = ::std::vec::Vec<Self>;
                    fn unpack_view(u: &mut #krate::frame::Unpacker<'a>) -> ::std::option::Option<Self> {
                        <Self as #krate::Schema>::unpack(u)
                    }
                    fn unpack_list_view(u: &mut #krate::frame::Unpacker<'a>) -> ::std::option::Option<::std::vec::Vec<Self>> {
                        <Self as #krate::Schema>::unpack_list(u)
                    }
                }
            }
        }
        _ => quote! {},
    })
}

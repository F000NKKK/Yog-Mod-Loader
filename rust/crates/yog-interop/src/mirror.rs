//! `#[yog_mirror]` (`docs/design/bus.md`, "Mirrors"): a struct made a
//! mirror's row — its spec, a bit const a field, a view and a writer a row.
//!
//! ```ignore
//! #[yog_mirror(space = yog_api::yog_ids::key_space::ENTITY)]
//! pub struct Threat {
//!     #[fact(yog_api::yog_ids::fact::ENTITY_POS)]   // the host's: linked
//!     pub pos: [f64; 3],
//!     pub level: f32,                               // its own
//! }
//! ```

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// The field attribute naming a field's fact.
const FACT: &str = "fact";
/// The mirror attribute's keys.
const SPACE: &str = "space";
const OWNER: &str = "owner";

/// What `#[yog_mirror(…)]` says.
struct Args {
    space: syn::Expr,
    owner: Option<syn::LitStr>,
}

fn args_of(attr: TokenStream) -> syn::Result<Args> {
    let (mut space, mut owner) = (None, None);
    let parser = syn::meta::parser(|m| {
        if m.path.is_ident(SPACE) {
            space = Some(m.value()?.parse::<syn::Expr>()?);
        } else if m.path.is_ident(OWNER) {
            owner = Some(m.value()?.parse::<syn::LitStr>()?);
        } else {
            return Err(m.error(format!("expected `{SPACE} = …` or `{OWNER} = \"…\"`")));
        }
        Ok(())
    });
    syn::parse::Parser::parse2(parser, attr)?;
    let space = space.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("`{SPACE} = …` is required"),
        )
    })?;
    Ok(Args { space, owner })
}

/// `PipeFlow` → `pipe_flow`.
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push_str(yog_ids::sep::WORD);
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// A field: its name, type and fact (its `#[fact(…)]` taken off).
struct Field {
    ident: syn::Ident,
    ty: syn::Type,
    fact: Option<syn::Expr>,
}

fn fields_of(item: &mut syn::ItemStruct) -> syn::Result<Vec<Field>> {
    let syn::Fields::Named(named) = &mut item.fields else {
        return Err(syn::Error::new_spanned(
            &item.ident,
            "a mirror's fields are named",
        ));
    };
    named
        .named
        .iter_mut()
        .map(|f| {
            let mut fact = None;
            for a in f.attrs.iter().filter(|a| a.path().is_ident(FACT)) {
                fact = Some(a.parse_args::<syn::Expr>()?);
            }
            f.attrs.retain(|a| !a.path().is_ident(FACT));
            Ok(Field {
                ident: f.ident.clone().unwrap_or_else(|| format_ident!("_")),
                ty: f.ty.clone(),
                fact,
            })
        })
        .collect()
}

/// The path the generated code reaches mirrors by.
fn api() -> TokenStream {
    quote! { ::yog_api::mirror }
}

/// What a generated item needs of the struct: its visibility and name.
struct Item<'a> {
    vis: &'a syn::Visibility,
    ident: &'a syn::Ident,
    fields: &'a [Field],
}

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let args = args_of(attr)?;
    let mut parsed = syn::parse2::<syn::ItemStruct>(item)?;
    let fields = fields_of(&mut parsed)?;
    if fields.len() > u64::BITS as usize {
        let most = u64::BITS;
        return Err(syn::Error::new_spanned(
            &parsed.ident,
            format!("a mirror has {most} fields at most"),
        ));
    }
    let it = Item {
        vis: &parsed.vis,
        ident: &parsed.ident,
        fields: &fields,
    };
    let (bits, mirror, view, writer) = (bits(&it), mirror_impl(&it, &args), view(&it), writer(&it));
    Ok(quote! { #parsed #bits #mirror #view #writer })
}

/// A bit const a field: `Threat::POS`.
fn bits(it: &Item) -> TokenStream {
    let (vis, ident) = (it.vis, it.ident);
    let bits = it.fields.iter().enumerate().map(|(i, f)| {
        let c = format_ident!("{}", f.ident.to_string().to_uppercase());
        let doc = format!("Field `{}`'s bit.", f.ident);
        quote! { #[doc = #doc] #vis const #c: u64 = 1 << #i; }
    });
    quote! { impl #ident { #(#bits)* } }
}

/// `impl Mirror`: its name, space, fields, and the whole row read and written.
fn mirror_impl(it: &Item, args: &Args) -> TokenStream {
    let (m, ident) = (api(), it.ident);
    let name = snake(&ident.to_string());
    let (view, writer) = (format_ident!("{ident}View"), format_ident!("{ident}Writer"));
    let space = &args.space;
    let owner = match &args.owner {
        Some(o) => quote! { Some(#o) },
        None => quote! { None },
    };
    let all = match it.fields.len() {
        0 => 0,
        n => u64::MAX >> (u64::BITS as usize - n),
    };
    let specs = it.fields.iter().map(|f| {
        let (n, ty) = (f.ident.to_string(), &f.ty);
        let fact = match &f.fact {
            Some(e) => quote! { Some((#e).to_owned()) },
            None => quote! { None },
        };
        quote! { #m::FieldSpec {
            name: #n.to_owned(),
            fact: #fact,
            kind: <#ty as #m::Cell>::KIND.code(),
            count: <#ty as #m::Cell>::COUNT,
        } }
    });
    let names: Vec<&syn::Ident> = it.fields.iter().map(|f| &f.ident).collect();
    let indices: Vec<usize> = (0..it.fields.len()).collect();
    quote! {
        impl #m::Mirror for #ident {
            const NAME: &'static str = #name;
            const OWNER: Option<&'static str> = #owner;
            const SPACE: &'static str = #space;
            const ALL: u64 = #all;
            type View<'a> = #view<'a>;
            type Writer<'w> = #writer<'w>;

            fn fields() -> Vec<#m::FieldSpec> {
                vec![#(#specs),*]
            }

            fn read(row: &#m::Row<'_, Self>) -> Self {
                #ident { #(#names: row.field(#indices)),* }
            }

            fn write(&self, row: &mut #m::RowWriter<'_, Self>) {
                #(row.set(#indices, || self.#names);)*
            }
        }
    }
}

/// `ThreatView`: a getter a field.
fn view(it: &Item) -> TokenStream {
    let (m, vis, ident) = (api(), it.vis, it.ident);
    let view = format_ident!("{ident}View");
    let getters = it.fields.iter().enumerate().map(|(i, f)| {
        let (n, ty) = (&f.ident, &f.ty);
        quote! { #vis fn #n(&self) -> #ty { self.0.field(#i) } }
    });
    quote! {
        /// A row of the mirror, read in place.
        #vis struct #view<'a>(#m::Row<'a, #ident>);

        impl<'a> From<#m::Row<'a, #ident>> for #view<'a> {
            fn from(row: #m::Row<'a, #ident>) -> Self {
                #view(row)
            }
        }

        impl #view<'_> {
            /// Its row in the key space.
            #vis fn row(&self) -> u32 {
                self.0.row()
            }

            /// Its key.
            #vis fn key(&self) -> Option<u64> {
                self.0.key()
            }

            #(#getters)*

            /// The whole row (a field not asked for reads as its default).
            #vis fn get(&self) -> #ident {
                <#ident as #m::Mirror>::read(&self.0)
            }
        }
    }
}

/// `ThreatWriter`: a setter a field, its value given by a closure called
/// only when the field is due.
fn writer(it: &Item) -> TokenStream {
    let (m, vis, ident) = (api(), it.vis, it.ident);
    let writer = format_ident!("{ident}Writer");
    let setters = it.fields.iter().enumerate().map(|(i, f)| {
        let (n, ty) = (&f.ident, &f.ty);
        quote! {
            /// Sets it to what `value` gives — called only when it's due.
            #vis fn #n(mut self, value: impl FnOnce() -> #ty) -> Self {
                self.0.set(#i, value);
                self
            }
        }
    });
    quote! {
        /// A row of the mirror being written.
        #vis struct #writer<'w>(#m::RowWriter<'w, #ident>);

        impl<'w> From<#m::RowWriter<'w, #ident>> for #writer<'w> {
            fn from(row: #m::RowWriter<'w, #ident>) -> Self {
                #writer(row)
            }
        }

        impl #writer<'_> {
            #(#setters)*

            /// Sets every due field from `value`.
            #vis fn put(mut self, value: &#ident) -> Self {
                <#ident as #m::Mirror>::write(value, &mut self.0);
                self
            }
        }
    }
}

//! The nanites a mod declares on a `fn(&mut Hook)` (`docs/design/nanites.md`),
//! made by the host before the game's classes load, the function their
//! handler: `#[inject]`, `#[redirect]` (and `#[wrap]`, the same with the
//! original to call), `#[modify_arg]`, `#[modify_variable]`,
//! `#[modify_constant]`, and `#[modify_return]` / `#[overwrite]` (an inject
//! at RETURN / HEAD that answers). Their keys: the member (a `#[nanite]`
//! impl's const) first, then `at` (`yog_ids::mixin_at`), `target` (what an
//! INVOKE/FIELD/NEW at names), `ordinal`, `shift` (`yog_ids::mixin_shift`),
//! `cancellable`, `priority`, `index` (an argument, a local), `constant`
//! (`"int:5"`), `ty` (a local's descriptor).

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

const AT: &str = "at";
const TARGET: &str = "target";
const ORDINAL: &str = "ordinal";
const SHIFT: &str = "shift";
const CANCELLABLE: &str = "cancellable";
const PRIORITY: &str = "priority";
const INDEX: &str = "index";
const CONSTANT: &str = "constant";
const TY: &str = "ty";
/// Mixin's defaults: every occurrence, no shift; no index (by type).
const EVERY: i32 = -1;
const NO_SHIFT: &str = "NONE";
const NO_INDEX: i32 = -1;

/// A nanite macro: its kind (`yog_ids::nanite_kind`'s const), its `at`
/// unless one is given, whether it answers (cancellable, unless it says).
pub(crate) struct Kind {
    pub kind: &'static str,
    pub at: &'static str,
    pub cancellable: bool,
}

struct Args {
    member: syn::Path,
    index: i32,
    constant: String,
    ty: String,
    at: syn::Ident,
    target: String,
    ordinal: i32,
    shift: syn::Ident,
    cancellable: bool,
    priority: i32,
}

fn args_of(attr: TokenStream, kind: &Kind) -> syn::Result<Args> {
    let mut it = attr.into_iter().peekable();
    let mut path = TokenStream::new();
    while let Some(t) = it.peek() {
        if matches!(t, proc_macro2::TokenTree::Punct(p) if p.as_char() == ',') {
            it.next();
            break;
        }
        path.extend(it.next());
    }
    let member: syn::Path = syn::parse2(path)?;
    let mut args = Args {
        member,
        index: NO_INDEX,
        constant: String::new(),
        ty: String::new(),
        at: format_ident!("{}", kind.at),
        target: String::new(),
        ordinal: EVERY,
        shift: format_ident!("{NO_SHIFT}"),
        cancellable: kind.cancellable,
        priority: 0,
    };
    let rest: TokenStream = it.collect();
    let parser = syn::meta::parser(|m| {
        if m.path.is_ident(AT) {
            args.at = m.value()?.parse()?;
        } else if m.path.is_ident(TARGET) {
            args.target = m.value()?.parse::<syn::LitStr>()?.value();
        } else if m.path.is_ident(ORDINAL) {
            args.ordinal = m.value()?.parse::<syn::LitInt>()?.base10_parse()?;
        } else if m.path.is_ident(SHIFT) {
            args.shift = m.value()?.parse()?;
        } else if m.path.is_ident(CANCELLABLE) {
            args.cancellable = true;
        } else if m.path.is_ident(PRIORITY) {
            args.priority = m.value()?.parse::<syn::LitInt>()?.base10_parse()?;
        } else if m.path.is_ident(INDEX) {
            args.index = m.value()?.parse::<syn::LitInt>()?.base10_parse()?;
        } else if m.path.is_ident(CONSTANT) {
            args.constant = m.value()?.parse::<syn::LitStr>()?.value();
        } else if m.path.is_ident(TY) {
            args.ty = m.value()?.parse::<syn::LitStr>()?.value();
        } else {
            return Err(m.error(
                "expected at, target, ordinal, shift, cancellable, priority, index, constant or ty",
            ));
        }
        Ok(())
    });
    syn::parse::Parser::parse2(parser, rest)?;
    Ok(args)
}

pub(crate) fn expand(
    kind: &Kind,
    attr: TokenStream,
    item: TokenStream,
) -> syn::Result<TokenStream> {
    let a = args_of(attr, kind)?;
    let f: syn::ItemFn = syn::parse2(item)?;
    let name = &f.sig.ident;
    let ctor = format_ident!("__yog_inject_{}", name);
    let n = quote! { ::yog_api::nanite };
    let ids = quote! { ::yog_api::yog_ids };
    let nanite_kind = format_ident!("{}", kind.kind);
    let Args {
        member,
        index,
        constant,
        ty,
        at,
        target,
        ordinal,
        shift,
        cancellable,
        priority,
    } = a;
    Ok(quote! {
        #f

        // Runs when this mod's library loads: what the runtime asks before the game does.
        #[::yog_api::ctor::ctor(unsafe, crate_path = ::yog_api::ctor)]
        #[doc(hidden)]
        fn #ctor() {
            #n::__yog_nanite_registry().lock().push(#n::InjectEntry {
                kind: #ids::nanite_kind::#nanite_kind,
                member: #member,
                index: #index,
                constant: #constant,
                value_desc: #ty,
                at: #ids::mixin_at::#at,
                target: #target,
                ordinal: #ordinal,
                shift: #ids::mixin_shift::#shift,
                cancellable: #cancellable,
                priority: #priority,
                handler: #name,
            });
        }
    })
}

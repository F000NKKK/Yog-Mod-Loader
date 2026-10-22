//! `#[inject(LivingEntity::HURT, at = HEAD, cancellable)]` on a
//! `fn(&mut Hook)` (`docs/design/nanites.md`): an injection the host makes
//! before the game's classes load, the function its handler. Its keys: the
//! member (a `#[nanite]` impl's const), `at` (`yog_ids::mixin_at`), `target`
//! (what an INVOKE/FIELD/NEW at names), `ordinal`, `shift`
//! (`yog_ids::mixin_shift`), `cancellable`, `priority`.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

const AT: &str = "at";
const TARGET: &str = "target";
const ORDINAL: &str = "ordinal";
const SHIFT: &str = "shift";
const CANCELLABLE: &str = "cancellable";
const PRIORITY: &str = "priority";
/// Mixin's defaults: every occurrence, no shift, at the method's start.
const EVERY: i32 = -1;
const NO_SHIFT: &str = "NONE";
const START: &str = "HEAD";

struct Args {
    member: syn::Path,
    at: syn::Ident,
    target: String,
    ordinal: i32,
    shift: syn::Ident,
    cancellable: bool,
    priority: i32,
}

fn args_of(attr: TokenStream) -> syn::Result<Args> {
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
        at: format_ident!("{START}"),
        target: String::new(),
        ordinal: EVERY,
        shift: format_ident!("{NO_SHIFT}"),
        cancellable: false,
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
        } else {
            return Err(m.error("expected at, target, ordinal, shift, cancellable or priority"));
        }
        Ok(())
    });
    syn::parse::Parser::parse2(parser, rest)?;
    Ok(args)
}

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let a = args_of(attr)?;
    let f: syn::ItemFn = syn::parse2(item)?;
    let name = &f.sig.ident;
    let ctor = format_ident!("__yog_inject_{}", name);
    let n = quote! { ::yog_api::nanite };
    let ids = quote! { ::yog_api::yog_ids };
    let Args {
        member,
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
                member: #member,
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

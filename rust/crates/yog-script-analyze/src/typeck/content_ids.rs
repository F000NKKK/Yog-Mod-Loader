//! Content's ids, typed by kind (`ItemId`, `BlockId`, `SoundGroupId`, …
//! one each kind of `yog_ids::kind`, a tag aside): each a
//! `ContentId<TSelf>` of itself (`Yog`'s, its members by `[Host]`), its
//! value its text. `new ItemId("minecraft:stick")` makes one, its id
//! checked when it's written out. A tag's id is a `TagId<TKind>` of the
//! id it tags (`TagId<ItemId>`), a `ContentId` too.

use yog_ids::script_problem;
use yog_script_ids::system::{CONTENT_ID, CONTENT_ID_SUFFIX, TAG_ID};

use super::Checker;
use super::model::Type;
use crate::ast::{Arg, Expr, ExprKind, StrSeg};

/// The id type of content kind `kind`: `item` → `ItemId`.
pub fn content_id_name(kind: &str) -> String {
    format!("{}{CONTENT_ID_SUFFIX}", yog_codegen::pascal(kind))
}

/// The tag id type of content kind `kind`: `item` → `TagId<ItemId>`;
/// `TagId` of no one kind (`""`).
pub fn tag_id_name(kind: &str) -> String {
    if kind.is_empty() {
        return TAG_ID.to_string();
    }
    format!("{TAG_ID}<{}>", content_id_name(kind))
}

/// `TagId<T>` written: `TagId<ItemId>` is a tag of items; one of a type
/// parameter stays what generic code takes.
pub(super) fn tag_id_of(args: Vec<Type>) -> Type {
    match args.as_slice() {
        [Type::Content(k)] => Type::Tag(k.clone()),
        _ => Type::Own(TAG_ID.to_string(), args),
    }
}

/// Whether `got` (a content's or a tag's id) goes where `wanted` is: a
/// `ContentId<T>`, a `TagId<T>` of generic code, a tag of no one kind.
pub(super) fn id_converts(got: &Type, wanted: &Type) -> bool {
    let generic = |of: &str, ys: &[Type]| ys.len() == 1 && (of == CONTENT_ID || of == TAG_ID);
    match (got, wanted) {
        (Type::Content(_), Type::Own(b, ys)) => b == CONTENT_ID && ys.len() == 1,
        (Type::Tag(_), Type::Own(b, ys)) => generic(b, ys),
        (Type::Tag(a), Type::Tag(b)) => a.is_empty() || b.is_empty(),
        _ => false,
    }
}

/// The content kind id type `name` is of (`ItemId` → `item`).
pub(super) fn content_id_kind(name: &str) -> Option<&'static str> {
    yog_ids::kind::ALL_VALUES
        .iter()
        .copied()
        .filter(|k| *k != yog_ids::kind::TAG)
        .find(|k| content_id_name(k) == name)
}

/// `ContentId<T>` written: `ContentId<ItemId>` is an `ItemId`; one of a
/// type parameter stays what generic code takes.
pub(super) fn content_id_of(args: Vec<Type>) -> Type {
    match args.as_slice() {
        [Type::Content(k)] => Type::Content(k.clone()),
        _ => Type::Own(CONTENT_ID.to_string(), args),
    }
}

impl Checker<'_, '_, '_> {
    /// `new TagId<ItemId>(args)` (`e`): its text, an id when it's written out.
    pub(super) fn new_tag_id(&mut self, e: &Expr, kind: &str, args: &[Arg]) {
        let Some(text) = self.id_text(e, &tag_id_name(kind), args) else {
            return;
        };
        if let Some(why) = bad_syntax(&text.0) {
            self.report
                .error(script_problem::BAD_REFERENCE, text.1, why);
        }
    }

    /// `new ItemId(args)` (`e`): its text, an id when it's written out.
    pub(super) fn new_content_id(&mut self, e: &Expr, kind: &str, args: &[Arg]) {
        let Some((id, at)) = self.id_text(e, &content_id_name(kind), args) else {
            return;
        };
        if let Some(why) = self.bad_id(kind, &id) {
            self.report.error(script_problem::BAD_REFERENCE, at, why);
        }
    }

    /// The text an id (of type `named`) is made of, when it's written out
    /// (and where); its one argument checked as a `string`.
    fn id_text(
        &mut self,
        e: &Expr,
        named: &str,
        args: &[Arg],
    ) -> Option<(String, crate::source::token::Span)> {
        let [arg] = args else {
            self.report.error(
                script_problem::WRONG_ARGS,
                e.span,
                format!("a `{named}` is made of its id: `new {named}(\"minecraft:stick\")`"),
            );
            return None;
        };
        self.fits(&arg.value, &Type::String);
        written_text(&arg.value).map(|t| (t, arg.value.span))
    }

    /// What's wrong with id `id` of kind `kind`, if anything: not an id
    /// (`namespace:path`, the game's when it has no namespace), or one of
    /// the mod's own namespace naming none of its content.
    fn bad_id(&self, kind: &str, id: &str) -> Option<String> {
        if let Some(why) = bad_syntax(id) {
            return Some(why);
        }
        let qualified = yog_ids::qualify(id);
        let ns = yog_ids::split(&qualified).0;
        let content = self.content.as_ref()?;
        let ours = content.iter().any(|(_, c)| yog_ids::split(c).0 == ns);
        let found = content.iter().any(|(k, c)| k == kind && *c == qualified);
        (ours && !found).then(|| format!("the mod has no {kind} `{qualified}`"))
    }
}

/// What's wrong with `id` as an id's text, if anything: `namespace:path`,
/// lowercase (the game's when it has no namespace).
fn bad_syntax(id: &str) -> Option<String> {
    let qualified = yog_ids::qualify(id);
    let (ns, path) = yog_ids::split(&qualified);
    let fits = |s: &str, extra: &[char]| {
        !s.is_empty()
            && s.chars().all(|c| {
                c.is_ascii_lowercase()
                    || c.is_ascii_digit()
                    || "_-.".contains(c)
                    || extra.contains(&c)
            })
    };
    (!fits(ns, &[]) || !fits(path, &['/']))
        .then(|| format!("`{id}` isn't an id: `namespace:path`, lowercase (`minecraft:stick`)"))
}

/// A string's text as written out (no holes), if it is one.
fn written_text(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::RawStr(s) => Some(s.clone()),
        ExprKind::Str(segs) => match segs.as_slice() {
            [StrSeg::Text(t)] => Some(t.clone()),
            [] => Some(String::new()),
            _ => None,
        },
        _ => None,
    }
}

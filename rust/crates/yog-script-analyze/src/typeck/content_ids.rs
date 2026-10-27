//! Content's ids, typed by kind (`ItemId`, `BlockId`, `SoundGroupId`, …
//! one each kind of `yog_ids::kind`, a tag aside — `#…` is a tag): each
//! a `ContentId<TSelf>` of itself (`Yog`'s, its members by `[Host]`), its
//! value its text. `new ItemId("minecraft:stick")` makes one, its id
//! checked when it's written out.

use yog_ids::script_problem;
use yog_script_ids::system::{CONTENT_ID, CONTENT_ID_SUFFIX};

use super::Checker;
use super::model::Type;
use crate::ast::{Arg, Expr, ExprKind, StrSeg};

/// The id type of content kind `kind`: `item` → `ItemId`.
pub fn content_id_name(kind: &str) -> String {
    format!("{}{CONTENT_ID_SUFFIX}", yog_codegen::pascal(kind))
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
    /// `new ItemId(args)` (`e`): its text, an id when it's written out.
    pub(super) fn new_content_id(&mut self, e: &Expr, kind: &str, args: &[Arg]) {
        let [arg] = args else {
            self.report.error(
                script_problem::WRONG_ARGS,
                e.span,
                format!("a `{}` is made of its id: `new {}(\"minecraft:stick\")`", content_id_name(kind), content_id_name(kind)),
            );
            return;
        };
        self.fits(&arg.value, &Type::String);
        let Some(id) = written_text(&arg.value) else {
            return;
        };
        if let Some(why) = self.bad_id(kind, &id) {
            self.report.error(script_problem::BAD_REFERENCE, arg.value.span, why);
        }
    }

    /// What's wrong with id `id` of kind `kind`, if anything: not an id
    /// (`namespace:path`, the game's when it has no namespace), or one of
    /// the mod's own namespace naming none of its content.
    fn bad_id(&self, kind: &str, id: &str) -> Option<String> {
        let qualified = yog_ids::qualify(id);
        let (ns, path) = yog_ids::split(&qualified);
        let fits = |s: &str, extra: &[char]| {
            !s.is_empty()
                && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-.".contains(c) || extra.contains(&c))
        };
        if !fits(ns, &[]) || !fits(path, &['/']) {
            return Some(format!("`{id}` isn't an id: `namespace:path`, lowercase (`minecraft:stick`)"));
        }
        let content = self.content.as_ref()?;
        let ours = content.iter().any(|(_, c)| yog_ids::split(c).0 == ns);
        let found = content.iter().any(|(k, c)| k == kind && *c == qualified);
        (ours && !found).then(|| format!("the mod has no {kind} `{qualified}`"))
    }
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

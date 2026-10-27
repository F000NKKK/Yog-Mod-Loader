//! `saved` and `synced` members (docs/design/storage.md S3): state the
//! world keeps or clients mirror is Yog Data — numbers, `bool`, `string`,
//! `Text`, quantities, ids, enums, records, lists and dictionaries of
//! those, a player (`Player?`, by its uuid) — and it has an owner (a
//! game's object an `extend` adds to, a content object, a component, the
//! mod), never a plain class's.

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use crate::ast::{Decl, DeclKind, Expr, ExprKind, Modifier};
use crate::index::Side;
use yog_script_ids::system::{DICTIONARY, LIST, TEXT_TYPE};

impl Checker<'_, '_, '_> {
    /// Member `d` of the type being checked, when it's `saved` or `synced`.
    pub(super) fn state_member(&mut self, d: &Decl) {
        if let DeclKind::Field(f) = &d.kind {
            self.was(d, f);
        }
        if !d.has(Modifier::Saved) && !d.has(Modifier::Synced) {
            return;
        }
        let (ty, name) = match &d.kind {
            DeclKind::Field(f) => (&f.ty, &f.name),
            DeclKind::Property { ty, name, .. } => (ty, name),
            _ => return,
        };
        if !self.state_owner {
            self.report.error(
                script_problem::STATE_OWNER,
                name.span,
                format!("`{}` keeps state with no owner: it goes on a game's object (`extend Player`), content, a component or the mod", name.text),
            );
        }
        let t = self.quietly(|c| c.type_of(ty));
        if t.known() && !self.data_able(&t, &mut Vec::new()) {
            self.report.error(
                script_problem::STATE_TYPE,
                ty.span,
                format!("a `{t}` isn't kept: a saved or synced member is Yog Data — numbers, `bool`, `string`, `Text`, quantities, ids, enums, records, lists and dictionaries of those, a player"),
            );
        }
    }

    /// Field `f`'s `was` (storage.md S5): what an old save called it, read
    /// as it is now. It needs a save (`saved`), and a type the member reads
    /// as its own: the same, or a number or quantity for one.
    fn was(&mut self, d: &Decl, f: &crate::ast::Field) {
        let Some((old, old_ty)) = &f.was else {
            return;
        };
        if !d.has(Modifier::Saved) {
            self.report.error(
                script_problem::STATE_WAS,
                old.span,
                format!(
                    "`{}` has a `was` but isn't `saved`: only a save has old names",
                    f.name.text
                ),
            );
            return;
        }
        let Some(old_ty) = old_ty else {
            return;
        };
        let (now, before) = (
            self.quietly(|c| c.type_of(&f.ty)),
            self.quietly(|c| c.type_of(old_ty)),
        );
        if now.known() && before.known() && !was_reads(&before, &now) {
            self.report.error(
                script_problem::STATE_WAS,
                old_ty.span,
                format!("an old `{before}` isn't read as a `{now}`: a `was` keeps the type, or goes number to number"),
            );
        }
    }

    /// Whether a value of `t` is Yog Data a store keeps (`seen`: the
    /// records looked into, a record holding itself not looped).
    fn data_able(&mut self, t: &Type, seen: &mut Vec<String>) -> bool {
        match t {
            Type::Bool
            | Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::F32
            | Type::F64
            | Type::String
            | Type::Unit(_)
            | Type::Content(_)
            | Type::Tag(_) => true,
            Type::Nullable(inner) | Type::Array(inner, 1) => self.data_able(inner, seen),
            // A handle: a player, an entity, a world, by its subject.
            Type::Yog(name) => crate::yog::is_handle(name),
            Type::Own(name, args) if name == LIST && args.len() == 1 => {
                self.data_able(&args[0], seen)
            }
            Type::Own(name, args) if name == DICTIONARY && args.len() == 2 => {
                self.data_able(&args[0], seen) && self.data_able(&args[1], seen)
            }
            Type::Own(name, args) if name == TEXT_TYPE && args.is_empty() => true,
            Type::Own(name, args) if args.is_empty() => self.own_data_able(name, seen),
            _ => false,
        }
    }

    /// Own type `name`: an enum (without data), or a record whose fields are.
    fn own_data_able(&mut self, name: &str, seen: &mut Vec<String>) -> bool {
        let Some(own) = self.own.get(name, 0) else {
            return false;
        };
        if !(own.is_enum || own.record) || seen.iter().any(|s| s == name) {
            return own.is_enum || own.record;
        }
        seen.push(name.to_string());
        // A record's fields; an enum's cases' data.
        let fields: Vec<_> = own
            .ctor
            .iter()
            .chain(own.variants.values().flatten())
            .map(|p| p.ty.clone())
            .collect();
        fields.iter().all(|f| {
            let t = self.quietly(|c| c.type_of(f));
            self.data_able(&t, seen)
        })
    }

    /// `place` written: a `synced` member is the server's — client code
    /// reads it, never changes it (storage.md §4).
    pub(super) fn synced_written(&mut self, place: &Expr) {
        if self.side != Side::Client {
            return;
        }
        let (owner, name) = match &place.kind {
            ExprKind::Member { expr, name, .. } => {
                (self.types.of(expr).cloned(), name.text.as_str())
            }
            ExprKind::Ident(name) if self.local(name).is_none() => {
                (self.this.clone(), name.as_str())
            }
            _ => return,
        };
        if owner.is_some_and(|t| self.is_synced(&t, name)) {
            self.report.error(
                script_problem::WRONG_SIDE,
                place.span,
                format!("`{name}` is synced: the server changes it, a client reads it"),
            );
        }
    }

    /// Whether member `name` of a `t` is `synced` (its own type's, or an
    /// `extend`'s of the game's object).
    fn is_synced(&self, t: &Type, name: &str) -> bool {
        let own = match t.non_null() {
            Type::Own(n, args) => self.own.get(n, args.len()),
            Type::Yog(n) | Type::Game(n) => self.own.extended.get(crate::index::simple_name(n)),
            _ => None,
        };
        own.is_some_and(|o| o.synced.iter().any(|s| s == name))
    }
}

/// Whether an old save's `before` is read as a member's `now`: as it is,
/// or one number or quantity as another (a cast).
fn was_reads(before: &Type, now: &Type) -> bool {
    let numeric = |t: &Type| super::builtin::is_number(t) || matches!(t, Type::Unit(_));
    before.converts_to(now) || (numeric(before) && numeric(now))
}

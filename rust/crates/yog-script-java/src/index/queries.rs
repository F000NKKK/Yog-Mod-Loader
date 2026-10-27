//! What the index answers of a type: its namespace, kind, events,
//! units, records' parts, statics, which members are called.

use std::collections::HashSet;

use super::{Index, Kind};
use crate::java_names::TypeRef;

impl Index {
    /// Enum `name`'s cases carrying data: each its name, its fields' names
    /// and types (as written).
    pub(crate) fn data_cases(&self, name: &str) -> Vec<(&str, &[String], &[yog_script_analyze::ast::Type])> {
        self.cases
            .iter()
            .filter(|((e, _), fields)| e == name && !fields.is_empty())
            .map(|((e, c), fields)| {
                let types = self.case_types.get(&(e.clone(), c.clone())).map_or(&[][..], Vec::as_slice);
                (c.as_str(), fields.as_slice(), types)
            })
            .collect()
    }

    /// Record `name`'s parameters' types, as written (none: not a record).
    pub(crate) fn record_types(&self, name: &str) -> &[yog_script_analyze::ast::Type] {
        self.record_types.get(name).map_or(&[], Vec::as_slice)
    }

    /// Type `name`'s namespace (empty when it's in none).
    pub fn namespace_of(&self, name: &str) -> &str {
        self.namespaces.get(name).map_or("", String::as_str)
    }

    /// Whether type `name` is a `[Host]` one: a built-in type's members.
    pub fn is_host(&self, name: &str) -> bool {
        self.hosts.contains(name)
    }

    /// Whether type `name` is the runtime's, written by hand.
    pub fn is_intrinsic(&self, name: &str) -> bool {
        self.intrinsics.contains(name)
    }

    /// Event `name` of type `owner` (the file's or a mod's: none): its bus
    /// address, when it's on the bus; `None` when there's no such event.
    pub fn event(&self, owner: Option<&str>, name: &str) -> Option<Option<&str>> {
        let owner = owner.unwrap_or_default();
        if let Some(found) = self.events.get(&(owner.to_string(), name.to_string())) {
            return Some(found.as_deref());
        }
        // What it inherits declares it (a block's `Use`: `BlockObject`'s).
        let mut seen = HashSet::new();
        self.types
            .keys()
            .filter(|(n, _)| n == owner)
            .find_map(|t| self.inherited_event(t, name, &mut seen))
    }

    /// Event `name` declared by a type `t` inherits (each type looked in once).
    fn inherited_event(
        &self,
        t: &TypeRef,
        name: &str,
        seen: &mut HashSet<TypeRef>,
    ) -> Option<Option<&str>> {
        if !seen.insert(t.clone()) {
            return None;
        }
        for base in &self.types.get(t)?.bases {
            if let Some(found) = self.events.get(&(base.0.clone(), name.to_string())) {
                return Some(found.as_deref());
            }
            if let Some(found) = self.inherited_event(base, name, seen) {
                return Some(found);
            }
        }
        None
    }

    /// Whether `name` is a unit: the game's (`Ticks`) or one declared.
    pub fn is_unit(&self, name: &str) -> bool {
        yog_script_ids::units::GAME_UNITS.contains(&name) || self.units.contains(name)
    }

    /// Whether type `name` (of no type parameters: an attribute) is made with arguments.
    pub fn takes_args(&self, name: &str) -> bool {
        self.types
            .get(&(name.to_string(), 0))
            .is_some_and(|i| i.takes_args)
    }

    /// Record `t`'s constructor's parameters, in order (none for another type).
    pub fn record_params(&self, t: &TypeRef) -> &[String] {
        self.types
            .get(t)
            .filter(|i| i.kind == Kind::Record)
            .map_or(&[], |i| i.record_params.as_slice())
    }

    /// Record `t`'s components, in order: its parameters, then its own fields.
    pub fn record_components(&self, t: &TypeRef) -> Vec<&str> {
        self.types
            .get(t)
            .filter(|i| i.kind == Kind::Record)
            .map_or_else(Vec::new, |i| {
                i.record_params
                    .iter()
                    .chain(&i.record_fields)
                    .map(String::as_str)
                    .collect()
            })
    }

    /// Whether type `t` is made by `new` and keeps the type arguments it's
    /// made with (`Typed`): a generic class or struct, not abstract — the
    /// compiler's, or one the runtime writes by hand.
    pub fn keeps_types(&self, t: &TypeRef) -> bool {
        t.1 > 0
            && self
                .types
                .get(t)
                .is_some_and(|i| matches!(i.kind, Kind::Class | Kind::Struct) && !i.is_abstract)
    }

    /// Whether `t` is one of the project's own generic classes or structs:
    /// made with its type arguments handed to its constructor (`T` known
    /// there, as C#'s), not set on it after.
    pub fn takes_types(&self, t: &TypeRef) -> bool {
        self.keeps_types(t) && self.own.contains(t)
    }

    /// Whether `name` is a static method of own type `t`.
    pub fn is_static(&self, t: &TypeRef, name: &str) -> bool {
        self.statics.contains(&(t.clone(), name.to_string()))
    }

    /// Whether `name` is a static method of `t` that takes `t`'s type
    /// arguments first: one of an own generic type.
    pub fn static_taking_types(&self, t: &TypeRef, name: &str) -> bool {
        self.takes_types(t) && self.statics.contains(&(t.clone(), name.to_string()))
    }

    /// Whether `name` is a static field of `t` kept per list of its
    /// arguments (C#'s: `Box<int>.Count` isn't `Box<string>.Count`): one of
    /// an own generic type's.
    pub fn static_per_type(&self, t: &TypeRef, name: &str) -> bool {
        self.takes_types(t) && self.static_fields.contains(&(t.clone(), name.to_string()))
    }

    /// Whether `t` is, implements or inherits `interface` (by name).
    pub fn implements(&self, t: &TypeRef, interface: &str) -> bool {
        if t.0 == interface {
            return true;
        }
        self.types
            .get(t)
            .is_some_and(|i| i.bases.iter().any(|b| self.implements(b, interface)))
    }

    pub fn kind(&self, t: &TypeRef) -> Option<Kind> {
        self.types.get(t).map(|i| i.kind)
    }

    /// Type `name` written without its arguments (`Math`, a case's enum):
    /// the one of the fewest, as `place_named` finds it.
    pub fn named(&self, name: &str) -> TypeRef {
        let arity = self
            .arities
            .get(name)
            .and_then(|a| a.first().copied())
            .unwrap_or_default();
        (name.to_string(), arity)
    }

    /// Whether `name` is a computed property of type `of` (or of what it
    /// inherits): read by calling its getter. A record's component is its
    /// field inside it; a name no member is (a local) isn't.
    pub fn member_called(&self, of: &TypeRef, name: &str) -> bool {
        self.types.get(of).is_some_and(|i| {
            i.computed.contains(name) || i.bases.iter().any(|b| self.member_called(b, name))
        })
    }

    /// Whether `name` of type `of` is read by calling it (a computed
    /// property, a record's field).
    pub fn called(&self, of: &TypeRef, name: &str) -> bool {
        self.types.get(of).is_some_and(|i| {
            i.kind == Kind::Record
                || i.computed.contains(name)
                || i.bases.iter().any(|b| self.called(b, name))
        })
    }
}

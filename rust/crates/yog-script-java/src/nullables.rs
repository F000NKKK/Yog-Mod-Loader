//! A value type's `T?` on the JVM: its box, `null` for none. Its members
//! (`System.Nullable<T>`'s) read the box: `HasValue` whether there's one,
//! `Value` it, `GetValueOrDefault` it or a default.

use yog_script_analyze::Type;
use yog_script_ids::java::{NULL, VAR};
use yog_script_ids::jvm::{OBJECTS, REQUIRE_NON_NULL};
use yog_script_ids::members::{GET_VALUE_OR_DEFAULT, HAS_VALUE, VALUE};
use yog_script_ids::system::NULLABLE;

use crate::Gen;
use crate::exprs::zero;

impl Gen<'_> {
    /// Member `name` of `target`, of a value type's `T?` (`on_type`), read.
    pub(crate) fn nullable_member(
        &mut self,
        target: &str,
        on_type: Option<&Type>,
        name: &str,
    ) -> Option<String> {
        let Some(Type::Nullable(inner)) = on_type else {
            return None;
        };
        if crate::types::primitive(inner).is_none() && !self.is_struct(inner) {
            return None;
        }
        match name {
            HAS_VALUE => Some(format!("({target} != {NULL})")),
            VALUE => Some(format!("{OBJECTS}.{REQUIRE_NON_NULL}({target})")),
            _ => None,
        }
    }

    /// Call `name(args)` of `target` (of owner `owner`): a `T?`'s
    /// `GetValueOrDefault`, its value else the default given or `T`'s.
    pub(crate) fn nullable_call(
        &mut self,
        (owner, name): (Option<&str>, &str),
        target: &str,
        args: &str,
        gives: Option<&Type>,
    ) -> Option<String> {
        if owner != Some(NULLABLE) || name != GET_VALUE_OR_DEFAULT {
            return None;
        }
        let held = self.temp();
        self.code.line(format!("{VAR} {held} = {target};"));
        let fallback = if args.is_empty() {
            gives.map_or(NULL, zero).to_string()
        } else {
            args.to_string()
        };
        Some(format!("({held} == {NULL} ? {fallback} : {held})"))
    }

    /// Whether `t` is one of the project's structs.
    fn is_struct(&self, t: &Type) -> bool {
        matches!(t, Type::Own(name, args) if self.index.kind(&(name.clone(), args.len())) == Some(crate::index::Kind::Struct))
    }
}

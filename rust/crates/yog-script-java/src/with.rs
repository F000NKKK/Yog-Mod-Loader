//! `x with { Y = 64 }`, computed before its statement: its values in the
//! order written, then a record made again of its components (each given
//! one or `x`'s), or a struct's copy, the members given set on it.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Expr, Name};
use yog_script_ids::java::{NEW, VAR};

use super::Gen;
use crate::index::Kind;
use crate::stmts::setter;

impl Gen<'_> {
    /// `v` (of type `t`) held for what comes after: a local, or in a
    /// machine a field (an `await` may come between).
    fn hold(&mut self, t: Option<Type>, v: String) -> String {
        if self.machine.is_some() {
            return self.field_of_type(t.as_ref(), &v);
        }
        let held = self.temp();
        self.code.line(format!("{VAR} {held} = {v};"));
        held
    }

    pub(crate) fn with(&mut self, e: &Expr, on: &Expr, inits: &[(Name, Expr)]) -> String {
        let Some(Type::Own(name, args)) = self.types.of(e).map(Type::non_null).cloned() else {
            self.not_yet("`with` on this", e.span);
            return String::new();
        };
        let of = (name, args.len());
        // A struct handed on is a copy already: a name's copied, a call's new.
        let source = self.handed_expr(on);
        let struct_copy = self.index.kind(&of) == Some(Kind::Struct);
        let copied = self.hold(self.types.of(e).cloned(), source);
        let mut given = Vec::new();
        for (member, value) in inits {
            let v = self.handed_expr(value);
            let held = self.hold(self.types.of(value).cloned(), v);
            given.push((member.text.as_str(), held));
        }
        if struct_copy {
            for (member, held) in &given {
                let n = Self::ident(member);
                let set = if self.index.called(&of, member) {
                    format!("{copied}.{}({held});", setter(&n))
                } else {
                    format!("{copied}.{n} = {held};")
                };
                self.code.line(set);
            }
            return copied;
        }
        let components: Vec<String> = self
            .index
            .record_components(&of)
            .into_iter()
            .map(|c| match given.iter().find(|(m, _)| *m == c) {
                Some((_, held)) => held.clone(),
                None => format!("{copied}.{}()", Self::ident(c)),
            })
            .collect();
        // A record keeps no type arguments: Java's, not `Typed`.
        let t = self.java(&Type::Own(of.0, args), true);
        format!("{NEW} {t}({})", components.join(", "))
    }
}

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
    pub(crate) fn with(&mut self, e: &Expr, on: &Expr, inits: &[(Name, Expr)]) -> String {
        let Some(Type::Own(name, args)) = self.types.of(e).map(Type::non_null).cloned() else {
            self.not_yet("`with` on this", e.span);
            return String::new();
        };
        let of = (name, args.len());
        // A struct handed on is a copy already: a name's copied, a call's new.
        let source = self.handed_expr(on);
        let copied = self.temp();
        let struct_copy = self.index.kind(&of) == Some(Kind::Struct);
        self.code.line(format!("{VAR} {copied} = {source};"));
        let mut given = Vec::new();
        for (member, value) in inits {
            let v = self.handed_expr(value);
            let held = self.temp();
            self.code.line(format!("{VAR} {held} = {v};"));
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

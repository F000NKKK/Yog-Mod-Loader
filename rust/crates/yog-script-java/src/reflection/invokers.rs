//! A method in its description: its name, what it is, its own type
//! parameters, what calls it (`(o, a) -> ((Box) o).Get((Integer) a[0])`, by
//! its Java name), its result and its parameters.

use yog_script_analyze::ast::{Decl, Method, Modifier, Pass};
use yog_script_ids::jvm::reflection::{METHOD, RETURNS};

use super::members::{ARGS, DESCRIPTION, OBJECT, Owner, member_traits};
use crate::Gen;

/// No caller: a method Reflection can't call yet.
const NONE: &str = "null";

impl Gen<'_> {
    /// Method `m` (declaration `d`) of `owner` described, when Reflection
    /// sees it: not an explicit implementation, not one taking `ref`/`out`
    /// (not compiled), not the bus's (`extern`).
    pub(super) fn method_described(&self, owner: &Owner, d: &Decl, m: &Method) -> Option<String> {
        let skipped = m.explicit.is_some()
            || d.has(Modifier::Extern)
            || m.params
                .iter()
                .any(|p| matches!(p.pass, Some(Pass::Ref | Pass::Out)));
        if skipped {
            return None;
        }
        let generics: Vec<String> = m
            .generics
            .iter()
            .filter(|g| !g.lifetime)
            .map(|g| g.name.text.clone())
            .collect();
        let names: Vec<String> = generics.iter().map(|g| self.code.str(g)).collect();
        let returns = match &m.ret {
            Some(t) => self.member_type(owner, t, &generics),
            None => self
                .value_of(&yog_script_analyze::Type::Void)
                .unwrap_or_default(),
        };
        Some(format!(
            "{DESCRIPTION}.{METHOD}({}, {}, new String[] {{ {} }}, {}).{RETURNS}({returns}){};",
            self.code.str(&m.name.text),
            member_traits(d),
            names.join(", "),
            self.invoker(owner, d, (m, &generics)),
            self.takes(owner, &m.params, &generics)
        ))
    }

    /// What calls `m`: `(o, a) -> …` giving its result, `null` for nothing.
    fn invoker(&self, owner: &Owner, d: &Decl, (m, generics): (&Method, &[String])) -> String {
        let owner_ref = (owner.name.to_string(), owner.arity);
        if self.index.is_reified(
            &owner_ref,
            &m.name.text,
            self.index.overload_of(&owner_ref, m),
        ) {
            return NONE.to_string();
        }
        let n = Self::ident(&self.index.java_name(&owner_ref, m));
        let args = self.args_cast(owner, &m.params, generics);
        let stat = d.has(Modifier::Static);
        let call = match &owner.host {
            Some((host, value)) if !stat => {
                let sep = if args.is_empty() { "" } else { ", " };
                format!("{host}.{n}(({value}) {OBJECT}{sep}{args})")
            }
            _ if stat => format!("{}.{n}({args})", owner.raw),
            _ => format!("(({}) {OBJECT}).{n}({args})", owner.raw),
        };
        if m.ret.is_none() {
            format!("({OBJECT}, {ARGS}) -> {{ {call}; return null; }}")
        } else {
            format!("({OBJECT}, {ARGS}) -> {call}")
        }
    }
}

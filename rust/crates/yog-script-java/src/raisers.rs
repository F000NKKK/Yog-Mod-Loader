//! An event raised by a loop over its handlers as they are
//! (`Event.handlers()`), in a helper of the file's class, one per
//! signature: its arguments computed once and nothing allocated per raise
//! — a lambda handed to each handler would be made every time, and would
//! compute the arguments again for each.

use yog_script_analyze::Type;
use yog_script_analyze::ast::Arg;
use yog_script_ids::clr::INVOKE;
use yog_script_ids::java::{FOR, INT, NULL, PRIVATE, RETURN, STATIC, VOID};
use yog_script_ids::jvm::{EVENT_CLASS, EVENT_HANDLERS, LIST, LIST_GET, LIST_SIZE, RAISER};

use crate::Gen;
use crate::reified::names_parameter;

/// A helper raising events of one delegate type with arguments of some
/// types, giving a value (boxed) or not.
pub(crate) struct Raiser {
    name: String,
    delegate: String,
    params: Vec<String>,
    gives: Option<String>,
}

impl Raiser {
    /// What tells two signatures apart.
    fn same(&self, delegate: &str, params: &[String], gives: &Option<String>) -> bool {
        self.delegate == delegate && self.params == params && self.gives == *gives
    }
}

impl Gen<'_> {
    /// The helper raising an event of `delegate` with `args`, giving
    /// `gives`: its name. None when a type names a type parameter or isn't
    /// known, or an argument is passed by reference or name, or the code
    /// is the library's (a file of a type each, with no class of the file's
    /// to hold it) — the event is raised the general way then.
    pub(crate) fn raiser(
        &mut self,
        delegate: &Type,
        args: &[Arg],
        gives: Option<&Type>,
    ) -> Option<String> {
        let plain = |t: &Type| t.known() && *t != Type::Null && !names_parameter(t);
        if !self.nested
            || !plain(delegate)
            || args.iter().any(|a| a.pass.is_some() || a.name.is_some())
        {
            return None;
        }
        let mut params = Vec::new();
        for a in args {
            let t = self.types.of(&a.value)?.clone();
            if !plain(&t) {
                return None;
            }
            params.push(self.java(&t, false));
        }
        let gives = match gives {
            Some(t) if !plain(t) => return None,
            Some(t) => Some(self.java(t, true)),
            None => None,
        };
        let delegate = self.java(delegate, false);
        if let Some(r) = self
            .raisers
            .iter()
            .find(|r| r.same(&delegate, &params, &gives))
        {
            return Some(r.name.clone());
        }
        let name = format!("{RAISER}{}", self.raisers.len() + 1);
        self.raisers.push(Raiser {
            name: name.clone(),
            delegate,
            params,
            gives,
        });
        Some(name)
    }

    /// The file's raisers, written in its class.
    pub(crate) fn raisers_code(&mut self) {
        for r in std::mem::take(&mut self.raisers) {
            let ret = r.gives.clone().unwrap_or_else(|| VOID.to_string());
            let (event, hs, out, i, n) = (
                self.temp(),
                self.temp(),
                self.temp(),
                self.temp(),
                self.temp(),
            );
            let names: Vec<String> = r.params.iter().map(|_| self.temp()).collect();
            let params = r
                .params
                .iter()
                .zip(&names)
                .map(|(t, p)| format!(", {t} {p}"))
                .collect::<String>();
            self.code.open(format!(
                "{PRIVATE} {STATIC} {ret} {}({EVENT_CLASS}<{}> {event}{params})",
                r.name, r.delegate
            ));
            self.code.line(format!(
                "{LIST}<{}> {hs} = {event}.{EVENT_HANDLERS}();",
                r.delegate
            ));
            let call = format!("{hs}.{LIST_GET}({i}).{INVOKE}({})", names.join(", "));
            let each = format!("{FOR} ({INT} {i} = 0, {n} = {hs}.{LIST_SIZE}(); {i} < {n}; {i}++)");
            if let Some(t) = &r.gives {
                self.code.line(format!("{t} {out} = {NULL};"));
                self.code.line(format!("{each} {out} = {call};"));
                self.code.line(format!("{RETURN} {out};"));
            } else {
                self.code.line(format!("{each} {call};"));
            }
            self.code.close("");
        }
    }
}

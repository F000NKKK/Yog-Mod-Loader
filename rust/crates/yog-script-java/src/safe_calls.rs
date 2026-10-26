//! `on?.M(args)`, as C#'s: `on` computed once, `M` called (its arguments
//! computed) only when it isn't `null`, the call's value `null` then. In a
//! machine whose arguments await: the call a state of its own, jumped past
//! for `null`.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Arg, Expr};
use yog_script_ids::java::{IF, NULL, VAR};

use crate::Gen;

impl Gen<'_> {
    pub(crate) fn safe_call(&mut self, e: &Expr, on: &Expr, name: &str, args: &[Arg]) -> String {
        let target = self.expr(on);
        let on_type = self.types.of(on).cloned();
        let gives = self.types.of(e).is_some_and(|t| *t != Type::Void);
        let held = if self.machine.is_some() {
            self.field_of_type(on_type.as_ref(), &target)
        } else {
            let t = self.temp();
            self.code.line(format!("{VAR} {t} = {target};"));
            t
        };
        if self.machine.is_some() && args.iter().any(|a| crate::machine::awaits(&a.value)) {
            return self.safe_call_split(e, (on, &held), name, args, gives);
        }
        let a = self.call_args(e, args);
        let call = self.method_call_on(e, on, held.clone(), name, &a);
        if gives {
            format!("({held} == {NULL} ? {NULL} : {call})")
        } else {
            self.code.line(format!("{IF} ({held} != {NULL}) {call};"));
            String::new()
        }
    }

    /// [`Self::safe_call`] whose arguments await, in the machine.
    fn safe_call_split(
        &mut self,
        e: &Expr,
        (on, held): (&Expr, &str),
        name: &str,
        args: &[Arg],
        gives: bool,
    ) -> String {
        let value = gives.then(|| self.branch_field(e));
        let past = self.new_state();
        let skipped = match &value {
            Some(v) => format!("{{ {v} = {NULL}; {} }}", Self::jump(past)),
            None => Self::jump(past),
        };
        self.code.line(format!("{IF} ({held} == {NULL}) {skipped}"));
        let a = self.call_args(e, args);
        let call = self.method_call_on(e, on, held.to_string(), name, &a);
        match &value {
            Some(v) => self.code.line(format!("{v} = {call};")),
            None => self.code.line(format!("{call};")),
        };
        self.state_starts(past);
        value.unwrap_or_default()
    }
}

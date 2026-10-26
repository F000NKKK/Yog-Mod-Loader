//! A `switch` whose arms await, in the machine: the arm taken found first
//! (its pattern and guard, which don't await), the names it binds kept in
//! fields; then each arm's value a state of its own, the others jumped
//! past, as C#'s machine does.

use yog_script_analyze::ast::{Arm, Expr, Pattern};
use yog_script_ids::java::{BREAK, IF, INT, THROW};
use yog_script_ids::jvm::{HELPERS, NO_ARM};

use super::awaits;
use crate::Gen;
use crate::types::RUNTIME;

/// What no arm taken is, before one is.
const NO_ARM_TAKEN: &str = "-1";

impl Gen<'_> {
    /// `on switch { arms }` (`e`) whose arms await: `None` when none does,
    /// or the code isn't a machine's.
    pub(crate) fn split_switch(&mut self, e: &Expr, on: &Expr, arms: &[Arm]) -> Option<String> {
        if self.machine.is_none() || !arms.iter().any(|a| awaits(&a.value)) {
            return None;
        }
        if let Some(g) = arms.iter().filter_map(|a| a.guard.as_ref()).find(|g| awaits(g)) {
            self.not_yet("an `await` in a `switch` arm's guard", g.span);
            return Some(String::new());
        }
        let result = self.branch_field(e);
        let matched = self.types.of(on).cloned();
        let v = self.expr(on);
        let value = self.field_of_type(matched.as_ref(), &v);
        let taken = self.temp();
        self.declare_field(INT, &taken);
        let kept = self.arm_taken(e, (&value, matched.as_ref()), &taken, arms);
        let past = self.new_state();
        for (i, arm) in arms.iter().enumerate() {
            let next = self.new_state();
            self.code
                .line(format!("{IF} ({taken} != {i}) {}", Self::jump(next)));
            let depth = self.renamed.len();
            self.renamed.extend(kept[i].iter().cloned());
            let out = self.handed_expr(&arm.value);
            self.renamed.truncate(depth);
            self.code.line(format!("{result} = {out};"));
            self.code.line(Self::jump(past));
            self.state_starts(next);
        }
        self.state_starts(past);
        Some(result)
    }

    /// Which of `arms` takes `value` (of `matched`), into field `taken`;
    /// the names each binds, kept in fields (name, field) by arm.
    fn arm_taken(
        &mut self,
        e: &Expr,
        (value, matched): (&str, Option<&yog_script_analyze::Type>),
        taken: &str,
        arms: &[Arm],
    ) -> Vec<Vec<(String, String)>> {
        let label = self.temp();
        self.code.line(format!("{taken} = {NO_ARM_TAKEN};"));
        self.code.open(format!("{label}:"));
        let mut kept = Vec::new();
        for (i, arm) in arms.iter().enumerate() {
            let depth = self.pattern_open(&arm.pattern, (value, matched));
            if let Some(g) = &arm.guard {
                let g = self.expr(g);
                self.code.open(format!("{IF} ({g})"));
            }
            let mut names = Vec::new();
            for n in bound_names(&arm.pattern) {
                let t = self.types.binding(n).cloned();
                let field = self.field_of_type(t.as_ref(), &Self::ident(&n.text));
                names.push((n.text.clone(), field));
            }
            kept.push(names);
            self.code.line(format!("{taken} = {i};"));
            self.code.line(format!("{BREAK} {label};"));
            if arm.guard.is_some() {
                self.code.close("");
            }
            for _ in 0..depth {
                self.code.close("");
            }
        }
        self.code.close("");
        self.code.line(format!(
            "{IF} ({taken} == {NO_ARM_TAKEN}) {THROW} {RUNTIME}.{HELPERS}.{NO_ARM}({value}, {});",
            self.at(e.span)
        ));
        kept
    }

    /// A field of type `t` (an object's where it isn't known) holding `v`.
    pub(crate) fn field_of_type(&mut self, t: Option<&yog_script_analyze::Type>, v: &str) -> String {
        let ty = match t.filter(|t| t.known()) {
            Some(t) => self.java(t, true),
            None => yog_script_ids::jvm::OBJECT.to_string(),
        };
        let field = self.temp();
        self.declare_field(&ty, &field);
        self.code.line(format!("{field} = {v};"));
        field
    }
}

/// The names pattern `p` binds, in order.
fn bound_names(p: &Pattern) -> Vec<&yog_script_analyze::ast::Name> {
    match p {
        Pattern::Let(n) => vec![n],
        Pattern::Variant { args, .. } | Pattern::Tuple(args) => {
            args.iter().flat_map(bound_names).collect()
        }
        Pattern::Property { fields, .. } => fields.iter().flat_map(|(_, f)| bound_names(f)).collect(),
        Pattern::And(a, b) | Pattern::Or(a, b) => {
            let mut all = bound_names(a);
            all.extend(bound_names(b));
            all
        }
        Pattern::Not(inner) => bound_names(inner),
        _ => Vec::new(),
    }
}

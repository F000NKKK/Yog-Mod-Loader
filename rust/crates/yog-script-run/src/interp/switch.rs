//! `switch (expr) { sections }`: the first section a label of takes the
//! value (`default`'s when none does) run; its `break` leaves the switch,
//! its `goto case v` runs the section of `v`, `goto default` `default`'s.

use yog_script_analyze::ast::{Expr, Pattern, Section};

use super::goto::Jump;
use super::{Interp, Run, Stop};
use crate::value::Value;

/// A section chosen, and the label that took the value (none: `default`).
type Chosen<'s> = Option<(&'s Section, Option<&'s Pattern>)>;

impl Interp<'_> {
    pub(super) fn switch_stmt(&mut self, expr: &Expr, sections: &[Section]) -> Run<()> {
        let mut v = self.eval(expr)?;
        let mut to_default = false;
        loop {
            let chosen = if to_default {
                default_of(sections)
            } else {
                self.section_for(sections, &v)?
                    .or_else(|| default_of(sections))
            };
            let Some((section, label)) = chosen else {
                return Ok(());
            };
            let ran = self.scoped(|i| {
                // Its label's names, bound again for its statements.
                if let Some(pattern) = label {
                    i.fits(pattern, &v)?;
                }
                i.run_stmts(&section.body)
            });
            match ran {
                Err(Stop::Break) => return Ok(()),
                Err(Stop::Goto(Jump::Case(next))) => {
                    v = next;
                    to_default = false;
                }
                Err(Stop::Goto(Jump::Default)) => to_default = true,
                other => return other,
            }
        }
    }

    /// The first section a label of takes `v`.
    fn section_for<'s>(&mut self, sections: &'s [Section], v: &Value) -> Run<Chosen<'s>> {
        for s in sections {
            for (pattern, guard) in s.labels.iter().flatten() {
                let taken = self.scoped(|i| -> Run<bool> {
                    if !i.fits(pattern, v)? {
                        return Ok(false);
                    }
                    match guard {
                        Some(g) => Ok(i.eval(g)? == Value::Bool(true)),
                        None => Ok(true),
                    }
                })?;
                if taken {
                    return Ok(Some((s, Some(pattern))));
                }
            }
        }
        Ok(None)
    }
}

/// The `default` section.
fn default_of(sections: &[Section]) -> Chosen<'_> {
    sections
        .iter()
        .find(|s| s.labels.iter().any(Option::is_none))
        .map(|s| (s, None))
}

//! `goto` on the JVM, which has none. A block with labels is written as a
//! loop over a `switch` of states, each label a `case` (C#'s compiler
//! makes the same of an iterator): what's before its first label as
//! any code, its locals after that declared before the loop (a state is
//! entered past their declarations); `goto L` sets L's state and goes on
//! round. A `switch` statement with `goto case`/`goto default` is run
//! again with the value given (or straight to its `default`).

use std::collections::HashSet;

use yog_script_analyze::ast::{Binding, Goto, Name, Section, Stmt};
use yog_script_analyze::leaves;
use yog_script_analyze::token::Span;
use yog_script_ids::java::{
    BOOLEAN, BREAK, CASE, CONTINUE, FALSE, IF, INT, SWITCH, TRUE, VAR, WHILE, ZERO,
};

use super::Gen;

/// Why a dispatch loop was left by a `break` or `continue` of the loop around it.
const LEAVE_BY_BREAK: &str = "1";
const LEAVE_BY_CONTINUE: &str = "2";

/// A dispatch loop inside another loop: what says it was left by a jump
/// of that one, and its jumps.
struct Leave {
    by: String,
    outer_break: Option<String>,
    outer_continue: Option<Option<String>>,
}

/// A label in reach: its state, of which dispatch loop and state variable.
pub(crate) struct LabelPlace {
    name: String,
    state: String,
    dispatch: String,
    index: usize,
}

/// A `switch` a `goto case` runs again: its value's and its "to `default`"
/// variables, its loop.
#[derive(Clone)]
pub(crate) struct CaseJump {
    pub value: String,
    pub to_default: String,
    pub dispatch: String,
}

impl Gen<'_> {
    /// A block's statements, with labels among them.
    pub(crate) fn labeled_stmts(&mut self, stmts: &[Stmt]) {
        let Some(first) = stmts.iter().position(|s| matches!(s, Stmt::Label(_))) else {
            return stmts.iter().for_each(|s| self.stmt(s));
        };
        // Before its first label: nothing goes in there; written as any
        // unless it jumps (then it's the state the loop starts in).
        let jumps = stmts[..first]
            .iter()
            .any(|s| s.any_inside(&|i| matches!(i, Stmt::Goto(Goto::Label(_), _)), &|_| true));
        let first = if jumps { 0 } else { first };
        stmts[..first].iter().for_each(|s| self.stmt(s));
        let rest = &stmts[first..];
        let start = usize::from(jumps);
        let (state, dispatch) = (self.temp(), self.temp());
        let hoisted = self.hoist(rest);
        let depth = self.goto_labels.len();
        let labels = rest.iter().filter_map(|s| match s {
            Stmt::Label(n) => Some(n.text.clone()),
            _ => None,
        });
        let places: Vec<LabelPlace> = labels
            .enumerate()
            .map(|(index, name)| LabelPlace {
                name,
                state: state.clone(),
                dispatch: dispatch.clone(),
                index: start + index,
            })
            .collect();
        self.goto_labels.extend(places);
        self.code.line(format!("{INT} {state} = {ZERO};"));
        let leave = self.leaving_through(&dispatch);
        self.code.open(format!("{dispatch}: {WHILE} ({TRUE})"));
        self.code.open(format!("{SWITCH} ({state})"));
        let mut index = 0;
        if jumps {
            self.code.line(format!("{CASE} {index}:"));
            index += 1;
        }
        for s in rest {
            if matches!(s, Stmt::Label(_)) {
                self.code.line(format!("{CASE} {index}:"));
                index += 1;
            } else {
                self.stmt(s);
            }
        }
        self.code.close("");
        // Its end left by the code (a `return`): the loop never ends either.
        if !rest.last().is_some_and(leaves) {
            self.code.line(format!("{BREAK} {dispatch};"));
        }
        self.code.close("");
        self.left_through(leave);
        self.goto_labels.truncate(depth);
        self.hoisted.retain(|n| !hoisted.contains(n));
    }

    /// A `break` or `continue` of the loop around a dispatch loop (`dispatch`)
    /// leaving it first: a variable set to which (`LEAVE_BY_BREAK`,
    /// `LEAVE_BY_CONTINUE`), then the dispatch loop left; what it was before.
    fn leaving_through(&mut self, dispatch: &str) -> Option<Leave> {
        let (outer_break, outer_continue) = (self.breaks.last().cloned()?, self.continues.last().cloned());
        let by = self.temp();
        self.code.line(format!("{INT} {by} = {ZERO};"));
        self.breaks.push(Some(format!("{by} = {LEAVE_BY_BREAK}; {BREAK} {dispatch};")));
        self.continues.push(Some(format!("{by} = {LEAVE_BY_CONTINUE}; {BREAK} {dispatch};")));
        Some(Leave {
            by,
            outer_break,
            outer_continue,
        })
    }

    /// After a dispatch loop: the `break` or `continue` that left it, done.
    fn left_through(&mut self, leave: Option<Leave>) {
        let Some(l) = leave else {
            return;
        };
        self.breaks.pop();
        self.continues.pop();
        let own = |jump: Option<String>, word: &str| jump.unwrap_or_else(|| format!("{word};"));
        self.code.line(format!("{IF} ({} == {LEAVE_BY_BREAK}) {{ {} }}", l.by, own(l.outer_break, BREAK)));
        if let Some(c) = l.outer_continue {
            self.code.line(format!("{IF} ({} == {LEAVE_BY_CONTINUE}) {{ {} }}", l.by, own(c, CONTINUE)));
        }
    }

    /// The locals `stmts` declare, declared now (each its zero): a state
    /// goes in past their declarations, which then set them.
    fn hoist(&mut self, stmts: &[Stmt]) -> HashSet<String> {
        let mut names = HashSet::new();
        for s in stmts {
            let Stmt::Local { ty, binding, value, .. } = s else {
                continue;
            };
            let Binding::One(n) = binding else {
                self.not_yet("taking apart a value after a label", binding_at(binding));
                continue;
            };
            let typed = match (ty, value.as_ref().and_then(|v| self.types.of(v)).cloned()) {
                (Some(w), _) => (self.written(w, false), self.zero_of_written(w)),
                (None, Some(t)) => (self.java(&t, false), crate::exprs::zero(&t)),
                (None, None) => {
                    self.not_yet("a local of no type after a label", n.span);
                    continue;
                }
            };
            if self.boxed.contains(&n.text) {
                self.not_yet("a local a lambda changes, after a label", n.span);
                continue;
            }
            self.code.line(format!("{} {} = {};", typed.0, Self::ident(&n.text), typed.1));
            names.insert(n.text.clone());
        }
        self.hoisted.extend(names.iter().cloned());
        names
    }

    /// `goto to;` (at `at`).
    pub(crate) fn goto_stmt(&mut self, to: &Goto, at: Span) {
        let line = match to {
            Goto::Label(n) => match self.goto_labels.iter().rev().find(|l| l.name == n.text) {
                Some(l) => format!("{} = {}; {CONTINUE} {};", l.state, l.index, l.dispatch),
                None => return self.not_yet("a `goto` out of a lambda", at),
            },
            Goto::Case(v) => match self.case_jumps.last().cloned().flatten() {
                Some(j) => {
                    let v = self.expr(v);
                    format!("{} = {v}; {} = {FALSE}; {CONTINUE} {};", j.value, j.to_default, j.dispatch)
                }
                None => return self.not_yet("`goto case` here", at),
            },
            Goto::Default => match self.case_jumps.last().cloned().flatten() {
                Some(j) => format!("{} = {TRUE}; {CONTINUE} {};", j.to_default, j.dispatch),
                None => return self.not_yet("`goto default` here", at),
            },
        };
        self.code.open("");
        self.code.line(line);
        self.code.close("");
    }

    /// `f` (a `switch` statement's code, of `sections`) for `goto case`s
    /// when one has some: in a loop run again with the value given; the
    /// value's variable (`value`, given `v`) and "to `default`" declared
    /// before it. `f` is given whether it's in one.
    pub(crate) fn case_jumping(
        &mut self,
        sections: &[Section],
        (value, v): (&str, &str),
        f: impl FnOnce(&mut Self, Option<&str>),
    ) {
        if !sections.iter().flat_map(|s| &s.body).any(Stmt::goes_to_case) {
            self.case_jumps.push(None);
            f(self, None);
            self.case_jumps.pop();
            return;
        }
        let (to_default, dispatch) = (self.temp(), self.temp());
        self.code.line(format!("{VAR} {value} = {v};"));
        self.code.line(format!("{BOOLEAN} {to_default} = {FALSE};"));
        self.code.open(format!("{dispatch}: {WHILE} ({TRUE})"));
        self.case_jumps.push(Some(CaseJump {
            value: value.to_string(),
            to_default: to_default.clone(),
            dispatch: dispatch.clone(),
        }));
        f(self, Some(&to_default));
        self.case_jumps.pop();
        if completes(sections) {
            self.code.line(format!("{BREAK} {dispatch};"));
        }
        self.code.close("");
    }
}

/// Whether a `switch` of `sections` written as Java (each section an `if`,
/// `default`'s last) may end normally: no `default`, or one not leaving,
/// or a section leaving by `break` or its end.
fn completes(sections: &[Section]) -> bool {
    let is_default = |s: &Section| s.labels.iter().any(Option::is_none);
    let ends = |s: &Section| !s.body.last().is_some_and(leaves);
    let breaks = |s: &Section| {
        s.body.iter().any(|x| {
            x.any_inside(&|i| matches!(i, Stmt::Break(_)), &|i| {
                !matches!(
                    i,
                    Stmt::Switch { .. } | Stmt::While { .. } | Stmt::DoWhile { .. } | Stmt::For { .. } | Stmt::Foreach { .. }
                )
            })
        })
    };
    !sections.iter().any(is_default) || sections.iter().any(|s| ends(s) || breaks(s))
}

/// Where binding `b` is written.
fn binding_at(b: &Binding) -> Span {
    match b {
        Binding::One(n) => n.span,
        Binding::Tuple(names) => names.first().map_or_else(Span::default, |n: &Name| n.span),
    }
}

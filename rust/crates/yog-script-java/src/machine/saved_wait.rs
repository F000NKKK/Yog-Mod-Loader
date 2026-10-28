//! A `saved async` method's machine (docs/design/storage.md S9): where it
//! waits saved with its owner, picked up again as its owner loads after a
//! restart. The method's code is a method of its own taking the wait to
//! pick up (`$run$Cook($resume, …)`); the method calls it with none, and
//! `$resume$Cook(bytes)` (its owner's `$load`) with one. The machine's
//! record (`$Wait`) is its code's fingerprint, its state, what's left of
//! the delay it waits for, and its fields but what it waits for, by the
//! bus's codec: `$kept` writes it, `$restore` reads it back and waits the
//! rest of the delay.

use yog_script_analyze::ast::Method;
use yog_script_ids::java::{
    BOOLEAN, BREAK, BYTE, CASE, CHAR, DEFAULT, DOUBLE, FALSE, FINAL, FLOAT, IF, INT, LONG, NEW,
    NULL, OVERRIDE, PRIVATE, PUBLIC, RECORD, RETURN, SHORT, SWITCH, THIS, TRUE, VOID, ZERO,
    ZERO_CHAR,
};
use yog_script_ids::jvm::STRING;
use yog_script_ids::jvm::state::{BYTES, FROM_BYTES};
use yog_script_ids::jvm::tasks::{
    COUNTS_TICKS, DELAY_AGAIN, DELAY_OF, DELAYED, KEPT, LEFT, LOST, RESTORE, RESUME_PARAM,
    RESUME_PREFIX, RUN, RUN_PREFIX, SAVED_TASKS, SHAPE, STARTED, STATE, SUSPEND, TASK, WAIT_RECORD,
};

use super::Machine;
use crate::Gen;

/// The record's own components: the code's fingerprint, the state, whether
/// the delay counts ticks, what's left of it.
const SHAPE_PART: &str = "$shape";
const STATE_PART: &str = "$at";
const TICKS_PART: &str = "$ticks";
const LEFT_PART: &str = "$left";
/// `$restore`'s locals: the record read, the delay made again.
const READ: &str = "$w";
const AGAIN: &str = "$d";

impl Gen<'_> {
    /// Saved async method `m` (its Java `head`, `ret`urning, taking
    /// `params` as Java declares them): the method, what picks its wait up
    /// again, and its code's method making its machine.
    pub(crate) fn saved_async_method(
        &mut self,
        head: &str,
        m: &Method,
        (ret, params): (&str, &str),
    ) {
        let name = m.name.text.clone();
        let run = format!("{RUN_PREFIX}{name}");
        let args: Vec<String> = m.params.iter().map(|p| Self::ident(&p.name.text)).collect();
        let zeros: Vec<String> = m
            .params
            .iter()
            .map(|p| zero_of(&self.written(&p.ty, false)).to_string())
            .collect();
        let call = |first: &str, rest: &[String]| {
            let mut all = vec![first.to_string()];
            all.extend(rest.iter().cloned());
            format!("{run}({})", all.join(", "))
        };
        self.code.open(head);
        let returns = if m.ret.is_some() {
            format!("{RETURN} ")
        } else {
            String::new()
        };
        self.code.line(format!("{returns}{};", call(NULL, &args)));
        self.code.close("");
        self.code.open(format!(
            "{PUBLIC} {VOID} {RESUME_PREFIX}{name}(byte[] {RESUME_PARAM})"
        ));
        self.code.line(format!("{};", call(RESUME_PARAM, &zeros)));
        self.code.close("");
        let lead = if params.is_empty() {
            String::new()
        } else {
            format!(", {params}")
        };
        let run_head = format!("{PRIVATE} {ret} {run}(byte[] {RESUME_PARAM}{lead})");
        self.saved_wait = Some(name);
        self.async_method(&run_head, m);
    }

    /// The machine's start: known as its owner's method `name`; picking up
    /// a wait when it's given one, else run from its start.
    pub(crate) fn wait_start(&mut self, made: &str, name: &str, task: bool) {
        let method = self.code.str(name);
        self.code.line(format!(
            "{SAVED_TASKS}.{STARTED}({THIS}, {method}, {made});"
        ));
        let given = if task {
            format!("{RETURN} {made}.{TASK}();")
        } else {
            format!("{RETURN};")
        };
        self.code.open(format!("{IF} ({RESUME_PARAM} != {NULL})"));
        self.code.line(format!(
            "{IF} (!{made}.{RESTORE}({RESUME_PARAM})) {SAVED_TASKS}.{LOST}({THIS}, {method});"
        ));
        self.code.line(&given);
        self.code.close("");
        self.code.line(format!("{made}.{RUN}();"));
        if task {
            self.code.line(given);
        }
    }

    /// The machine's `$Wait` record, `$kept` and `$restore`.
    pub(crate) fn wait_members(&mut self, m: &Machine) {
        let kept: Vec<&(String, String)> = m
            .typed
            .iter()
            .filter(|(_, f)| !m.awaited.iter().any(|(_, a)| a == f))
            .collect();
        let mut parts = vec![
            format!("{STRING} {SHAPE_PART}"),
            format!("{INT} {STATE_PART}"),
            format!("{BOOLEAN} {TICKS_PART}"),
            format!("{LONG} {LEFT_PART}"),
        ];
        parts.extend(kept.iter().map(|(t, f)| format!("{t} {f}")));
        self.code
            .line(format!("{RECORD} {WAIT_RECORD}({}) {{}}", parts.join(", ")));
        let values: Vec<&str> = kept.iter().map(|(_, f)| f.as_str()).collect();
        self.code.line(OVERRIDE);
        self.code.open(format!("{PUBLIC} byte[] {KEPT}()"));
        self.code
            .line(format!("{FINAL} {DELAYED} {AGAIN} = {DELAY_OF}();"));
        self.code
            .line(format!("{IF} ({AGAIN} == {NULL}) {RETURN} {NULL};"));
        let mut args = vec![
            format!("{SHAPE}()"),
            STATE.to_string(),
            format!("{AGAIN}.{COUNTS_TICKS}()"),
            format!("{AGAIN}.{LEFT}()"),
        ];
        args.extend(values.iter().map(|v| v.to_string()));
        self.code.line(format!(
            "{RETURN} {BYTES}({NEW} {WAIT_RECORD}({}));",
            args.join(", ")
        ));
        self.code.close("");
        self.restore_member(m, &values);
    }

    /// `$restore(bytes)`: the record read; the same code's, its state and
    /// fields set, the rest of its delay waited for; false otherwise.
    fn restore_member(&mut self, m: &Machine, values: &[&str]) {
        self.code
            .open(format!("{BOOLEAN} {RESTORE}(byte[] {RESUME_PARAM})"));
        self.code.line(format!(
            "{FINAL} {WAIT_RECORD} {READ} = {FROM_BYTES}({RESUME_PARAM}, {WAIT_RECORD}.class);"
        ));
        self.code.line(format!(
            "{IF} ({READ} == {NULL} || !{READ}.{SHAPE_PART}().equals({SHAPE}())) {RETURN} {FALSE};"
        ));
        self.code.line(format!("{STATE} = {READ}.{STATE_PART}();"));
        for v in values {
            self.code.line(format!("{THIS}.{v} = {READ}.{v}();"));
        }
        self.code.line(format!(
            "{FINAL} {DELAYED} {AGAIN} = {DELAYED}.{DELAY_AGAIN}({READ}.{TICKS_PART}(), {READ}.{LEFT_PART}());"
        ));
        self.code.open(format!("{SWITCH} ({STATE})"));
        for (state, field) in &m.awaited {
            self.code
                .line(format!("{CASE} {state}: {field} = {AGAIN}; {BREAK};"));
        }
        self.code.line(format!("{DEFAULT}: {RETURN} {FALSE};"));
        self.code.close("");
        self.code.line(format!("{SUSPEND}({AGAIN});"));
        self.code.line(format!("{RETURN} {TRUE};"));
        self.code.close("");
    }
}

/// What a parameter of Java type `ty` is given when a wait is picked up
/// (its machine's field is set from the save instead).
fn zero_of(ty: &str) -> &'static str {
    match ty {
        BOOLEAN => FALSE,
        CHAR => ZERO_CHAR,
        BYTE | SHORT | INT | LONG | FLOAT | DOUBLE => ZERO,
        _ => NULL,
    }
}

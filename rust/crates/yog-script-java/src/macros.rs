//! The built-in macros' calls, each a runtime helper's: `panic!`, `todo!`
//! and `unreachable!` panic, `assert!` and `assert_eq!` check, `dbg!` logs
//! its expression as written and gives its value. `include_str!` is the
//! file's text by now (the build reads it); one left here had no files.

use yog_script_analyze::ast::Expr;
use yog_script_analyze::macros::builtin::{self, Gives};
use yog_script_ids::jvm::{CHECK, CHECK_EQ, DBG, HELPERS, PANIC, STRING, VALUE_OF};

use super::Gen;
use crate::types::RUNTIME;

impl Gen<'_> {
    pub(crate) fn macro_call(&mut self, e: &Expr, name: &str, args: &[Expr]) -> String {
        let Some(m) = builtin::named(name).filter(|m| m.gives != Gives::Text) else {
            self.not_yet("this macro", e.span);
            return String::new();
        };
        let at = self.at(e.span);
        let given: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
        let first = given.first().cloned().unwrap_or_default();
        let ys = format!("{RUNTIME}.{HELPERS}");
        if std::ptr::eq(m, &builtin::ASSERT) {
            let message = given
                .get(1)
                .cloned()
                .unwrap_or_else(|| self.code.str(m.says));
            return format!("{ys}.{CHECK}({first}, {message}, {at})");
        }
        if std::ptr::eq(m, &builtin::ASSERT_EQ) {
            let second = given.get(1).cloned().unwrap_or_default();
            return format!("{ys}.{CHECK_EQ}({first}, {second}, {at})");
        }
        if std::ptr::eq(m, &builtin::DBG) {
            let what = args
                .first()
                .map_or_else(String::new, |a| self.code.str(self.report.text(a.span)));
            return format!("{ys}.{DBG}({first}, {what}, {at})");
        }
        // `panic!`, `todo!`, `unreachable!`: its message, else its own.
        let message = if given.is_empty() {
            self.code.str(m.says)
        } else {
            format!("{STRING}.{VALUE_OF}({first})")
        };
        format!("{ys}.{PANIC}({message}, {at})")
    }
}

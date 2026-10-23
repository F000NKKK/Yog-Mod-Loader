//! The built-in macros run: `panic!` and its kind stop with a panic,
//! `assert!`s check, `dbg!` logs.

use yog_script_analyze::ast::{Expr, Name};
use yog_script_analyze::macros::builtin::{self, Gives};
use yog_script_analyze::token::Span;

use super::{Interp, Run};
use crate::value::Value;

impl<'p> Interp<'p> {
    pub(crate) fn builtin_macro(&mut self, name: &Name, args: &[Expr], at: Span) -> Run<Value> {
        let Some(m) = builtin::named(&name.text) else {
            return self.panic(format!("no macro `{}!`", name.text), at);
        };
        let given = args.iter().map(|a| self.eval(a)).collect::<Run<Vec<_>>>()?;
        let said = |k: usize| given.get(k).map(ToString::to_string);
        if m.gives == Gives::Never {
            let message = said(0).unwrap_or_else(|| m.doc.to_string());
            return self.panic(message, at);
        }
        if std::ptr::eq(m, &builtin::ASSERT) && !given.first().is_some_and(Value::truthy) {
            return self.panic(
                said(1).unwrap_or_else(|| "assertion failed".to_string()),
                at,
            );
        }
        if std::ptr::eq(m, &builtin::ASSERT_EQ) && given.first() != given.get(1) {
            let (a, b) = (said(0).unwrap_or_default(), said(1).unwrap_or_default());
            return self.panic(format!("assertion failed: `{a}` isn't `{b}`"), at);
        }
        Ok(match m.gives {
            Gives::First => {
                let v = given.first().cloned().unwrap_or(Value::Void);
                self.log.push(format!("{v}"));
                v
            }
            _ => Value::Void,
        })
    }
}

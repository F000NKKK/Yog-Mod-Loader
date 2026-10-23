//! A built-in macro's call (`panic!`, `assert!`, …): its arguments
//! checked as it takes them, its value typed.

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use crate::ast::{Expr, Name};
use crate::macros::builtin::{self, Gives};

impl Checker<'_, '_, '_> {
    pub(super) fn builtin_macro(&mut self, name: &Name, args: &[Expr]) -> Type {
        let Some(m) = builtin::named(&name.text) else {
            self.report.error(
                script_problem::MACRO,
                name.span,
                format!("no macro `{}!` is declared here", name.text),
            );
            args.iter().for_each(|a| {
                self.expr(a);
            });
            return Type::Unknown;
        };
        let given: Vec<Type> = args
            .iter()
            .enumerate()
            .map(|(i, a)| {
                // `assert!`'s condition is a `bool`; the rest are anything.
                if i == 0 && std::ptr::eq(m, &builtin::ASSERT) {
                    self.fits(a, &Type::Bool);
                    Type::Bool
                } else {
                    self.expr(a)
                }
            })
            .collect();
        if std::ptr::eq(m, &builtin::ASSERT_EQ)
            && let [a, b] = given.as_slice()
            && !self.converts(a, b)
            && !self.converts(b, a)
        {
            self.want(b, a, args[1].span);
        }
        match m.gives {
            // It never gives a value, so it goes where any is wanted.
            Gives::Never => Type::Unknown,
            Gives::Nothing => Type::Void,
            Gives::First => given.first().cloned().unwrap_or(Type::Unknown),
            Gives::Text => Type::String,
        }
    }
}

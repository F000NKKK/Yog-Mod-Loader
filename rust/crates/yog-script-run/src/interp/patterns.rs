//! Patterns: a value matched by its shape, the names it binds bound.

use yog_script_analyze::ast::{Arm, Expr, Pattern, Type, TypeKind};
use yog_script_analyze::token::Span;

use super::{Interp, Run};
use crate::value::Value;

impl<'p> Interp<'p> {
    /// `x switch { … }`: the first arm whose pattern (and guard) fits.
    pub(crate) fn switch(&mut self, on: &Expr, arms: &[Arm], at: Span) -> Run<Value> {
        let v = self.eval(on)?;
        for arm in arms {
            let picked = self.scoped(|i| {
                if !i.fits(&arm.pattern, &v)? {
                    return Ok(None);
                }
                if let Some(g) = &arm.guard
                    && !i.eval(g)?.truthy()
                {
                    return Ok(None);
                }
                i.eval(&arm.value).map(Some)
            })?;
            if let Some(out) = picked {
                return Ok(out);
            }
        }
        self.panic(format!("no arm of this `switch` takes `{v}`"), at)
    }

    /// `v is pattern`, its names bound in the current scope.
    pub(crate) fn matches(&mut self, p: &Pattern, v: &Value) -> Run<Value> {
        self.fits(p, v).map(Value::Bool)
    }

    fn fits(&mut self, p: &Pattern, v: &Value) -> Run<bool> {
        Ok(match p {
            Pattern::Wild(_) => true,
            Pattern::Let(n) => {
                self.bind(&n.text, v.clone());
                true
            }
            Pattern::Value(e) => self.eval(e)? == *v,
            Pattern::Type { ty, bind } => {
                let fits = match self.case_named(ty) {
                    Some((of, case)) => {
                        matches!(v, Value::Case(c) if c.name == case && of.is_none_or(|o| o == c.of))
                    }
                    None => self.is_type(v, ty),
                };
                if fits && let Some(n) = bind {
                    self.bind(&n.text, v.clone());
                }
                fits
            }
            Pattern::Variant { path, args } => {
                let Value::Case(c) = v else { return Ok(false) };
                let named = path.last().is_some_and(|n| n.text == c.name);
                let of = path.len().checked_sub(2).map(|k| path[k].text.as_str());
                if !named || of.is_some_and(|o| o != c.of) || args.len() != c.fields.len() {
                    return Ok(false);
                }
                let fields = c.fields.clone();
                for (a, f) in args.iter().zip(&fields) {
                    if !self.fits(a, f)? {
                        return Ok(false);
                    }
                }
                true
            }
            _ => self.fits_more(p, v)?,
        })
    }

    fn fits_more(&mut self, p: &Pattern, v: &Value) -> Run<bool> {
        Ok(match p {
            Pattern::Relational { op, value } => {
                let r = self.eval(value)?;
                self.binary_values(*op, v.clone(), r, value.span)?.truthy()
            }
            Pattern::Range { from, to } => match (self.eval(from)?, self.eval(to)?, v) {
                (Value::Int(a), Value::Int(b), Value::Int(n)) => (a..=b).contains(n),
                _ => false,
            },
            Pattern::Tuple(all) => {
                let Value::List(items) = v else {
                    return Ok(false);
                };
                let items = items.borrow().clone();
                if items.len() != all.len() {
                    return Ok(false);
                }
                for (p, item) in all.iter().zip(&items) {
                    if !self.fits(p, item)? {
                        return Ok(false);
                    }
                }
                true
            }
            Pattern::Property { ty, fields } => {
                if ty.as_ref().is_some_and(|t| !self.is_type(v, t)) {
                    return Ok(false);
                }
                for (name, p) in fields {
                    let field = self.member(v, &name.text, name.span)?;
                    if !self.fits(p, &field)? {
                        return Ok(false);
                    }
                }
                true
            }
            Pattern::Or(a, b) => self.fits(a, v)? || self.fits(b, v)?,
            Pattern::And(a, b) => self.fits(a, v)? && self.fits(b, v)?,
            Pattern::Not(a) => !self.fits(a, v)?,
            _ => false,
        })
    }

    /// `Mode.Idle` written as a type: the enum (if named) and the case.
    fn case_named<'t>(&self, ty: &'t Type) -> Option<(Option<&'t str>, &'t str)> {
        let TypeKind::Path { path, .. } = &ty.kind else {
            return None;
        };
        let case = path.last()?.text.as_str();
        let of = path.len().checked_sub(2).map(|k| path[k].text.as_str());
        let is_case = match of {
            Some(o) => self.program.variant(o, case).is_some(),
            None => self
                .program
                .types
                .values()
                .any(|t| t.variants.iter().any(|v| v.name.text == case)),
        };
        is_case.then_some((of, case))
    }
}

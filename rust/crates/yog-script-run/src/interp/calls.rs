//! Calls: a lambda, a method of `this`, the file's, a type's static one,
//! an object's, a case made (`Ok(v)`, `Mode.Charging(4)`), a native one —
//! and what `ref` and `out` arguments get back.

use std::rc::Rc;

use yog_script_analyze::ast::{Arg, Expr, ExprKind, LambdaBody, Method, Pass};
use yog_script_analyze::token::Span;

use super::{Interp, Run, Stop};
use crate::program::Member;
use crate::value::{Closure, Value};

/// A delegate's own call, as C#'s.
const INVOKE: &str = "Invoke";

impl<'p> Interp<'p> {
    pub(crate) fn call_expr(&mut self, callee: &Expr, args: &[Arg], at: Span) -> Run<Value> {
        let given = args
            .iter()
            .map(|a| self.eval(&a.value))
            .collect::<Run<Vec<_>>>()?;
        match &callee.kind {
            ExprKind::Ident(name) => self.call_name(name, args, given, at),
            ExprKind::Member { expr, name, .. } => {
                let on = self.eval(expr)?;
                self.call_member(on, &name.text, args, given, at)
            }
            _ => {
                let f = self.eval(callee)?;
                self.call_value(&f, given, at)
            }
        }
    }

    /// Type `of`'s static method `name` called with `args`.
    pub fn call_static(
        &mut self,
        of: &str,
        name: &str,
        args: Vec<Value>,
    ) -> Result<Value, super::Panic> {
        let Some(m) = self.method_of(of, name, args.len()) else {
            return Err(super::Panic {
                message: format!(
                    "`{of}` has no method `{name}` taking {} arguments",
                    args.len()
                ),
                span: Span::default(),
            });
        };
        match self.run_member(m, None, &[], args, m.method.name.span) {
            Err(Stop::Panic(p)) => Err(p),
            Err(_) => Ok(Value::Void),
            Ok(v) => Ok(v),
        }
    }

    /// `name(…)`: a local lambda, a method of `this`, the file's, a case of
    /// the prelude's `Result`.
    fn call_name(&mut self, name: &str, args: &[Arg], given: Vec<Value>, at: Span) -> Run<Value> {
        if let Some(f) = self.lookup(name) {
            return self.call_value(&f, given, at);
        }
        if let Some(Value::Object(o)) = self.this.clone() {
            let class = o.borrow().class.clone();
            if let Some(m) = self.method_of(&class, name, given.len()) {
                let this = Value::Object(o);
                return self.run_member(m, Some(this), args, given, at);
            }
        }
        if let Some(m) = self.program.file_method(name, given.len()) {
            return self.run(m, None, args, given, at);
        }
        let result = yog_script_analyze::system::RESULT;
        if self.program.variant(result, name).is_some() {
            return Ok(Value::case(result, name, given));
        }
        self.panic(
            format!("no method `{name}` takes {} arguments here", given.len()),
            at,
        )
    }

    /// `on.name(…)`.
    fn call_member(
        &mut self,
        on: Value,
        name: &str,
        args: &[Arg],
        given: Vec<Value>,
        at: Span,
    ) -> Run<Value> {
        match &on {
            Value::Type(of) => {
                if let Some(m) = self.method_of(of, name, given.len()) {
                    return self.run_member(m, None, args, given, at);
                }
                if self.program.variant(of, name).is_some() {
                    return Ok(Value::case(of, name, given));
                }
                match crate::natives::call_static(of, name, &given) {
                    Some(v) => v.map_err(|m| self.stop(m, at)),
                    None => self.panic(format!("`{of}` has no `{name}`"), at),
                }
            }
            Value::Object(o) => {
                let class = o.borrow().class.clone();
                match self.method_of(&class, name, given.len()) {
                    Some(m) => self.run_member(m, Some(on.clone()), args, given, at),
                    None => self.native(&on, name, given, at),
                }
            }
            Value::Fn(_) if name == INVOKE => self.call_value(&on, given, at),
            _ => self.native(&on, name, given, at),
        }
    }

    fn native(&mut self, on: &Value, name: &str, given: Vec<Value>, at: Span) -> Run<Value> {
        let ok = match on {
            Value::Case(c) => self.is_ok_case(c),
            _ => false,
        };
        match crate::natives::call(on, ok, name, &given) {
            Some(v) => v.map_err(|m| self.stop(m, at)),
            None => self.panic(
                format!(
                    "`{on}` has no method `{name}` taking {} arguments",
                    given.len()
                ),
                at,
            ),
        }
    }

    fn method_of(&self, of: &str, name: &str, count: usize) -> Option<Member<'p>> {
        let (key, _) = self.program.types.get_key_value(of)?;
        self.program.method(key, name, count)
    }

    /// A type's method: run, or — declared without a body, the library's
    /// — done natively.
    fn run_member(
        &mut self,
        m: Member<'p>,
        this: Option<Value>,
        args: &[Arg],
        given: Vec<Value>,
        at: Span,
    ) -> Run<Value> {
        let this = if m.is_static { None } else { this };
        if m.method.body.is_none() {
            let on = this.unwrap_or(Value::Void);
            return self.native(&on, &m.method.name.text, given, at);
        }
        self.run(m.method, this, args, given, at)
    }

    /// `m` run; what its `ref` and `out` parameters end with written back.
    fn run(
        &mut self,
        m: &'p Method,
        this: Option<Value>,
        args: &[Arg],
        given: Vec<Value>,
        at: Span,
    ) -> Run<Value> {
        let ended = self.invoke(m, this, given, at)?;
        for (a, v) in args.iter().zip(ended.params) {
            match (&a.pass, &a.value.kind) {
                (Some(Pass::Out), ExprKind::Declare { name, .. }) => self.bind(&name.text, v),
                (Some(Pass::Ref | Pass::Out), _) => self.assign(&a.value, v)?,
                _ => {}
            }
        }
        Ok(ended.value)
    }

    /// A lambda called with `given`.
    pub(crate) fn call_value(&mut self, f: &Value, given: Vec<Value>, at: Span) -> Run<Value> {
        let Value::Fn(c) = f else {
            return self.panic(format!("`{f}` isn't called"), at);
        };
        let c: Rc<Closure> = Rc::clone(c);
        let outer = (
            std::mem::replace(&mut self.scopes, c.scopes.clone()),
            std::mem::replace(&mut self.this, c.this.clone()),
        );
        let out = self.scoped(|i| {
            let params = &c.params;
            if params.is_empty() && given.len() == 1 {
                i.bind(yog_script_analyze::ast::IT, given[0].clone());
            }
            for (p, v) in params.iter().zip(given) {
                i.bind(&p.name.text, v);
            }
            match &c.body {
                LambdaBody::Expr(e) => i.eval(e),
                LambdaBody::Block(stmts) => match stmts.iter().try_for_each(|s| i.stmt(s)) {
                    Ok(()) => Ok(Value::Void),
                    Err(Stop::Return(v)) => Ok(v),
                    Err(e) => Err(e),
                },
            }
        });
        (self.scopes, self.this) = outer;
        out
    }

    pub(crate) fn stop(&self, message: String, span: Span) -> Stop {
        Stop::Panic(super::Panic { message, span })
    }
}

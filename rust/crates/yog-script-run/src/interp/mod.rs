//! The interpreter: frames of locals, `this`, and how a run stops — a
//! panic (a bug, with where it was), a `return`, a `break`, a `continue`.

mod calls;
mod exprs;
mod macros;
mod objects;
mod ops;
mod patterns;
mod stmts;

use std::collections::HashMap;

use yog_script_analyze::ast::{Method, MethodBody};
use yog_script_analyze::token::Span;

use crate::program::Program;
use crate::value::Value;

/// A panic: a bug the code ran into, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct Panic {
    pub message: String,
    pub span: Span,
}

/// Why running stopped before its end.
pub(crate) enum Stop {
    Panic(Panic),
    Return(Value),
    Break,
    Continue,
}

pub(crate) type Run<T> = Result<T, Stop>;

/// How deep calls may nest: a recursion without end panics here.
const MAX_DEPTH: usize = 512;

pub struct Interp<'p> {
    pub(crate) program: &'p Program<'p>,
    /// The running method's locals, innermost scope last.
    scopes: Vec<HashMap<String, Value>>,
    this: Option<Value>,
    depth: usize,
    /// What `dbg!` logged, in order.
    pub log: Vec<String>,
}

/// A method's end: its result, and its parameters' last values (what a
/// `ref` or an `out` gives back).
pub(crate) struct Ended {
    pub value: Value,
    pub params: Vec<Value>,
}

impl<'p> Interp<'p> {
    pub fn new(program: &'p Program<'p>) -> Self {
        Interp {
            program,
            scopes: vec![HashMap::new()],
            this: None,
            depth: 0,
            log: Vec::new(),
        }
    }

    /// The file's method `name` called with `args`: its result, or the panic
    /// that stopped it.
    pub fn call(&mut self, name: &str, args: Vec<Value>) -> Result<Value, Panic> {
        let Some(m) = self.program.file_method(name, args.len()) else {
            return Err(Panic {
                message: format!("no method `{name}` takes {} arguments", args.len()),
                span: Span::default(),
            });
        };
        match self.invoke(m, None, args, m.name.span) {
            Ok(ended) => Ok(ended.value),
            Err(Stop::Panic(p)) => Err(p),
            Err(_) => Ok(Value::Void),
        }
    }

    /// An object of type `name` made with `args` (its constructor's).
    pub fn make_object(&mut self, name: &str, args: Vec<Value>) -> Result<Value, Panic> {
        match self.make(name, args, Span::default()) {
            Err(Stop::Panic(p)) => Err(p),
            Err(_) => Ok(Value::Void),
            Ok(v) => Ok(v),
        }
    }

    /// A block run on its own (a test's): to its end, or the panic that
    /// stopped it.
    pub fn run_block(&mut self, b: &yog_script_analyze::ast::Block) -> Result<(), Panic> {
        match self.block(b) {
            Err(Stop::Panic(p)) => Err(p),
            _ => Ok(()),
        }
    }

    /// `m` run with `args` (its defaults for the rest), `this` its object.
    pub(crate) fn invoke(
        &mut self,
        m: &'p Method,
        this: Option<Value>,
        args: Vec<Value>,
        at: Span,
    ) -> Run<Ended> {
        if self.depth >= MAX_DEPTH {
            return self.panic(format!("`{}` calls itself without end", m.name.text), at);
        }
        let outer = (
            std::mem::replace(&mut self.scopes, vec![HashMap::new()]),
            std::mem::replace(&mut self.this, this),
        );
        self.depth += 1;
        let out = self.run_method(m, args);
        self.depth -= 1;
        let params = m
            .params
            .iter()
            .map(|p| self.lookup(&p.name.text).unwrap_or(Value::Void))
            .collect();
        (self.scopes, self.this) = outer;
        out.map(|value| Ended { value, params })
    }

    fn run_method(&mut self, m: &'p Method, args: Vec<Value>) -> Run<Value> {
        let mut args = args.into_iter();
        for p in &m.params {
            let v = match (args.next(), &p.default) {
                (Some(v), _) => v.copied(),
                (None, Some(d)) => self.eval(d)?,
                (None, None) => Value::Void,
            };
            self.bind(&p.name.text, v);
        }
        self.body(m.body.as_ref())
    }

    /// A method's or a getter's body run: its value.
    pub(crate) fn body(&mut self, body: Option<&MethodBody>) -> Run<Value> {
        let run = match body {
            Some(MethodBody::Block(b)) => self.block(b).map(|()| Value::Void),
            Some(MethodBody::Expr(e)) => self.eval(e),
            None => Ok(Value::Void),
        };
        match run {
            Err(Stop::Return(v)) => Ok(v),
            Err(Stop::Break | Stop::Continue) => Ok(Value::Void),
            other => other,
        }
    }

    pub(crate) fn scoped<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.scopes.push(HashMap::new());
        let out = f(self);
        self.scopes.pop();
        out
    }

    pub(crate) fn bind(&mut self, name: &str, v: Value) {
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name.to_string(), v);
        }
    }

    pub(crate) fn lookup(&self, name: &str) -> Option<Value> {
        self.scopes.iter().rev().find_map(|s| s.get(name)).cloned()
    }

    /// Local `name` set to `v`, if there's one.
    pub(crate) fn set_local(&mut self, name: &str, v: Value) -> bool {
        match self.scopes.iter_mut().rev().find_map(|s| s.get_mut(name)) {
            Some(slot) => {
                *slot = v;
                true
            }
            None => false,
        }
    }

    pub(crate) fn panic<T>(&self, message: impl Into<String>, span: Span) -> Run<T> {
        Err(Stop::Panic(Panic {
            message: message.into(),
            span,
        }))
    }
}

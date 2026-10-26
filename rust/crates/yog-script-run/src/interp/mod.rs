//! The interpreter: frames of locals, `this`, and how a run stops — a
//! panic (a bug, with where it was), a `return`, a `break`, a `continue`.

mod arrays;
mod calls;
mod ctors;
mod exprs;
mod macros;
mod objects;
mod ops;
mod patterns;
mod statics;
mod stmts;

use std::collections::HashMap;

use yog_script_analyze::ast::{Decl, Method, MethodBody};
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
    /// The items each iterator being run has yielded, innermost last: the
    /// interpreter runs an iterator to its end at once, its items a list.
    yielded: Vec<Vec<Value>>,
    /// Each type's static fields' values, set when first reached.
    statics: HashMap<(String, String), Value>,
    /// The type whose method is running (its statics reached by name).
    class: Option<&'p str>,
    /// The local functions in reach, innermost last.
    local_fns: Vec<std::rc::Rc<Decl>>,
}

/// How many items an iterator gives the interpreter before it's taken for
/// an endless one (run at once, it'd never end).
const MAX_YIELDED: usize = 1_000_000;

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
            yielded: Vec::new(),
            statics: HashMap::new(),
            class: None,
            local_fns: Vec::new(),
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

    /// Expression `e` computed on its own (a content's property, a
    /// constant): its value, or the panic that stopped it.
    pub fn value_of(&mut self, e: &yog_script_analyze::ast::Expr) -> Result<Value, Panic> {
        match self.eval(e) {
            Ok(v) => Ok(v),
            Err(Stop::Panic(p)) => Err(p),
            Err(_) => Ok(Value::Void),
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
        let class = std::mem::replace(&mut self.class, self.program.owner_of(m));
        self.depth += 1;
        let out = self.run_method(m, args);
        self.depth -= 1;
        self.class = class;
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
        if !yog_script_analyze::body_yields(m.body.as_ref()) {
            let v = self.body(m.body.as_ref())?;
            // An `async` one ran to its end: a task done with it.
            return Ok(if self.program.is_async(m) {
                Value::done(v)
            } else {
                v
            });
        }
        // An iterator: run to its end, its items a list.
        self.yielded.push(Vec::new());
        let ran = self.body(m.body.as_ref());
        let items = self.yielded.pop().unwrap_or_default();
        ran.map(|_| Value::list(items))
    }

    /// `yield return v`: an item of the iterator being run.
    pub(crate) fn yield_item(&mut self, v: Value, at: Span) -> Run<()> {
        let full = self
            .yielded
            .last()
            .is_some_and(|items| items.len() >= MAX_YIELDED);
        if full {
            return self.panic(
                format!("an iterator gave {MAX_YIELDED} items: the interpreter runs one to its end, and this one may have none"),
                at,
            );
        }
        if let Some(items) = self.yielded.last_mut() {
            items.push(v);
        }
        Ok(())
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

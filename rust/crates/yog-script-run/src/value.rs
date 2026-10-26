//! What the interpreter computes with: numbers, strings, objects (shared,
//! as the heap's are), enum cases, lists, functions, a type named as a value.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use yog_script_analyze::ast::{LambdaBody, LambdaParam};

/// A value.
#[derive(Clone)]
pub enum Value {
    Void,
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Char(char),
    Str(Rc<str>),
    /// A class's, a struct's or a record's object.
    Object(Rc<RefCell<Object>>),
    /// An enum's case and its data.
    Case(Rc<Case>),
    List(Rc<RefCell<Vec<Value>>>),
    /// `a..b` (both ends in), `a..<b` (`b` out).
    Range(i64, i64, bool),
    Fn(Rc<Closure>),
    /// A type named where a value goes (`Math` in `Math.Max`).
    Type(String),
    /// A task, done (the interpreter runs an `async` method to its end):
    /// what it finished with.
    Done(Rc<Value>),
}

pub struct Object {
    pub class: String,
    pub fields: HashMap<String, Value>,
    /// A struct's: copied where it goes.
    pub value: bool,
}

pub struct Case {
    pub of: String,
    pub name: String,
    pub fields: Vec<Value>,
}

/// A lambda and what it captured.
pub struct Closure {
    pub params: Vec<LambdaParam>,
    pub body: LambdaBody,
    pub scopes: Vec<HashMap<String, Value>>,
    pub this: Option<Value>,
    /// An `async` lambda: what it gives is a task.
    pub asynchronous: bool,
}

impl Value {
    /// A task done with `v`.
    pub fn done(v: Value) -> Value {
        Value::Done(Rc::new(v))
    }

    pub fn str(s: &str) -> Value {
        Value::Str(Rc::from(s))
    }

    pub fn list(items: Vec<Value>) -> Value {
        Value::List(Rc::new(RefCell::new(items)))
    }

    pub fn case(of: &str, name: &str, fields: Vec<Value>) -> Value {
        Value::Case(Rc::new(Case {
            of: of.to_string(),
            name: name.to_string(),
            fields,
        }))
    }

    /// The value as it lands somewhere new: a struct's object copied, any
    /// other shared.
    pub fn copied(&self) -> Value {
        match self {
            Value::Object(o) if o.borrow().value => {
                let o = o.borrow();
                Value::Object(Rc::new(RefCell::new(Object {
                    class: o.class.clone(),
                    fields: o
                        .fields
                        .iter()
                        .map(|(k, v)| (k.clone(), v.copied()))
                        .collect(),
                    value: true,
                })))
            }
            v => v.clone(),
        }
    }

    pub fn truthy(&self) -> bool {
        matches!(self, Value::Bool(true))
    }
}

impl PartialEq for Value {
    /// Numbers and strings by value, records and cases by their data,
    /// other objects by identity — as C#'s `==`.
    fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Void, Value::Void) | (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a), Value::Float(b)) | (Value::Float(b), Value::Int(a)) => {
                (*a as f64) == *b
            }
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Case(a), Value::Case(b)) => {
                a.of == b.of && a.name == b.name && a.fields == b.fields
            }
            (Value::Object(a), Value::Object(b)) => {
                Rc::ptr_eq(a, b)
                    || a.borrow().value
                        && b.borrow().class == a.borrow().class
                        && a.borrow().fields == b.borrow().fields
            }
            (Value::List(a), Value::List(b)) => Rc::ptr_eq(a, b),
            (Value::Type(a), Value::Type(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Void => Ok(()),
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Char(c) => write!(f, "{c}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Object(o) => write!(f, "{}", o.borrow().class),
            Value::Case(c) if c.fields.is_empty() => write!(f, "{}.{}", c.of, c.name),
            Value::Case(c) => {
                let fields: Vec<String> = c.fields.iter().map(ToString::to_string).collect();
                write!(f, "{}.{}({})", c.of, c.name, fields.join(", "))
            }
            Value::List(items) => {
                let items: Vec<String> = items.borrow().iter().map(ToString::to_string).collect();
                write!(f, "[{}]", items.join(", "))
            }
            Value::Range(a, b, inclusive) => {
                write!(f, "{a}..{}{b}", if *inclusive { "" } else { "<" })
            }
            Value::Fn(_) => write!(f, "<lambda>"),
            Value::Type(t) => write!(f, "{t}"),
            Value::Done(v) => write!(f, "Task({v})"),
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

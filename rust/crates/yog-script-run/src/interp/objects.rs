//! Objects: made by `new` (a class's fields from its bases down, its
//! constructor's parameters in scope; a record's parameters are its
//! fields), their members read and written.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use yog_script_analyze::ast::{Arg, Expr, Name, Type, TypeKind};
use yog_script_analyze::token::Span;

use super::{Interp, Run};
use crate::value::{Object, Value};

impl<'p> Interp<'p> {
    pub(crate) fn new_object(
        &mut self,
        ty: Option<&Type>,
        args: &[Arg],
        init: &[(Name, Expr)],
        at: Span,
    ) -> Run<Value> {
        let Some(name) = ty.and_then(|t| match &t.kind {
            TypeKind::Path { path, .. } => path.last().map(|n| n.text.as_str()),
            _ => None,
        }) else {
            return self.panic("`new` here needs its type written", at);
        };
        let args = args
            .iter()
            .map(|a| self.eval(&a.value).map(|v| v.copied()))
            .collect::<Run<Vec<_>>>()?;
        let made = self.make(name, args, at)?;
        for (field, e) in init {
            let v = self.eval(e)?.copied();
            self.set_field(Some(&made), &field.text, v, field.span)?;
        }
        Ok(made)
    }

    /// An object of type `name` made with `args`.
    pub(crate) fn make(&mut self, name: &str, args: Vec<Value>, at: Span) -> Run<Value> {
        let Some((key, def)) = self.program.types.get_key_value(name) else {
            return self.panic(format!("no type `{name}` to make"), at);
        };
        let object = Value::Object(Rc::new(RefCell::new(Object {
            class: name.to_string(),
            fields: HashMap::new(),
            value: def.value,
        })));
        let lineage = self.program.lineage(key);
        let outer = self.this.replace(object.clone());
        let made = self.scoped(|i| {
            for (p, v) in def.ctor.iter().zip(args.iter()) {
                i.bind(&p.name.text, v.clone());
                if def.record {
                    i.set_field(Some(&object), &p.name.text, v.clone(), at)?;
                }
            }
            for class in lineage.iter().rev() {
                for (field, value) in &i.program.types[class].fields {
                    let v = match value {
                        Some(e) => i.eval(e)?.copied(),
                        None => Value::Null,
                    };
                    i.set_field(Some(&object), field, v, at)?;
                }
            }
            Ok(())
        });
        self.this = outer;
        made.map(|()| object)
    }

    /// `on.name`: a field, a computed property, a case's data, an enum's
    /// case, a string's or a list's size.
    pub(crate) fn member(&mut self, on: &Value, name: &str, at: Span) -> Run<Value> {
        if let Some(v) = self.field(on, name)? {
            return Ok(v);
        }
        if let Some(v) = crate::natives::member(on, name) {
            return Ok(v);
        }
        match on {
            Value::Type(of) if self.program.variant(of, name).is_some() => {
                Ok(Value::case(of, name, Vec::new()))
            }
            Value::Case(c) => {
                let at_field = self
                    .program
                    .variant(&c.of, &c.name)
                    .and_then(|v| v.fields.iter().position(|f| f.name.text == name));
                match at_field.and_then(|k| c.fields.get(k)) {
                    Some(v) => Ok(v.clone()),
                    None => self.panic(format!("`{on}` has no `{name}`"), at),
                }
            }
            _ => self.panic(format!("`{on}` has no member `{name}`"), at),
        }
    }

    /// An object's field or computed property `name`, if it has one.
    pub(crate) fn field(&mut self, on: &Value, name: &str) -> Run<Option<Value>> {
        let Value::Object(o) = on else {
            return Ok(None);
        };
        if let Some(v) = o.borrow().fields.get(name) {
            return Ok(Some(v.clone()));
        }
        let class = o.borrow().class.clone();
        let Some((key, _)) = self.program.types.get_key_value(class.as_str()) else {
            return Ok(None);
        };
        let getter = self
            .program
            .lineage(key)
            .into_iter()
            .find_map(|t| self.program.types[t].computed.get(name).copied());
        let Some(body) = getter else { return Ok(None) };
        let outer = (
            std::mem::take(&mut self.scopes),
            self.this.replace(on.clone()),
        );
        self.scopes.push(HashMap::new());
        let v = self.body(Some(body));
        (self.scopes, self.this) = outer;
        v.map(Some)
    }

    pub(crate) fn set_field(
        &mut self,
        on: Option<&Value>,
        name: &str,
        v: Value,
        at: Span,
    ) -> Run<()> {
        match on {
            Some(Value::Object(o)) => {
                o.borrow_mut().fields.insert(name.to_string(), v);
                Ok(())
            }
            _ => self.panic(format!("`{name}` can't be set here"), at),
        }
    }
}

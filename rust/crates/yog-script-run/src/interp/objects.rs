//! Objects: made by `new` (a class's fields from its bases down, its
//! constructor's parameters in scope; a record's parameters are its
//! fields), their members read and written.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use yog_script_analyze::ast::{Arg, Expr, Name, Stmt, Type, TypeKind};
use yog_script_analyze::token::Span;

use super::{Interp, Run};
use crate::value::{Object, Value};

impl<'p> Interp<'p> {
    pub(crate) fn new_object(
        &mut self,
        ty: Option<&Type>,
        args: &[Arg],
        (init, items): (&[(Name, Expr)], &[Stmt]),
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
        // `new ItemId(text)`: its id, as its text.
        let tag = name == yog_script_ids::system::TAG_ID;
        if tag
            || matches!(
                yog_script_analyze::system_type(name),
                Some(yog_script_analyze::Type::Content(_))
            )
        {
            let text = args.first().map(ToString::to_string).unwrap_or_default();
            return Ok(Value::str(&yog_ids::qualify(&text)));
        }
        let made = self.make(name, args, at)?;
        for (field, e) in init {
            let v = self.eval(e)?.copied();
            self.set_field(Some(&made), &field.text, v, field.span)?;
        }
        // A collection's items: statements on it, `$made` naming it.
        self.scoped(|i| {
            i.bind(yog_script_ids::members::MADE, made.clone());
            items.iter().try_for_each(|s| i.stmt(s))
        })?;
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
                    // Straight into it: a property's first value isn't set
                    // through its setter (C#'s).
                    if let Value::Object(o) = &object {
                        o.borrow_mut().fields.insert((*field).to_string(), v);
                    }
                }
            }
            Ok(())
        });
        self.this = outer;
        made?;
        self.construct(key, &object, args, at)?;
        Ok(object)
    }

    /// `on.name`: a field, a computed property, a case's data, an enum's
    /// case, a string's or a list's size.
    pub(crate) fn member(&mut self, on: &Value, name: &str, at: Span) -> Run<Value> {
        if let Some(v) = self.field(on, name)? {
            return Ok(v);
        }
        if let Value::Type(of) = on
            && let Some(v) = self.static_value(of, name)?
        {
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

    /// An object's field or property `name`, if it has one: a property's
    /// getter run (its own value as `field`), else the value kept.
    pub(crate) fn field(&mut self, on: &Value, name: &str) -> Run<Option<Value>> {
        let Value::Object(o) = on else {
            return Ok(None);
        };
        if let Some(get) = self.property(on, name).and_then(|p| p.get) {
            return self.accessor(on, name, get, None).map(Some);
        }
        Ok(o.borrow().fields.get(name).cloned())
    }

    pub(crate) fn set_field(
        &mut self,
        on: Option<&Value>,
        name: &str,
        v: Value,
        at: Span,
    ) -> Run<()> {
        match on {
            Some(object @ Value::Object(o)) => {
                // A property's setter runs, `value` what's set.
                if let Some(set) = self.property(object, name).and_then(|p| p.set) {
                    return self.accessor(object, name, set, Some(v)).map(drop);
                }
                o.borrow_mut().fields.insert(name.to_string(), v);
                Ok(())
            }
            _ => self.panic(format!("`{name}` can't be set here"), at),
        }
    }
}

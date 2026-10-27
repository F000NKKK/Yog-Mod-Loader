//! Properties whose accessors have bodies: the getter run as the object's,
//! the setter given `value`; both reach the property's own value as
//! `field` (kept under its name), what the setter leaves in it kept.

use std::collections::HashMap;

use yog_script_analyze::ast::MethodBody;
use yog_script_analyze::words::{field, value};

use super::{Interp, Run};
use crate::program::Property;
use crate::value::Value;

impl<'p> Interp<'p> {
    /// Property `name` of `on`'s class (or of one it inherits), if it has bodies.
    pub(crate) fn property(&self, on: &Value, name: &str) -> Option<Property<'p>> {
        let Value::Object(o) = on else {
            return None;
        };
        let class = o.borrow().class.clone();
        let (key, _) = self.program.types.get_key_value(class.as_str())?;
        self.program
            .lineage(key)
            .into_iter()
            .find_map(|t| self.program.types[t].properties.get(name).copied())
    }

    /// Accessor `body` of property `name` run on `on` (`set`: the setter
    /// given it): its value; `field` written back.
    pub(crate) fn accessor(
        &mut self,
        on: &Value,
        name: &str,
        body: &'p MethodBody,
        set: Option<Value>,
    ) -> Run<Value> {
        let Value::Object(o) = on else {
            return Ok(Value::Void);
        };
        let kept = o.borrow().fields.get(name).cloned().unwrap_or(Value::Null);
        let outer = (
            std::mem::take(&mut self.scopes),
            self.this.replace(on.clone()),
        );
        self.scopes.push(HashMap::new());
        self.bind(field::WORD.text, kept);
        if let Some(v) = set {
            self.bind(value::WORD.text, v);
        }
        let ran = self.body(Some(body));
        let after = self.lookup(field::WORD.text);
        (self.scopes, self.this) = outer;
        // Kept only by a property keeping its own value (made with the object).
        let keeps = o.borrow().fields.contains_key(name);
        if let Some(v) = after.filter(|_| keeps) {
            o.borrow_mut().fields.insert(name.to_string(), v);
        }
        ran
    }
}

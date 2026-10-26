//! Static fields and constants: one value each per type, its first value
//! worked out when it's first reached (as the CLR runs a type's static
//! constructor), read and set by the type's name or, in its own code, by
//! the field's alone. A generic type's are one for all its arguments here.

use std::collections::HashMap;

use super::{Interp, Run};
use crate::value::Value;

impl<'p> Interp<'p> {
    /// Type `of`'s static field `name`, if it has one: its value.
    pub(crate) fn static_value(&mut self, of: &str, name: &str) -> Run<Option<Value>> {
        let Some(first) = self.program.static_field(of, name) else {
            return Ok(None);
        };
        let key = (of.to_string(), name.to_string());
        if let Some(v) = self.statics.get(&key) {
            return Ok(Some(v.clone()));
        }
        // Worked out in no method's locals, as its type's own code.
        let outer = (
            std::mem::replace(&mut self.scopes, vec![HashMap::new()]),
            self.this.take(),
            std::mem::replace(
                &mut self.class,
                self.program.types.get_key_value(of).map(|(k, _)| *k),
            ),
        );
        let v = first.map_or(Ok(Value::Null), |e| self.eval(e));
        (self.scopes, self.this, self.class) = outer;
        let v = v?;
        self.statics.insert(key, v.clone());
        Ok(Some(v))
    }

    /// Static field `name` of the running method's type, if it has one.
    pub(crate) fn own_static(&mut self, name: &str) -> Run<Option<Value>> {
        match self.class {
            Some(of) => self.static_value(of, name),
            None => Ok(None),
        }
    }

    /// `of.name = v` (`of` none: the running method's type): whether it's
    /// a static field to set.
    pub(crate) fn set_static(&mut self, of: Option<&str>, name: &str, v: Value) -> Run<bool> {
        let Some(of) = of.or(self.class) else {
            return Ok(false);
        };
        if self.program.static_field(of, name).is_none() {
            return Ok(false);
        }
        // Its first value worked out first, as reading it would.
        self.static_value(of, name)?;
        self.statics.insert((of.to_string(), name.to_string()), v);
        Ok(true)
    }
}

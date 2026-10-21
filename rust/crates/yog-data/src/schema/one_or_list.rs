//! One value, or a list of them: `"a"` or `["a", "b"]` — what the game's
//! formats accept where several will do (an ingredient's items, a state's
//! models to pick from).

use super::{Reader, Schema, Shape};
use crate::{Value, ValueRef};

/// One value or several, read either way and written back the same way (one
/// as itself). Never empty: an empty list is a problem.
#[derive(Debug, Clone, PartialEq)]
pub struct OneOrList<T>(pub Vec<T>);

impl<T: Schema> Schema for OneOrList<T> {
    fn shape() -> Shape {
        Shape::List(T::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        match v {
            ValueRef::List([]) => {
                cx.problem("an empty list: at least one");
                None
            }
            ValueRef::List(_) => Vec::<T>::read(v, cx).map(OneOrList),
            _ => T::read(v, cx).map(|one| OneOrList(vec![one])),
        }
    }
    fn write(&self) -> Value {
        match self.0.as_slice() {
            [one] => one.write(),
            many => Value::List(many.iter().map(Schema::write).collect()),
        }
    }
}

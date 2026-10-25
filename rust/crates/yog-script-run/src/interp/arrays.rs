//! Arrays as lists of their items (a rectangular one's rows lists too):
//! made with their sizes, each item its type's zero, or with their items.

use yog_script_analyze::ast::{ArrayItem, Expr, Type, TypeKind};
use yog_script_analyze::{Type as Checked, builtin_type};

use super::{Interp, Run};
use crate::value::Value;

impl Interp<'_> {
    /// `new T[sizes]`, `new T[] { … }`.
    pub(super) fn new_array(
        &mut self,
        elem: Option<&Type>,
        sizes: &[Expr],
        init: Option<&[ArrayItem]>,
    ) -> Run<Value> {
        if let Some(items) = init {
            return self.array_items(items);
        }
        let mut lengths = Vec::new();
        for s in sizes {
            let length = match self.eval(s)? {
                Value::Int(n) => usize::try_from(n).ok(),
                _ => None,
            };
            match length {
                Some(n) => lengths.push(n),
                None => {
                    return self.panic("an array's size is a whole number, not below 0", s.span);
                }
            }
        }
        Ok(filled(&lengths, &zero(elem)))
    }

    fn array_items(&mut self, items: &[ArrayItem]) -> Run<Value> {
        let mut out = Vec::with_capacity(items.len());
        for i in items {
            out.push(match i {
                ArrayItem::Value(v) => self.eval(v)?.copied(),
                ArrayItem::Nested(inner) => self.array_items(inner)?,
            });
        }
        Ok(Value::list(out))
    }
}

/// An array of `lengths` (one a rank), each item `item`.
fn filled(lengths: &[usize], item: &Value) -> Value {
    match lengths.split_first() {
        None => item.clone(),
        Some((&n, rest)) => Value::list((0..n).map(|_| filled(rest, item)).collect()),
    }
}

/// What an array of `elem` holds before it's set: a number's zero, `false`,
/// `'\0'`; `null` for anything else.
fn zero(elem: Option<&Type>) -> Value {
    let Some(TypeKind::Path { path, args }) = elem.map(|t| &t.kind) else {
        return Value::Null;
    };
    let word = path.last().map(|n| n.text.as_str()).unwrap_or_default();
    match builtin_type(word).filter(|_| args.is_empty()) {
        Some(Checked::Bool) => Value::Bool(false),
        Some(Checked::Char) => Value::Char('\0'),
        Some(t) if t.is_integer() => Value::Int(0),
        Some(t) if t.is_numeric() => Value::Float(0.0),
        _ => Value::Null,
    }
}

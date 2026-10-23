//! A directive's condition: names, values, `== != < <= > >=`, `! && ||`,
//! brackets, `true`/`false`. Values compare by their dot-separated parts,
//! as numbers where both are (`1.21.1 > 1.9`), else as text.

use std::cmp::Ordering;

use super::{Defines, TRUE};

/// The comparisons, longest first where one starts another.
const COMPARE: [&str; 6] = ["==", "!=", "<=", ">=", "<", ">"];
const AND: &str = "&&";
const OR: &str = "||";
const NOT: char = '!';
const OPEN: char = '(';
const CLOSE: char = ')';
const FALSE: &str = "false";
/// What separates a value's parts.
const PART_SEP: char = '.';

/// Whether `text` holds with `defines`.
pub(super) fn eval(text: &str, defines: &Defines) -> Result<bool, String> {
    let mut c = Cursor {
        rest: text.trim(),
        defines,
    };
    let held = c.or()?;
    if !c.rest.is_empty() {
        return Err(format!("`{}` doesn't belong in the condition", c.rest));
    }
    Ok(held)
}

struct Cursor<'a> {
    rest: &'a str,
    defines: &'a Defines,
}

impl<'a> Cursor<'a> {
    fn eat(&mut self, what: &str) -> bool {
        let found = self.rest.starts_with(what);
        if found {
            self.rest = self.rest[what.len()..].trim_start();
        }
        found
    }

    fn or(&mut self) -> Result<bool, String> {
        let mut held = self.and()?;
        while self.eat(OR) {
            held |= self.and()?;
        }
        Ok(held)
    }

    fn and(&mut self) -> Result<bool, String> {
        let mut held = self.unary()?;
        while self.eat(AND) {
            held &= self.unary()?;
        }
        Ok(held)
    }

    fn unary(&mut self) -> Result<bool, String> {
        if !self.rest.starts_with("!=") && self.eat(&NOT.to_string()) {
            return self.unary().map(|h| !h);
        }
        if self.eat(&OPEN.to_string()) {
            let held = self.or()?;
            if !self.eat(&CLOSE.to_string()) {
                return Err(format!("`{CLOSE}` expected"));
            }
            return Ok(held);
        }
        let left = self.atom()?;
        let Some(op) = COMPARE.into_iter().find(|op| self.rest.starts_with(op)) else {
            return Ok(truthy(&self.value(left)));
        };
        self.eat(op);
        let right = self.atom()?;
        let ord = compare(&self.value(left), &self.value(right));
        Ok(match op {
            "==" => ord == Ordering::Equal,
            "!=" => ord != Ordering::Equal,
            "<=" => ord != Ordering::Greater,
            ">=" => ord != Ordering::Less,
            "<" => ord == Ordering::Less,
            _ => ord == Ordering::Greater,
        })
    }

    /// A name or a value, as written.
    fn atom(&mut self) -> Result<&'a str, String> {
        let len = self
            .rest
            .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == PART_SEP))
            .unwrap_or(self.rest.len());
        if len == 0 {
            return Err(format!("a name or a value expected at `{}`", self.rest));
        }
        let (atom, rest) = self.rest.split_at(len);
        self.rest = rest.trim_start();
        Ok(atom)
    }

    /// A name's value (an undefined one is empty); a value as itself.
    fn value(&self, atom: &str) -> String {
        match self.defines.get(atom) {
            Some(v) => v.clone(),
            None if atom.starts_with(|c: char| c.is_alphabetic() || c == '_')
                && atom != TRUE
                && atom != FALSE =>
            {
                String::new()
            }
            None => atom.to_string(),
        }
    }
}

/// Whether a value is true: set, and not `false` or `0`.
fn truthy(v: &str) -> bool {
    !(v.is_empty() || v == FALSE || v == "0")
}

/// Two values by their parts: numbers as numbers, else text.
fn compare(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.split(PART_SEP), b.split(PART_SEP));
    loop {
        match (x.next(), y.next()) {
            (None, None) => return Ordering::Equal,
            (p, q) => {
                let (p, q) = (p.unwrap_or("0"), q.unwrap_or("0"));
                let ord = match (p.parse::<u64>(), q.parse::<u64>()) {
                    (Ok(m), Ok(n)) => m.cmp(&n),
                    _ => p.cmp(q),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

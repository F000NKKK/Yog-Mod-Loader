//! Atoms: names that repeat across messages (`minecraft:stone`, a live
//! entity's kind) kept once a process and sent on the bus as their number.
//! An [`Atom`] is a `u32`: copied, compared and packed as one; its text is
//! the table's, alive as long as the process. A frame carries the number,
//! every other form (text, the tagged binary form, files) the text — the
//! numbers mean nothing outside the process.
//!
//! The table is the runtime's: a mod's copy of this crate is pointed at it
//! ([`install`]) so every mod and the host agree on the numbers. Without
//! one (tools, tests) the crate keeps its own.
//!
//! ```
//! use yog_data::Atom;
//!
//! let stone = Atom::new("minecraft:stone");
//! assert_eq!(stone, Atom::new("minecraft:stone"));
//! assert_eq!(stone.as_str(), "minecraft:stone");
//! ```

use std::collections::HashMap;
use std::fmt;
use std::sync::{OnceLock, RwLock};

/// A name kept once a process; see the module.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Atom(u32);

/// The table's two functions: a name's number (made the first time), and
/// a number's name (`None`: no such atom).
#[derive(Clone, Copy)]
pub struct Table {
    pub intern: fn(&str) -> u32,
    pub name: fn(u32) -> Option<&'static str>,
}

static TABLE: OnceLock<Table> = OnceLock::new();

/// Points this copy of the crate at the process's table (the runtime's);
/// before any atom is made. False if it has one already.
pub fn install(table: Table) -> bool {
    TABLE.set(table).is_ok()
}

fn table() -> &'static Table {
    TABLE.get_or_init(|| Table {
        intern: local::intern,
        name: local::name,
    })
}

impl Atom {
    /// `name`'s atom.
    pub fn new(name: &str) -> Atom {
        Atom((table().intern)(name))
    }

    /// The atom numbered `n`, if there's one.
    pub fn from_number(n: u32) -> Option<Atom> {
        (table().name)(n).map(|_| Atom(n))
    }

    pub fn number(self) -> u32 {
        self.0
    }

    pub fn as_str(self) -> &'static str {
        (table().name)(self.0).unwrap_or_default()
    }
}

impl fmt::Debug for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Atom({:?})", self.as_str())
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for Atom {
    fn from(name: &str) -> Atom {
        Atom::new(name)
    }
}

impl From<String> for Atom {
    fn from(name: String) -> Atom {
        Atom::new(&name)
    }
}

impl From<&String> for Atom {
    fn from(name: &String) -> Atom {
        Atom::new(name)
    }
}

impl PartialEq<str> for Atom {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Atom {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl std::ops::Deref for Atom {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl Default for Atom {
    /// The empty name's atom.
    fn default() -> Atom {
        Atom::new("")
    }
}

/// The table a process keeps itself: the runtime's, or a tool's.
pub mod local {
    use super::*;

    #[derive(Default)]
    struct Names {
        numbers: HashMap<&'static str, u32>,
        names: Vec<&'static str>,
    }

    static NAMES: OnceLock<RwLock<Names>> = OnceLock::new();

    fn names() -> &'static RwLock<Names> {
        NAMES.get_or_init(Default::default)
    }

    /// `name`'s number, made the first time (its text kept for good).
    pub fn intern(name: &str) -> u32 {
        if let Some(&n) = names()
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .numbers
            .get(name)
        {
            return n;
        }
        let mut t = names().write().unwrap_or_else(|e| e.into_inner());
        if let Some(&n) = t.numbers.get(name) {
            return n;
        }
        let kept: &'static str = Box::leak(name.into());
        let n = u32::try_from(t.names.len()).unwrap_or(u32::MAX);
        t.names.push(kept);
        t.numbers.insert(kept, n);
        n
    }

    pub fn name(n: u32) -> Option<&'static str> {
        names()
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .names
            .get(n as usize)
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_one_atom() {
        let a = Atom::new("test:atom_a");
        assert_eq!(a, Atom::new("test:atom_a"));
        assert_ne!(a, Atom::new("test:atom_b"));
        assert_eq!(Atom::from_number(a.number()), Some(a));
        assert_eq!(Atom::from_number(u32::MAX), None);
        assert_eq!(a.to_string(), "test:atom_a");
    }
}

//! The macros every source has (`System`'s, as Rust's std's): what each
//! is called, what it means, what it gives.

/// A built-in macro.
pub struct Builtin {
    pub name: &'static str,
    /// What it means, for editors (hover, completion).
    pub doc: &'static str,
    pub gives: Gives,
    /// What a panic of it says when it's given no message (Rust's words).
    pub says: &'static str,
}

/// What a built-in macro's call is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Gives {
    /// It never returns: it panics.
    Never,
    /// Nothing: `assert!`.
    Nothing,
    /// Its first argument's value: `dbg!`.
    First,
    /// A string: `include_str!`.
    Text,
}

pub static PANIC: Builtin = Builtin {
    name: "panic",
    doc: "Panics: a bug, reported with its line; the handler stops, the game goes on. `panic!(\"no charger at {pos}\")`.",
    gives: Gives::Never,
    says: "explicit panic",
};
pub static ASSERT: Builtin = Builtin {
    name: "assert",
    doc: "Panics when a condition is false: `assert!(energy >= 0, \"energy below zero\")`.",
    gives: Gives::Nothing,
    says: "assertion failed",
};
pub static ASSERT_EQ: Builtin = Builtin {
    name: "assert_eq",
    doc: "Panics when two values differ, showing both: `assert_eq!(battery.Energy, 100)`.",
    gives: Gives::Nothing,
    says: "assertion failed",
};
pub static NOT_YET: Builtin = Builtin {
    name: "todo",
    doc: "Panics with \"not done yet\": a place left for later.",
    gives: Gives::Never,
    says: "not done yet",
};
pub static UNREACHABLE: Builtin = Builtin {
    name: "unreachable",
    doc: "Panics: code the author says never runs.",
    gives: Gives::Never,
    says: "entered unreachable code",
};
pub static DBG: Builtin = Builtin {
    name: "dbg",
    doc: "Logs a value with its expression and line, and gives it back: `let r = dbg!(rate * 2);`.",
    gives: Gives::First,
    says: "",
};
pub static INCLUDE_STR: Builtin = Builtin {
    name: "include_str",
    doc: "A file of the mod as a string, read at build time: `include_str!(\"help.txt\")`.",
    gives: Gives::Text,
    says: "",
};

/// Every built-in macro.
pub static ALL: &[&Builtin] = &[
    &PANIC,
    &ASSERT,
    &ASSERT_EQ,
    &NOT_YET,
    &UNREACHABLE,
    &DBG,
    &INCLUDE_STR,
];

/// The built-in macro called `name`.
pub fn named(name: &str) -> Option<&'static Builtin> {
    ALL.iter().copied().find(|b| b.name == name)
}

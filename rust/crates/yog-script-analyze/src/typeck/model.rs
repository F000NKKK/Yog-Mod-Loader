//! Types as the checker knows them, how they're shown, and which convert
//! to which without a cast (`ys-grammar.md` §8).

use std::fmt;

use super::builtin;

/// A type.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Decimal,
    Char,
    Bool,
    String,
    /// `object`: every type converts to it.
    Object,
    /// `dynamic`: checked when the code runs, not here.
    Dynamic,
    Void,
    /// `null` before it meets a type.
    Null,
    /// Translatable text: `t"key"`.
    Text,
    /// A quantity with a unit: `Ticks`, `Seconds`, `Blocks`, a mod's own.
    Unit(String),
    /// A content reference of a kind: `@item:…` is `Content("item")`.
    Content(String),
    Tag,
    /// A type the source declares, by name, with its type arguments
    /// (`Box<i32>`; none when it isn't generic).
    Own(String, Vec<Type>),
    /// A type parameter (`T`) where its argument isn't known: in its
    /// generic's own code, or a method's before inference.
    Param(String),
    /// An own type named where a value goes (`Mode` in `Mode.Idle`): its
    /// static members and its cases.
    /// A type named where a value goes (`Math` in `Math.Max`), with the
    /// type arguments written (`Comparer<i32>` in `Comparer<i32>.Default`).
    Static(String, Vec<Type>),
    /// A game class, by its internal name (`.yns` only).
    Game(String),
    /// Yog's: a handle (`Player`), a domain's group (`Chat`), a struct or
    /// an enum an action answers with — by name.
    Yog(String),
    /// A namespace (`Game.World`), on the way to what's in it.
    Namespace(String),
    /// An array: `T[]` (rank 1), `T[,]` (rank 2); a Java array a game
    /// class gives or takes too.
    Array(Box<Type>, usize),
    /// `(i32 X, string)`: its items' types, and their names where given
    /// (names don't make tuple types apart: `(i32 X)` is `(i32 Y)`).
    Tuple(Vec<Type>, Vec<Option<String>>),
    /// A delegate: `Action<A, B>` (its result `void`), `Func<A, R>`.
    Fn(Vec<Type>, Box<Type>),
    Nullable(Box<Type>),
    /// Not known here (the library's, or what couldn't be typed): checks
    /// involving it pass, so one gap doesn't cascade.
    Unknown,
}

/// The units a whole game has; a mod declares its own (`unit Energy;`).
pub use yog_script_ids::units::{BLOCKS, SECONDS, TICKS};

impl Type {
    /// An own type that isn't generic.
    pub fn own(name: impl Into<String>) -> Type {
        Type::Own(name.into(), Vec::new())
    }

    pub fn nullable(self) -> Type {
        match self {
            Type::Nullable(_) | Type::Null | Type::Unknown => self,
            other => Type::Nullable(Box::new(other)),
        }
    }

    /// The type without its `?`.
    pub fn non_null(&self) -> &Type {
        match self {
            Type::Nullable(t) => t,
            t => t,
        }
    }

    pub fn is_nullable(&self) -> bool {
        matches!(self, Type::Nullable(_) | Type::Null)
    }

    pub fn known(&self) -> bool {
        !matches!(self, Type::Unknown)
    }

    pub fn is_numeric(&self) -> bool {
        builtin::is_number(self) || matches!(self, Type::Unit(_))
    }

    pub fn is_integer(&self) -> bool {
        builtin::is_integer(self)
    }

    /// Whether a value of `self` goes where `to` is wanted without a cast.
    pub fn converts_to(&self, to: &Type) -> bool {
        if self == to || !self.known() || !to.known() {
            return true;
        }
        if matches!(self, Type::Dynamic) || matches!(to, Type::Dynamic | Type::Object) {
            return true;
        }
        match (self, to) {
            (Type::Null, Type::Nullable(_)) => true,
            (Type::Nullable(a), Type::Nullable(b)) => a.converts_to(b),
            (a, Type::Nullable(b)) => a.converts_to(b),
            (Type::Unit(a), Type::Unit(b)) => a == SECONDS && b == TICKS,
            // A plain number is a quantity of any unit (`20` ticks), as
            // literals are; quantities of different units never mix.
            (n, Type::Unit(_)) => builtin::is_number(n),
            (Type::Unit(_), n) => builtin::widens(&Type::I32, n),
            (Type::String, Type::Text) => true,
            (Type::Array(a, ra), Type::Array(b, rb)) => ra == rb && a == b,
            (a, b) => builtin::widens(a, b),
        }
    }

    /// The type both of two numbers become in an operation (`i32 + f64` is `f64`).
    /// A whole number's width in bits and whether it's signed (`u8`: 8, no).
    pub fn integer_width(&self) -> Option<(u32, bool)> {
        builtin::integer_width(self)
    }

    pub fn wider(a: &Type, b: &Type) -> Type {
        match (a, b) {
            (Type::Unit(_), _) => a.clone(),
            (_, Type::Unit(_)) => b.clone(),
            _ if builtin::widens(a, b) => b.clone(),
            _ => a.clone(),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::F32
            | Type::F64
            | Type::Decimal
            | Type::Char
            | Type::Bool
            | Type::String
            | Type::Object
            | Type::Dynamic
            | Type::Void => write!(f, "{}", builtin::word(self).unwrap_or_default()),
            Type::Null => write!(f, "{}", crate::words::null::WORD.text),
            Type::Text => write!(f, "Text"),
            Type::Own(name, args) if !args.is_empty() => {
                let args: Vec<_> = args.iter().map(ToString::to_string).collect();
                write!(f, "{name}<{}>", args.join(", "))
            }
            Type::Unit(u)
            | Type::Own(u, _)
            | Type::Static(u, _)
            | Type::Param(u)
            | Type::Yog(u)
            | Type::Namespace(u) => {
                write!(f, "{u}")
            }
            Type::Content(kind) => write!(f, "@{kind}"),
            Type::Tag => write!(f, "{}", yog_ids::kind::TAG),
            Type::Game(name) => write!(f, "{}", crate::index::simple_name(name)),
            // As C# writes it: the item, then the ranks outermost first
            // (`i32[][,]`, an array of `i32[,]`).
            Type::Array(..) => {
                let mut item = self;
                let mut ranks = String::new();
                while let Type::Array(inner, rank) = item {
                    ranks.push_str(&format!("[{}]", ",".repeat(rank - 1)));
                    item = inner;
                }
                write!(f, "{item}{ranks}")
            }
            Type::Tuple(items, names) => {
                let items: Vec<String> = items
                    .iter()
                    .zip(names.iter().chain(std::iter::repeat(&None)))
                    .map(|(t, n)| match n {
                        Some(n) => format!("{t} {n}"),
                        None => t.to_string(),
                    })
                    .collect();
                write!(f, "({})", items.join(", "))
            }
            Type::Fn(params, ret) => {
                let mut all: Vec<String> = params.iter().map(ToString::to_string).collect();
                let word = if **ret == Type::Void {
                    yog_script_ids::system::ACTION
                } else {
                    all.push(ret.to_string());
                    yog_script_ids::system::FUNC
                };
                write!(f, "{word}<{}>", all.join(", "))
            }
            Type::Nullable(t) => write!(f, "{t}?"),
            Type::Unknown => write!(f, "?"),
        }
    }
}

#[cfg(test)]
mod tests;

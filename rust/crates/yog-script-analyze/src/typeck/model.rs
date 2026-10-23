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
    /// A type the source declares, by name.
    Own(String),
    /// An own type named where a value goes (`Mode` in `Mode.Idle`): its
    /// static members and its cases.
    Static(String),
    /// A game class, by its internal name (`.yns` only).
    Game(String),
    /// Yog's: a handle (`Player`), a domain's group (`Chat`), a struct or
    /// an enum an action answers with — by name.
    Yog(String),
    /// A namespace (`Game.World`), on the way to what's in it.
    Namespace(String),
    List(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Tuple(Vec<Type>),
    /// A delegate: `Action<A, B>` (its result `void`), `Func<A, R>`.
    Fn(Vec<Type>, Box<Type>),
    Nullable(Box<Type>),
    /// Not known here (the library's, or what couldn't be typed): checks
    /// involving it pass, so one gap doesn't cascade.
    Unknown,
}

/// The units a whole game has; a mod declares its own (`unit Energy;`).
pub const TICKS: &str = "Ticks";
pub const SECONDS: &str = "Seconds";
pub const BLOCKS: &str = "Blocks";

impl Type {
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
            (Type::List(a), Type::List(b)) => a.converts_to(b),
            (a, b) => builtin::widens(a, b),
        }
    }

    /// The type both of two numbers become in an operation (`i32 + f64` is `f64`).
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
            Type::Unit(u) | Type::Own(u) | Type::Static(u) | Type::Yog(u) | Type::Namespace(u) => {
                write!(f, "{u}")
            }
            Type::Content(kind) => write!(f, "@{kind}"),
            Type::Tag => write!(f, "{}", yog_ids::kind::TAG),
            Type::Game(name) => write!(f, "{}", crate::index::simple_name(name)),
            Type::List(t) => write!(f, "List<{t}>"),
            Type::Map(k, v) => write!(f, "Dictionary<{k}, {v}>"),
            Type::Tuple(items) => {
                let items: Vec<_> = items.iter().map(ToString::to_string).collect();
                write!(f, "({})", items.join(", "))
            }
            Type::Fn(params, ret) => {
                let mut all: Vec<String> = params.iter().map(ToString::to_string).collect();
                let word = if **ret == Type::Void {
                    super::written::ACTION
                } else {
                    all.push(ret.to_string());
                    super::written::FUNC
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

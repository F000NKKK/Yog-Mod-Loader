//! Types as the checker knows them, how they're shown, and which convert
//! to which without a cast (`ys-grammar.md` §8).

use std::fmt;

/// A type.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Long,
    Float,
    Double,
    Byte,
    Char,
    Bool,
    String,
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

    /// A number: its place in widening order (`int` → `long` → `float` → `double`).
    fn numeric_rank(&self) -> Option<u8> {
        match self {
            Type::Byte => Some(0),
            Type::Char => Some(1),
            Type::Int => Some(2),
            Type::Long => Some(3),
            Type::Float => Some(4),
            Type::Double => Some(5),
            _ => None,
        }
    }

    pub fn is_numeric(&self) -> bool {
        self.numeric_rank().is_some() || matches!(self, Type::Unit(_))
    }

    /// Whether a value of `self` goes where `to` is wanted without a cast.
    pub fn converts_to(&self, to: &Type) -> bool {
        if self == to || !self.known() || !to.known() {
            return true;
        }
        match (self, to) {
            (Type::Null, Type::Nullable(_)) => true,
            (Type::Nullable(a), Type::Nullable(b)) => a.converts_to(b),
            (a, Type::Nullable(b)) => a.converts_to(b),
            (Type::Unit(a), Type::Unit(b)) => a == SECONDS && b == TICKS,
            // A plain number is a quantity of any unit (`20` ticks), as
            // literals are; quantities of different units never mix.
            (n, Type::Unit(_)) => n.numeric_rank().is_some(),
            (Type::Unit(_), n) => n
                .numeric_rank()
                .is_some_and(|r| r >= Type::Int.numeric_rank().unwrap_or(0)),
            (Type::String, Type::Text) => true,
            (Type::List(a), Type::List(b)) => a.converts_to(b),
            (a, b) => match (a.numeric_rank(), b.numeric_rank()) {
                (Some(x), Some(y)) => x <= y,
                _ => false,
            },
        }
    }

    /// The type both of two numbers become in an operation (`int + double` is `double`).
    pub fn wider(a: &Type, b: &Type) -> Type {
        match (a, b) {
            (Type::Unit(_), _) => a.clone(),
            (_, Type::Unit(_)) => b.clone(),
            _ if a.numeric_rank() >= b.numeric_rank() => a.clone(),
            _ => b.clone(),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::Long => write!(f, "long"),
            Type::Float => write!(f, "float"),
            Type::Double => write!(f, "double"),
            Type::Byte => write!(f, "byte"),
            Type::Char => write!(f, "char"),
            Type::Bool => write!(f, "bool"),
            Type::String => write!(f, "string"),
            Type::Void => write!(f, "void"),
            Type::Null => write!(f, "null"),
            Type::Text => write!(f, "Text"),
            Type::Unit(u) | Type::Own(u) | Type::Yog(u) | Type::Namespace(u) => write!(f, "{u}"),
            Type::Content(kind) => write!(f, "@{kind}"),
            Type::Tag => write!(f, "{}", yog_ids::kind::TAG),
            Type::Game(name) => write!(f, "{}", crate::index::simple_name(name)),
            Type::List(t) => write!(f, "List<{t}>"),
            Type::Map(k, v) => write!(f, "Dictionary<{k}, {v}>"),
            Type::Tuple(items) => {
                let items: Vec<_> = items.iter().map(ToString::to_string).collect();
                write!(f, "({})", items.join(", "))
            }
            Type::Nullable(t) => write!(f, "{t}?"),
            Type::Unknown => write!(f, "?"),
        }
    }
}

#[cfg(test)]
mod tests;

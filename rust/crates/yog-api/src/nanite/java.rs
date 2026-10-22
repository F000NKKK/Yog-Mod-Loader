//! Rust types as Java's: each one's descriptor and how its values cross.

use yog_data::Value;

use super::{Handle, Nanite};

/// A Java type a nanite's member takes or gives.
pub trait Java: Sized {
    /// Its descriptor (`I`, `Ljava/lang/String;`).
    fn desc() -> String;
    fn to_value(&self) -> Value;
    fn from_value(v: Value) -> Option<Self>;
}

/// What a member's call gives back, from its reply (`None`: it failed, or
/// gave null): a number reads as 0 then, an `Option` as `None`.
pub trait Reply: Sized {
    /// Its descriptor as a return type.
    fn desc() -> String;
    fn from_reply(v: Option<Value>) -> Self;
}

/// The descriptors of Java's own types.
const VOID: &str = "V";
const STRING: &str = "Ljava/lang/String;";

macro_rules! ints {
    ($($t:ty => $desc:literal),* $(,)?) => {$(
        impl Java for $t {
            fn desc() -> String {
                $desc.to_owned()
            }
            fn to_value(&self) -> Value {
                Value::Int(i64::from(*self))
            }
            fn from_value(v: Value) -> Option<Self> {
                match v {
                    Value::Int(n) => <$t>::try_from(n).ok(),
                    _ => None,
                }
            }
        }
    )*};
}

ints!(i8 => "B", i16 => "S", u16 => "C", i32 => "I", i64 => "J");

macro_rules! floats {
    ($($t:ty => $desc:literal),* $(,)?) => {$(
        impl Java for $t {
            fn desc() -> String {
                $desc.to_owned()
            }
            fn to_value(&self) -> Value {
                Value::Float(f64::from(*self))
            }
            fn from_value(v: Value) -> Option<Self> {
                match v {
                    // The narrowing is Java's own (a float handed back as a double).
                    Value::Float(f) => Some(f as $t),
                    Value::Int(n) => Some(n as $t),
                    _ => None,
                }
            }
        }
    )*};
}

floats!(f32 => "F", f64 => "D");

impl Java for bool {
    fn desc() -> String {
        "Z".to_owned()
    }
    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }
    fn from_value(v: Value) -> Option<Self> {
        match v {
            Value::Bool(b) => Some(b),
            _ => None,
        }
    }
}

impl Java for String {
    fn desc() -> String {
        STRING.to_owned()
    }
    fn to_value(&self) -> Value {
        Value::Str(self.clone())
    }
    fn from_value(v: Value) -> Option<Self> {
        match v {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// A class's descriptor: `L<internal name>;`.
pub(super) fn class_desc(class: &str) -> String {
    format!("L{};", class.replace('.', "/"))
}

impl<T: Nanite> Java for T {
    fn desc() -> String {
        class_desc(T::CLASS)
    }
    fn to_value(&self) -> Value {
        self.handle().to_value()
    }
    fn from_value(v: Value) -> Option<Self> {
        Handle::from_value(v).map(T::from_handle)
    }
}

/// Every Java value is a reply: missing reads as its default.
macro_rules! replies {
    ($($t:ty),* $(,)?) => {$(
        impl Reply for $t {
            fn desc() -> String {
                <$t as Java>::desc()
            }
            fn from_reply(v: Option<Value>) -> Self {
                v.and_then(<$t as Java>::from_value).unwrap_or_default()
            }
        }
    )*};
}

replies!(i8, i16, u16, i32, i64, f32, f64, bool, String);

impl Reply for () {
    fn desc() -> String {
        VOID.to_owned()
    }
    fn from_reply(_: Option<Value>) -> Self {}
}

/// An object (or a null) given back.
impl<T: Java> Reply for Option<T> {
    fn desc() -> String {
        T::desc()
    }
    fn from_reply(v: Option<Value>) -> Self {
        v.and_then(T::from_value)
    }
}

/// A nullable object: `None` crosses as null.
impl<T: Java> Java for Option<T> {
    fn desc() -> String {
        T::desc()
    }
    fn to_value(&self) -> Value {
        self.as_ref().map_or(Value::Null, Java::to_value)
    }
    fn from_value(v: Value) -> Option<Self> {
        Some(T::from_value(v))
    }
}

//! The built-in types by their words (`i32` is `System.Int32`, …) and the
//! numbers' implicit conversions, as C#'s (§10.2.3 of its spec).

use yog_script_ids::clr;

use super::model::Type;
use crate::words::{self, Word};

/// Each built-in type, its word and its type's name in `System` (C#'s:
/// `i32` is `System.Int32`, `string` `System.String`): the one place a
/// word names a type.
static BUILTINS: &[(&Word, Type, &str)] = &[
    (&words::i8::WORD, Type::I8, clr::SBYTE),
    (&words::i16::WORD, Type::I16, clr::INT16),
    (&words::i32::WORD, Type::I32, clr::INT32),
    (&words::i64::WORD, Type::I64, clr::INT64),
    (&words::u8::WORD, Type::U8, clr::BYTE),
    (&words::u16::WORD, Type::U16, clr::UINT16),
    (&words::u32::WORD, Type::U32, clr::UINT32),
    (&words::u64::WORD, Type::U64, clr::UINT64),
    (&words::f32::WORD, Type::F32, clr::SINGLE),
    (&words::f64::WORD, Type::F64, clr::DOUBLE),
    (&words::decimal::WORD, Type::Decimal, clr::DECIMAL),
    (&words::char::WORD, Type::Char, clr::CHAR),
    (&words::bool::WORD, Type::Bool, clr::BOOLEAN),
    (&words::string::WORD, Type::String, clr::STRING),
    (&words::object::WORD, Type::Object, clr::OBJECT),
    (&words::dynamic::WORD, Type::Dynamic, clr::OBJECT),
    (&words::void::WORD, Type::Void, clr::VOID),
];

/// The built-in type its keyword names.
pub(super) fn by_word(name: &str) -> Option<Type> {
    BUILTINS
        .iter()
        .find(|(w, ..)| w.text == name)
        .map(|(_, t, _)| t.clone())
}

/// A built-in type's keyword.
pub(super) fn word(t: &Type) -> Option<&'static str> {
    BUILTINS
        .iter()
        .find(|(_, b, _)| b == t)
        .map(|(w, ..)| w.text)
}

/// The built-in type `System`'s type `name` is (`String` is `string`).
pub(super) fn by_system_name(name: &str) -> Option<Type> {
    BUILTINS
        .iter()
        .find(|(_, t, n)| *n == name && *t != Type::Dynamic)
        .map(|(_, t, _)| t.clone())
}

/// A built-in type's type in `System` (`string`'s `String`).
pub(super) fn system_name(t: &Type) -> Option<&'static str> {
    BUILTINS.iter().find(|(_, b, _)| b == t).map(|(.., n)| *n)
}

/// The numbers a number converts to without a cast.
fn widens_to(t: &Type) -> &'static [Type] {
    match t {
        Type::I8 => &[
            Type::I16,
            Type::I32,
            Type::I64,
            Type::F32,
            Type::F64,
            Type::Decimal,
        ],
        Type::U8 => &[
            Type::I16,
            Type::U16,
            Type::I32,
            Type::U32,
            Type::I64,
            Type::U64,
            Type::F32,
            Type::F64,
            Type::Decimal,
        ],
        Type::I16 => &[Type::I32, Type::I64, Type::F32, Type::F64, Type::Decimal],
        Type::U16 | Type::Char => &[
            Type::I32,
            Type::U32,
            Type::I64,
            Type::U64,
            Type::F32,
            Type::F64,
            Type::Decimal,
        ],
        Type::I32 => &[Type::I64, Type::F32, Type::F64, Type::Decimal],
        Type::U32 => &[Type::I64, Type::U64, Type::F32, Type::F64, Type::Decimal],
        Type::I64 | Type::U64 => &[Type::F32, Type::F64, Type::Decimal],
        Type::F32 => &[Type::F64],
        _ => &[],
    }
}

/// Whether `t` is a plain number (not a quantity).
pub(super) fn is_number(t: &Type) -> bool {
    t == &Type::F64 || t == &Type::Decimal || !widens_to(t).is_empty()
}

/// Whether `t` is a whole number (`char` among them, as C#'s).
pub(super) fn is_integer(t: &Type) -> bool {
    is_number(t) && !matches!(t, Type::F32 | Type::F64 | Type::Decimal)
}

/// Whether number `from` goes where number `to` is wanted.
pub(super) fn widens(from: &Type, to: &Type) -> bool {
    from == to || widens_to(from).contains(to)
}

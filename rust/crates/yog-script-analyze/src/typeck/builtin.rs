//! The built-in types by their words (`i32` is `System.Int32`, …) and the
//! numbers' implicit conversions, as C#'s (§10.2.3 of its spec).

use super::model::Type;
use crate::words::{self, Word};

/// Each built-in type and its word: the one place a word names a type.
static BUILTINS: &[(&Word, Type)] = &[
    (&words::i8::WORD, Type::I8),
    (&words::i16::WORD, Type::I16),
    (&words::i32::WORD, Type::I32),
    (&words::i64::WORD, Type::I64),
    (&words::u8::WORD, Type::U8),
    (&words::u16::WORD, Type::U16),
    (&words::u32::WORD, Type::U32),
    (&words::u64::WORD, Type::U64),
    (&words::f32::WORD, Type::F32),
    (&words::f64::WORD, Type::F64),
    (&words::decimal::WORD, Type::Decimal),
    (&words::char::WORD, Type::Char),
    (&words::bool::WORD, Type::Bool),
    (&words::string::WORD, Type::String),
    (&words::object::WORD, Type::Object),
    (&words::dynamic::WORD, Type::Dynamic),
    (&words::void::WORD, Type::Void),
];

/// The built-in type its keyword names.
pub(super) fn by_word(name: &str) -> Option<Type> {
    BUILTINS
        .iter()
        .find(|(w, _)| w.text == name)
        .map(|(_, t)| t.clone())
}

/// A built-in type's keyword.
pub(super) fn word(t: &Type) -> Option<&'static str> {
    BUILTINS.iter().find(|(_, b)| b == t).map(|(w, _)| w.text)
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

/// Whether number `from` goes where number `to` is wanted.
pub(super) fn widens(from: &Type, to: &Type) -> bool {
    from == to || widens_to(from).contains(to)
}

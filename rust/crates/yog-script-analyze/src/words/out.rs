//! `out` — a value given back.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "out",
    doc: "A parameter the method gives (`bool TryGet(string key, out i32 value)`, called `TryGet(k, out let v)`), \
          or a type parameter an interface or a delegate only gives (`interface IEnumerable<out T>`): \
          an `IEnumerable<Monster>` is an `IEnumerable<Entity>`.",
    reserved: true,
    roles: &[Role::Part],
};

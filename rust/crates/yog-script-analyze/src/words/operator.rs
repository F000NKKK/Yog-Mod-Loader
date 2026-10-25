//! `operator`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "operator",
    doc: "An operator of a type of one's own, as in C#: `public static Vec operator +(Vec a, Vec b) => new Vec(a.X + b.X, a.Y + b.Y);` (`+ - * / % == != < > <= >=`, unary `-` `!`). `==` and `!=` come together, as `<` and `>` do.",
    reserved: true,
    roles: &[Role::Part],
};

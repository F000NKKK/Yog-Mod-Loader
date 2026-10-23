//! `where` — a type parameter's constraint; a point's selector.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "where",
    doc: "What a type parameter must be: `T? Heaviest<T>(List<T> list) where T : Entity => …;`. \
          In a patch, which of several matching points: `at call Vec3.Add where (x > 0) (x) { … }`.",
    reserved: false,
    roles: &[Role::Part],
};

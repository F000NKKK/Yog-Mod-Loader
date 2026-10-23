//! The toolkit Yog's code generators are written with: the cases names take
//! in each language, string literals each language reads back, the header
//! every generated file opens with, an indenting writer for brace-block
//! languages, and writes that leave unchanged files (and their mtimes) be.

mod case;
mod consts;
mod header;
mod keywords;
mod lang;
mod literal;
mod write;
mod writer;

pub use case::{camel, pascal, screaming, snake, words};
pub use header::Header;
pub use lang::Lang;
pub use literal::{java_str, rust_str, ts_str};
pub use write::write_if_changed;
pub use writer::Code;

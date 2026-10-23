//! The toolkit Yog's code generators are written with — what holds in any
//! language: the traits a language implements to be generated
//! ([`Comments`], [`Literals`], [`Identifiers`], [`Blocks`]), names in each
//! case, quoting by a language's escapes, the header every generated file
//! opens with, an indenting writer, and writes that leave unchanged files be.
//! The languages themselves are their own crates (`yog-codegen-java`, …).

mod case;
mod consts;
mod header;
pub mod quote;
mod syntax;
mod write;
mod writer;

pub use case::{camel, pascal, screaming, snake, words};
pub use header::{Header, Regenerate};
pub use syntax::{Blocks, Comments, Identifiers, Literals};
pub use write::write_if_changed;
pub use writer::Code;

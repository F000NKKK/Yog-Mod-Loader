//! Reflection's side of the Java: each type's description written beside
//! it (`$TYPE`, from its declaration — Java's class knows only Java's
//! names), a type as a value (`typeof(T)`), and the runtime's hand-written
//! types' descriptions as companions the library writes.

mod attributes;
mod companions;
mod described;
mod invokers;
mod members;
mod values;

pub(crate) use companions::Companions;
pub(crate) use values::of_class;

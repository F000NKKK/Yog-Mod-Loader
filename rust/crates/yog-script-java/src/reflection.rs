//! Reflection's side of the Java: each type's description written beside
//! it (`$TYPE`, from its declaration — Java's class knows only Java's
//! names), a type as a value (`typeof(T)`), and the runtime's hand-written
//! types' descriptions as companions the library writes.

mod companions;
mod described;
mod values;

pub(crate) use companions::Companions;

//! Yog Script's names, each written once: what the analyzer, the
//! compilers and the tools refer to by name — the System library's types,
//! attributes and namespaces (`system`), the CLR's names of members
//! (`clr`), the library's members tools run by hand (`members`), the
//! game's units (`units`), the Java language's words (`java`), and where
//! the runtime is on the JVM (`jvm`) —
//! a name under another composed of it by literal macros (`concat!`). A language's own syntax stays with its reader or writer.

pub mod clr;
pub mod java;
pub mod jvm;
pub mod members;
pub mod rust;
pub mod system;
pub mod units;

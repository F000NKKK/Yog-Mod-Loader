//! Yog Script's names, each written once: what the analyzer, the
//! compilers and the tools refer to by name — the System library's types,
//! attributes and namespaces (`system`), the CLR's names of members
//! (`clr`), the library's members tools run by hand (`members`), the
//! game's units (`units`), and where the runtime is on the JVM (`jvm`). A language's own syntax stays with its reader or writer.

pub mod clr;
pub mod jvm;
pub mod members;
pub mod system;
pub mod units;

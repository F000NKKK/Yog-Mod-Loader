//! What mods call that isn't a message: `YogApi`'s registration functions,
//! machines, the client's own asks; the Java mods' bus (tests).

pub(crate) mod client;
pub(crate) mod fns;
#[cfg(test)]
pub(crate) mod jvm;
pub(crate) mod machine;

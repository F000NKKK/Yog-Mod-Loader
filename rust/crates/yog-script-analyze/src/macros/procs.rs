//! Procedural macros (`ys.md` §3, Attributes and macros): written in Yog
//! Script and run by whoever builds — this crate only asks. A call
//! `Name!(…)` no declared macro takes, and each `[Derive(Name)]` on a type,
//! are handed over as text; the code given back is read in their place.

/// What runs the procedural macros a build has.
pub trait ProcMacros {
    /// `name!(input)`: the code it writes, or why it failed; `None`: no
    /// such macro.
    fn call(&self, name: &str, input: &str) -> Option<Result<String, String>>;

    /// `[Derive(name)]` on the declaration written `item`: the code added
    /// after it.
    fn derive(&self, name: &str, item: &str) -> Option<Result<String, String>>;
}

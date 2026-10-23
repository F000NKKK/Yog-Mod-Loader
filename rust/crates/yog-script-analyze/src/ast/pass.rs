//! How a value is handed to a method (`ys.md` §3, Ownership): C#'s `in`,
//! `ref` and `out`, and Rust's moves. Written at both ends — the parameter
//! and the argument — so a reader of either sees it.

/// A parameter's or an argument's way of passing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    /// `in`: lent to read (Rust's `&T`).
    In,
    /// `ref`: lent to change; the caller's must be `let mut` (`&mut T`).
    Ref,
    /// `out`: the method gives it (`out let x` at the call).
    Out,
    /// `move`: owned from then on; the caller's is gone after.
    Move,
}

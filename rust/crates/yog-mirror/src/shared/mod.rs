//! Shared blocks as readers see them, the seqlock guarding them, the host's own mirrors.

pub mod host;
pub(crate) mod seq;
pub mod view;

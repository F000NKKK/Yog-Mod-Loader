//! When a handler runs: before or after the game does it.

/// Whether a handler is running before or after the action.
///
/// In `Pre` phase the handler's return value may cancel the action.
/// In `Post` phase the return value is ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    Pre,
    Post,
}

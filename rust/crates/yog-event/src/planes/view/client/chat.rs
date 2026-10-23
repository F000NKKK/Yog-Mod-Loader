//! What the player sends to the chat.

use yog_data::Schema;

/// A chat line or command the player sends (`/` and all). Before: a
/// handler that cancels it keeps it from the server — a bot's own
/// commands typed into the chat.
#[derive(Debug, Clone, Schema)]
pub struct Send {
    pub text: String,
}

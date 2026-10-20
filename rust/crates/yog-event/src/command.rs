//! Commands mods declare: what the host registers with the game.

use yog_data::Schema;

/// A command a mod handles (the `command/declare` service): its name, and its arguments' types
/// (`int player`, see `yog_command::Fit`) — `None`: plain, whatever is
/// typed. A name may be declared with several schemas.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct CommandSpec {
    pub name: String,
    pub schema: Option<String>,
}

/// Every declared command, for the host to register.
#[derive(Debug, Clone, Schema)]
pub struct Declared {}

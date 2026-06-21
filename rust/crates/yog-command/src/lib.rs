//! Command domain — the context passed to a command handler.
//!
//! A mod registers a command by name on the registry; when a player runs
//! `/<name> <args>`, its handler receives a [`CommandContext`] and may return a
//! reply string sent back to the source.

/// Details of a command invocation.
#[derive(Debug, Clone)]
pub struct CommandContext {
    /// Command name that was run, without the leading slash.
    pub name: String,
    /// Raw argument string after the command name (empty if none).
    pub args: String,
    /// Name of the player (or console) that ran the command.
    pub source: String,
    /// UUID of the executing entity (empty if run from the console).
    pub uuid: String,
}

impl CommandContext {
    /// Arguments split on whitespace, ignoring empty fields.
    pub fn arg_list(&self) -> Vec<&str> {
        self.args.split_whitespace().collect()
    }
}

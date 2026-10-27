//! Mods' commands run.

use yog_data::Schema;

/// A mod's command run; keyed by its name. Its handlers (subcommands of
/// different shapes) are tried in order — the first to answer sets `reply`,
/// what the source is told.
#[derive(Debug, Clone, Schema)]
pub struct Run {
    /// The command, without the slash.
    pub name: String,
    /// Its arguments: a typed command's values in order, a plain one's words.
    pub args: Vec<String>,
    /// Who ran it: a player's name, or the console.
    pub source: String,
    /// The running entity's uuid; empty for the console.
    pub uuid: String,
    /// Where it ran; empty for the console.
    pub dimension: String,
    /// Its permission level, as the game has it (0 anyone … 4 the
    /// server's owners; the console all of them).
    pub permission: i32,
    /// Whether it's an operator: the level the game's operators' commands
    /// want, by the game's own measure.
    pub op: bool,
    pub reply: Option<String>,
}

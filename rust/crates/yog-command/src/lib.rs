//! Command domain — the context passed to a command handler.
//!
//! A mod registers a command by name on the registry; when a player runs
//! `/<name> <args>`, its handler receives a [`CommandContext`] and may return a
//! reply string sent back to the source.

mod fit;

pub use fit::Fit;

/// Details of a command invocation.
#[derive(Debug, Clone)]
pub struct CommandContext {
    /// Command name that was run, without the leading slash.
    pub name: String,
    /// Its arguments: a typed command's values in order, a plain one's words.
    pub args: Vec<String>,
    /// Name of the player (or console) that ran the command.
    pub source: String,
    /// UUID of the executing entity (empty if run from the console).
    pub uuid: String,
    /// Dimension the source is in, empty for console-run commands (no world).
    pub dimension: String,
}

impl CommandContext {
    /// Argument `idx`, or `None` if out of range.
    pub fn arg_str(&self, idx: usize) -> Option<&str> {
        self.args.get(idx).map(String::as_str)
    }

    /// Typed argument `idx` parsed as `i32`, or `None`.
    pub fn arg_int(&self, idx: usize) -> Option<i32> {
        self.arg_str(idx)?.parse().ok()
    }

    /// Typed argument `idx` parsed as `f32`, or `None`.
    pub fn arg_float(&self, idx: usize) -> Option<f32> {
        self.arg_str(idx)?.parse().ok()
    }

    /// Typed argument `idx` as a player name, or `None`.
    pub fn arg_player(&self, idx: usize) -> Option<&str> {
        self.arg_str(idx)
    }

    /// Typed argument `idx` as a block position (`"x,y,z"`), or `None`.
    pub fn arg_blockpos(&self, idx: usize) -> Option<(i32, i32, i32)> {
        let s = self.arg_str(idx)?;
        let mut it = s.split(yog_ids::sep::LIST);
        let x = it.next()?.parse().ok()?;
        let y = it.next()?.parse().ok()?;
        let z = it.next()?.parse().ok()?;
        Some((x, y, z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(args: &[&str]) -> CommandContext {
        CommandContext {
            name: "hex".into(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            source: "Dev".into(),
            uuid: String::new(),
            dimension: String::new(),
        }
    }

    #[test]
    fn typed_arguments() {
        let t = ctx(&["5", "2.5", "Steve", "1,-2,3"]);
        assert_eq!(t.arg_int(0), Some(5));
        assert_eq!(t.arg_float(1), Some(2.5));
        assert_eq!(t.arg_player(2), Some("Steve"));
        assert_eq!(t.arg_blockpos(3), Some((1, -2, 3)));
        assert_eq!(t.arg_int(2), None, "not a number");
        assert_eq!(t.arg_str(9), None);
        assert_eq!(ctx(&[]).arg_str(0), None);
        assert_eq!(ctx(&["1,2"]).arg_blockpos(0), None);
    }
}

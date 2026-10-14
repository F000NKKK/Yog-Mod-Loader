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
    /// Dimension the source is in, empty for console-run commands (no world).
    pub dimension: String,
}

impl CommandContext {
    /// Arguments split on whitespace — for plain (untyped) commands.
    pub fn arg_list(&self) -> Vec<&str> {
        self.args.split_whitespace().collect()
    }

    // ── typed-command helpers ─────────────────────────────────────────────────
    //
    // Typed commands (registered with `Registry::on_typed_command`) receive
    // tab-separated argument values.  Use these helpers to extract them.

    /// Arguments split on `\t` — for typed commands registered with a schema.
    pub fn typed_args(&self) -> Vec<&str> {
        if self.args.is_empty() {
            vec![]
        } else {
            self.args.split('\t').collect()
        }
    }

    /// The raw string of typed argument at `idx`, or `None` if out of range.
    pub fn arg_str(&self, idx: usize) -> Option<&str> {
        if self.args.is_empty() {
            return None;
        }
        self.args.split('\t').nth(idx)
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
        let mut it = s.split(',');
        let x = it.next()?.parse().ok()?;
        let y = it.next()?.parse().ok()?;
        let z = it.next()?.parse().ok()?;
        Some((x, y, z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(args: &str) -> CommandContext {
        CommandContext { name: "hex".into(), args: args.into(), source: "Dev".into(), uuid: String::new(), dimension: String::new() }
    }

    #[test]
    fn plain_and_typed_arguments() {
        assert_eq!(ctx("  a  b ").arg_list(), ["a", "b"]);
        let t = ctx("5\t2.5\tSteve\t1,-2,3");
        assert_eq!(t.typed_args().len(), 4);
        assert_eq!(t.arg_int(0), Some(5));
        assert_eq!(t.arg_float(1), Some(2.5));
        assert_eq!(t.arg_player(2), Some("Steve"));
        assert_eq!(t.arg_blockpos(3), Some((1, -2, 3)));
        assert_eq!(t.arg_int(2), None, "not a number");
        assert_eq!(t.arg_str(9), None);
        assert!(ctx("").typed_args().is_empty());
        assert_eq!(ctx("").arg_str(0), None);
        assert_eq!(ctx("1,2").arg_blockpos(0), None);
    }
}

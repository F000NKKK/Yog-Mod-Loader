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

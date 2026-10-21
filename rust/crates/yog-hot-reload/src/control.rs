//! The control socket's protocol, between a game and whatever launched it
//! (Yog-IDLE, `dev.sh`): JSON lines, each a [`Command`] to the game or an
//! [`Event`] from it. The launcher sets `yog_ids::env::CONTROL_SOCKET` and
//! `yog_ids::env::CONTROL_TOKEN`; its first line is [`Command::Hello`]
//! with the token, answered by [`Event::Ready`].

use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// What the launcher asks the game.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// The launcher proves it's the one (the token), and who may trace the game (its pid).
    Hello { token: String, pid: u32 },
    /// Swap in mod `mod_id`'s new `.yog`; with `resources`, the game rereads its data and resources too.
    HotReload {
        mod_id: String,
        yog_path: PathBuf,
        #[serde(default)]
        resources: bool,
    },
    /// What the new `.yog` registers differently from the game as it started.
    HotReloadCheck { mod_id: String, yog_path: PathBuf },
    /// The game rereads its resources and data (F3+T, `/reload`).
    ReloadResources,
    /// The mods the game runs now.
    ListMods,
}

/// What the game tells the launcher.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Event {
    /// The answer to [`Command::Hello`]: the game's pid, its mods, and
    /// whether the launcher may now trace it (`None` from a runtime that
    /// predates telling).
    Ready {
        pid: u32,
        mods: Vec<ModInfo>,
        #[serde(default)]
        ptracer: Option<bool>,
    },
    /// [`Command::HotReload`] done, or refused.
    HotReloadDone { mod_id: String, ok: bool },
    /// The answer to [`Command::HotReloadCheck`].
    HotReloadCheck {
        mod_id: String,
        #[serde(flatten)]
        changes: Changes,
    },
    /// [`Command::ReloadResources`] done.
    ResourcesReloaded,
    /// The answer to [`Command::ListMods`].
    Mods { mods: Vec<ModInfo> },
}

/// A mod the game runs.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ModInfo {
    pub id: String,
    pub name: String,
    pub version: String,
}

/// What a new build registers that only a restart applies — `<kind> <id>` each.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Changes {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

/// `message` as one line of the protocol, its newline included.
pub fn line<T: Serialize>(message: &T) -> String {
    let mut line = serde_json::to_string(message).unwrap_or_default();
    line.push('\n');
    line
}

/// A line of the protocol read back; `None` for anything else.
pub fn parse<T: DeserializeOwned>(line: &str) -> Option<T> {
    serde_json::from_str(line.trim()).ok()
}

#[cfg(test)]
mod tests;

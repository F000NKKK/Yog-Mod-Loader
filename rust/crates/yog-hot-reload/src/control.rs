//! The control channel's protocol, between a game and whatever launched it
//! (Yog-IDLE, `dev.sh` through `yog control`): frames of binary Yog Data
//! (`yog_data::io`), each a [`Command`] to the game or an [`Event`] from it.
//! The launcher sets `yog_ids::env::CONTROL_SOCKET` and
//! `yog_ids::env::CONTROL_TOKEN`; its first frame is [`Command::Hello`]
//! with the token, answered by [`Event::Ready`] ([`Client::connect`]).

use std::io;
use std::net::TcpStream;
use std::path::PathBuf;

use yog_data::Schema;

/// What the launcher asks the game.
#[derive(Debug, Clone, PartialEq, Schema)]
#[data(tag = "cmd")]
pub enum Command {
    /// The launcher proves it's the one (the token), and who may trace the game (its pid).
    Hello { token: String, pid: u32 },
    /// Swap in mod `mod_id`'s new `.yog`; with `resources`, the game rereads its data and resources too.
    HotReload {
        mod_id: String,
        yog_path: PathBuf,
        #[data(default)]
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
#[derive(Debug, Clone, PartialEq, Schema)]
#[data(tag = "event")]
pub enum Event {
    /// The answer to [`Command::Hello`]: the game's pid, its mods, and
    /// whether the launcher may now trace it.
    Ready {
        pid: u32,
        mods: Vec<ModInfo>,
        ptracer: bool,
    },
    /// [`Command::HotReload`] done, or refused.
    HotReloadDone { mod_id: String, ok: bool },
    /// The answer to [`Command::HotReloadCheck`].
    HotReloadCheck { mod_id: String, changes: Changes },
    /// [`Command::ReloadResources`] done.
    ResourcesReloaded,
    /// The answer to [`Command::ListMods`].
    Mods { mods: Vec<ModInfo> },
}

/// A mod the game runs.
// Serde: Yog-IDLE hands it to its webview.
#[derive(Debug, Clone, PartialEq, Eq, Default, Schema, serde::Serialize)]
pub struct ModInfo {
    pub id: String,
    pub name: String,
    pub version: String,
}

/// What a new build registers that only a restart applies — `<kind> <id>` each.
// Serde: Yog-IDLE hands it to its webview.
#[derive(Debug, Clone, PartialEq, Eq, Default, Schema, serde::Serialize)]
pub struct Changes {
    #[data(default)]
    pub added: Vec<String>,
    #[data(default)]
    pub removed: Vec<String>,
    #[data(default)]
    pub changed: Vec<String>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

/// Sends `message` (a [`Command`] or an [`Event`]) as one frame.
pub fn send<T: Schema>(w: &mut impl io::Write, message: &T) -> io::Result<()> {
    yog_data::io::write_frame(w, message)
}

/// The next message; `None` once the other side has gone.
pub fn receive<T: Schema>(r: &mut impl io::Read) -> io::Result<Option<T>> {
    yog_data::io::read_frame(r)
}

/// A launcher's end of the channel.
pub struct Client {
    stream: TcpStream,
}

impl Client {
    /// Connects to the game at `addr` and says hello with `token`; the
    /// client and the game's [`Event::Ready`].
    pub fn connect(addr: &str, token: &str) -> io::Result<(Client, Event)> {
        Self::hello(TcpStream::connect(addr)?, token)
    }

    /// Says hello over `stream`, a connection to the game.
    pub fn hello(stream: TcpStream, token: &str) -> io::Result<(Client, Event)> {
        let mut client = Client { stream };
        client.send(&Command::Hello {
            token: token.to_owned(),
            pid: std::process::id(),
        })?;
        let ready = client
            .event()?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "the game hung up"))?;
        Ok((client, ready))
    }

    pub fn send(&mut self, command: &Command) -> io::Result<()> {
        send(&mut self.stream, command)
    }

    /// The game's next event; `None` once it has gone.
    pub fn event(&mut self) -> io::Result<Option<Event>> {
        receive(&mut self.stream)
    }

    /// Sends `command`, then reads events until `answer` takes one.
    pub fn ask<T>(
        &mut self,
        command: &Command,
        mut answer: impl FnMut(Event) -> Option<T>,
    ) -> io::Result<T> {
        self.send(command)?;
        loop {
            let event = self
                .event()?
                .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "the game hung up"))?;
            if let Some(answer) = answer(event) {
                return Ok(answer);
            }
        }
    }

    /// The connection (for timeouts, or a second handle to write with).
    pub fn stream(&self) -> &TcpStream {
        &self.stream
    }
}

#[cfg(test)]
mod tests;

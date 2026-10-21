//! A control channel to whatever launched this game instance — Yog-IDLE or
//! `dev.sh`, though nothing here is specific to either (dev builds only).
//! It answers two things a launcher otherwise can't:
//!
//! - "what's the actual pid to attach a debugger to" — a launcher wrapper
//!   (`./gradlew runClient`) is very often *not* an ancestor of the real
//!   JVM once Gradle's daemon is involved. This tells the client directly.
//! - "hot-reload this mod" — the handler runs inside this process, so it
//!   just calls [`crate::hot_reload::trigger_hot_reload`].
//!
//! Opt-in: it starts only when `YOG_CONTROL_SOCKET` (a loopback address,
//! `127.0.0.1:<port>`) and `YOG_CONTROL_TOKEN` are set. TCP on loopback works
//! on every platform; the token keeps other local processes out: a client's
//! first line is `{"cmd":"hello","token":…,"pid":<its pid>}`, anything else
//! closes the connection. The protocol (`yog_hot_reload::control`) follows:
//! the runtime answers `hello` with `ready` (its pid, its mods), then each
//! command with its event.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};

use yog_hot_reload::control::{self, Command, Event, ModInfo};

/// Reads `YOG_CONTROL_SOCKET` and `YOG_CONTROL_TOKEN`; with both, listens on
/// the address (loopback only) on a background thread. Call once, after
/// `load_mods` (so `ready`'s mod list is complete) — see `nativeInit`.
pub fn start_if_requested() {
    let (Ok(addr), Ok(token)) = (
        std::env::var(yog_ids::env::CONTROL_SOCKET),
        std::env::var(yog_ids::env::CONTROL_TOKEN),
    ) else {
        return;
    };
    let addr: SocketAddr = match addr.parse() {
        Ok(a) => a,
        Err(e) => {
            yog_logging::error!("control socket: `{addr}` isn't an address ({e})");
            return;
        }
    };
    if !addr.ip().is_loopback() {
        yog_logging::error!("control socket: {addr} isn't a loopback address — not listening");
        return;
    }
    let listener = match TcpListener::bind(addr) {
        Ok(l) => l,
        Err(e) => {
            yog_logging::error!("control socket: failed to bind {addr}: {e}");
            return;
        }
    };
    std::thread::spawn(move || {
        // One client at a time (the launcher); it may reconnect.
        for stream in listener.incoming().flatten() {
            handle_client(stream, &token);
        }
    });
}

/// Lets process `pid` (the client, which proved itself with the token)
/// `ptrace` this one. Yama's default policy (`ptrace_scope = 1`) only allows
/// tracing descendants, and the game usually isn't one of the IDE's
/// (Gradle's daemon launches it) — but a process may name a tracer itself
/// (`PR_SET_PTRACER`), which needs no privileges. Without Yama, `prctl`
/// fails with `EINVAL`: nothing to lift then.
///
/// Returns whether `pid` may now trace us — reported in `ready` as
/// `ptracer`, so the client can tell an outdated runtime (no field) from a
/// policy that really refuses it.
#[cfg(target_os = "linux")]
fn allow_to_trace(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY: plain `prctl` call with integer arguments.
    let rc = unsafe { libc::prctl(libc::PR_SET_PTRACER, pid as libc::c_ulong, 0, 0, 0) };
    if rc == 0 {
        return true;
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::EINVAL) {
        return true;
    }
    yog_logging::warn!("control socket: PR_SET_PTRACER({pid}) failed: {err}");
    false
}

fn write_line(stream: &mut TcpStream, event: &Event) {
    let _ = stream.write_all(control::line(event).as_bytes());
}

fn mod_list() -> Vec<ModInfo> {
    crate::globals::MOD_INFOS
        .lock()
        .iter()
        .map(|m| ModInfo {
            id: m.id.clone(),
            name: m.name.clone(),
            version: m.version.clone(),
        })
        .collect()
}

fn handle_client(mut stream: TcpStream, token: &str) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut lines = BufReader::new(read_half).lines().map_while(Result::ok);
    if !greet(&mut stream, lines.next(), token) {
        return;
    }
    for line in lines {
        if let Some(cmd) = control::parse(&line) {
            answer(&mut stream, cmd);
        }
    }
}

/// The client proves it's the launcher first (`hello` with the token); it's
/// answered `ready`: this process, its mods, whether it may trace it.
fn greet(stream: &mut TcpStream, first: Option<String>, token: &str) -> bool {
    let Some(Command::Hello { token: theirs, pid }) = first.and_then(|l| control::parse(&l)) else {
        yog_logging::warn!("control socket: a client that didn't say hello — closed");
        return false;
    };
    if theirs != token {
        yog_logging::warn!("control socket: a client without the token — closed");
        return false;
    }
    #[cfg(target_os = "linux")]
    let ptracer = allow_to_trace(pid);
    #[cfg(not(target_os = "linux"))]
    let ptracer = {
        let _ = pid;
        false
    };
    write_line(
        stream,
        &Event::Ready {
            pid: std::process::id(),
            mods: mod_list(),
            ptracer: Some(ptracer),
        },
    );
    true
}

/// One command, answered by its event.
fn answer(stream: &mut TcpStream, cmd: Command) {
    let reply = match cmd {
        Command::HotReload {
            mod_id,
            yog_path,
            resources,
        } => {
            let ok = crate::hot_reload::trigger_hot_reload(&mod_id, &yog_path);
            if ok && resources {
                crate::registrations::reload_game_resources();
            }
            Event::HotReloadDone { mod_id, ok }
        }
        // What a new `.yog` registers differently from the game as it
        // started — changes only a restart applies.
        Command::HotReloadCheck { mod_id, yog_path } => {
            let changes = crate::registrations::changes(&mod_id, &yog_path);
            Event::HotReloadCheck { mod_id, changes }
        }
        Command::ReloadResources => {
            crate::registrations::reload_game_resources();
            Event::ResourcesReloaded
        }
        Command::ListMods => Event::Mods { mods: mod_list() },
        Command::Hello { .. } => return,
    };
    write_line(stream, &reply);
}

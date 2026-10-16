//! A control channel to whatever launched this game instance — Yog-IDLE or
//! `dev.sh`, though nothing here is specific to either (dev builds only).
//! It answers two things a launcher otherwise can't:
//!
//! - "what's the actual pid to attach a debugger to" — a launcher wrapper
//!   (`./gradlew runClient`) is very often *not* an ancestor of the real
//!   JVM once Gradle's daemon is involved. This tells the client directly.
//! - "hot-reload this mod" — the handler runs inside this process, so it
//!   just calls [`crate::trigger_hot_reload`].
//!
//! Opt-in: it starts only when `YOG_CONTROL_SOCKET` (a loopback address,
//! `127.0.0.1:<port>`) and `YOG_CONTROL_TOKEN` are set. TCP on loopback works
//! on every platform; the token keeps other local processes out: a client's
//! first line is `{"cmd":"hello","token":…,"pid":<its pid>}`, anything else
//! closes the connection. The JSON-lines protocol follows: the runtime
//! answers `hello` with `ready` (its pid, its mods), then commands
//! (`hot_reload_check`, `hot_reload`, `reload_resources`, `list_mods`) with
//! their events.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;

/// Reads `YOG_CONTROL_SOCKET` and `YOG_CONTROL_TOKEN`; with both, listens on
/// the address (loopback only) on a background thread. Call once, after
/// `load_mods` (so `ready`'s mod list is complete) — see `nativeInit`.
pub fn start_if_requested() {
    let (Ok(addr), Ok(token)) = (std::env::var("YOG_CONTROL_SOCKET"), std::env::var("YOG_CONTROL_TOKEN")) else {
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
fn allow_to_trace(pid: i64) -> bool {
    if pid <= 0 {
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

fn write_line(stream: &mut TcpStream, json: &serde_json::Value) {
    let mut line = json.to_string();
    line.push('\n');
    let _ = stream.write_all(line.as_bytes());
}

fn mod_list_json() -> Vec<serde_json::Value> {
    crate::MOD_INFOS
        .lock()
        .expect("mod infos lock poisoned")
        .iter()
        .map(|m| serde_json::json!({ "id": m[0], "name": m[1], "version": m[2] }))
        .collect()
}

fn str_of<'a>(cmd: &'a serde_json::Value, key: &str) -> &'a str {
    cmd.get(key).and_then(|v| v.as_str()).unwrap_or_default()
}

fn handle_client(mut stream: TcpStream, token: &str) {
    let Ok(read_half) = stream.try_clone() else { return };
    let mut lines = BufReader::new(read_half).lines().map_while(Result::ok);

    // The client proves it's the launcher first.
    let Some(hello) = lines.next().and_then(|l| serde_json::from_str::<serde_json::Value>(&l).ok()) else { return };
    if str_of(&hello, "cmd") != "hello" || str_of(&hello, "token") != token {
        yog_logging::warn!("control socket: a client without the token — closed");
        return;
    }
    #[cfg(target_os = "linux")]
    let ptracer = allow_to_trace(hello.get("pid").and_then(|v| v.as_i64()).unwrap_or(0));
    #[cfg(not(target_os = "linux"))]
    let ptracer = false;
    write_line(
        &mut stream,
        &serde_json::json!({
            "event": "ready",
            "pid": std::process::id(),
            "mods": mod_list_json(),
            "ptracer": ptracer,
        }),
    );

    for line in lines {
        let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        match str_of(&cmd, "cmd") {
            "hot_reload" => {
                let mod_id = str_of(&cmd, "mod_id");
                let ok = crate::trigger_hot_reload(mod_id, Path::new(str_of(&cmd, "yog_path")));
                // `"resources": true`: the game rereads its data and resources too.
                if ok && cmd.get("resources").and_then(|v| v.as_bool()).unwrap_or(false) {
                    crate::registrations::reload_game_resources();
                }
                write_line(&mut stream, &serde_json::json!({ "event": "hot-reload-done", "mod_id": mod_id, "ok": ok }));
            }
            // What a new `.yog` registers differently from the game as it
            // started — changes only a restart applies.
            "hot_reload_check" => {
                let mod_id = str_of(&cmd, "mod_id");
                let (added, removed, changed) = crate::registrations::changes(mod_id, Path::new(str_of(&cmd, "yog_path")));
                write_line(
                    &mut stream,
                    &serde_json::json!({
                        "event": "hot-reload-check", "mod_id": mod_id,
                        "added": added, "removed": removed, "changed": changed,
                    }),
                );
            }
            "reload_resources" => {
                crate::registrations::reload_game_resources();
                write_line(&mut stream, &serde_json::json!({ "event": "resources-reloaded" }));
            }
            "list_mods" => {
                write_line(&mut stream, &serde_json::json!({ "event": "mods", "mods": mod_list_json() }));
            }
            _ => {}
        }
    }
}

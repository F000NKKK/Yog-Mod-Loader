//! A control channel to whatever launched this game instance — Yog-IDLE,
//! in practice, though nothing here is Yog-IDLE-specific. Solves two
//! problems a host process previously had no real answer to:
//!
//! - "what's the actual pid to attach a debugger to" — a launcher wrapper
//!   (`./gradlew runClient`) is very often *not* an ancestor of the real
//!   JVM once Gradle's daemon is involved, making any process-tree-walking
//!   discovery fundamentally unreliable. This just tells the client
//!   directly, from inside the one process that unambiguously knows.
//! - "trigger a hot reload from outside" — previously impossible without
//!   either a JNI-attach from the external process or `ptrace` function
//!   injection. The socket handler runs *inside* this process already, so
//!   it just calls [`crate::trigger_hot_reload`] directly.
//!
//! Entirely opt-in: only starts if `YOG_CONTROL_SOCKET` is set in the
//! environment (Yog-IDLE sets it before spawning `yog run`) — a normal
//! player's game never has it set, so this is a complete no-op for them.
//!
//! Unix only: std has no Unix domain sockets on Windows, and Yog-IDLE's
//! launch/debug path is Linux-only anyway.

#[cfg(unix)]
pub use unix::start_if_requested;

/// No control channel off Unix — log once if a launcher asked for one.
#[cfg(not(unix))]
pub fn start_if_requested() {
    if std::env::var_os("YOG_CONTROL_SOCKET").is_some() {
        yog_logging::warn!("control socket: not supported on this platform");
    }
}

#[cfg(unix)]
mod unix {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::Path;

    /// Reads `YOG_CONTROL_SOCKET`; if present, binds a Unix domain socket
    /// there and spawns the accept loop on a background thread. Call once,
    /// after `load_mods` has finished (so the `ready` message's mod list is
    /// complete) — see `nativeInit`.
    pub fn start_if_requested() {
        let Ok(path) = std::env::var("YOG_CONTROL_SOCKET") else {
            return;
        };

        // A crashed previous run can leave the socket file behind; bind fails
        // on an existing path otherwise.
        let _ = std::fs::remove_file(&path);

        let listener = match UnixListener::bind(&path) {
            Ok(l) => l,
            Err(e) => {
                yog_logging::error!("control socket: failed to bind {}: {}", path, e);
                return;
            }
        };

        std::thread::spawn(move || {
            // Exactly one client is expected (Yog-IDLE, for this one launch) —
            // subsequent connection attempts after it drops are accepted too,
            // in case the client reconnects, but there's no fan-out to several
            // clients at once.
            for stream in listener.incoming().flatten() {
                handle_client(stream);
            }
        });
    }

    /// Lets the connected client `ptrace` this process. Yama's default
    /// policy (`ptrace_scope = 1`) only allows tracing descendants, and the
    /// game usually isn't one of the IDE's (Gradle's daemon launches it) —
    /// but a process may name a tracer itself (`PR_SET_PTRACER`), which needs
    /// no privileges or system settings. The peer is taken from the socket
    /// (`SO_PEERCRED`), and only a client running as our own user counts —
    /// it could `ptrace` us without Yama anyway. Without Yama, `prctl`
    /// fails with `EINVAL`: nothing to lift then.
    ///
    /// Returns whether the peer may now trace us — reported in `ready` as
    /// `ptracer`, so the client can tell an outdated runtime (no field) from
    /// a policy that really refuses it.
    #[cfg(target_os = "linux")]
    fn allow_peer_to_trace(stream: &UnixStream) -> bool {
        use std::os::fd::AsRawFd;

        let mut cred = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: `cred`/`len` are a valid out-buffer of the size passed.
        let rc = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut cred as *mut libc::ucred).cast(),
                &mut len,
            )
        };
        // SAFETY: `getuid` has no preconditions.
        if rc != 0 || cred.pid <= 0 || cred.uid != unsafe { libc::getuid() } {
            return false;
        }
        // SAFETY: plain `prctl` call with integer arguments.
        let rc = unsafe { libc::prctl(libc::PR_SET_PTRACER, cred.pid as libc::c_ulong, 0, 0, 0) };
        if rc == 0 {
            return true;
        }
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EINVAL) {
            return true;
        }
        yog_logging::warn!(
            "control socket: PR_SET_PTRACER({}) failed: {}",
            cred.pid,
            err
        );
        false
    }

    fn write_line(stream: &mut UnixStream, json: &serde_json::Value) {
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

    fn handle_client(mut stream: UnixStream) {
        #[cfg(target_os = "linux")]
        let ptracer = allow_peer_to_trace(&stream);
        #[cfg(not(target_os = "linux"))]
        let ptracer = false;
        let ready = serde_json::json!({
            "event": "ready",
            "pid": std::process::id(),
            "mods": mod_list_json(),
            "ptracer": ptracer,
        });
        write_line(&mut stream, &ready);

        let reader = match stream.try_clone() {
            Ok(s) => BufReader::new(s),
            Err(_) => return,
        };

        for line in reader.lines().map_while(Result::ok) {
            let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            match cmd.get("cmd").and_then(|c| c.as_str()) {
                Some("hot_reload") => {
                    let mod_id = cmd
                        .get("mod_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let yog_path = cmd
                        .get("yog_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let ok = crate::trigger_hot_reload(mod_id, Path::new(yog_path));
                    let response = serde_json::json!({ "event": "hot-reload-done", "mod_id": mod_id, "ok": ok });
                    write_line(&mut stream, &response);
                }
                Some("list_mods") => {
                    let response = serde_json::json!({ "event": "mods", "mods": mod_list_json() });
                    write_line(&mut stream, &response);
                }
                _ => {}
            }
        }
    }
}

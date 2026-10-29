//! A minimal Debug Adapter Protocol (DAP) transport + request dispatch —
//! just the message framing and the handful of requests this pass's
//! [`Debugger`](crate::Debugger) actually needs to serve
//! (`initialize`/`attach`/`setBreakpoints`/`continue`/`next`/`stepIn`/
//! `stackTrace`/`threads`/`disconnect`). No `variables`/`evaluate` yet —
//! those need full DWARF location-expression evaluation, out of scope for
//! this pass.
//!
//! Transport framing is provided by Substrate; this module only dispatches
//! Yog debugger requests.

use std::io::{BufRead, Write};

use serde_json::{Value, json};
use substrate_framing::{read_frame, write_frame};

/// One resolved stack frame, ready to hand back as a DAP `stackTrace` entry.
pub struct StackFrameInfo {
    pub id: i64,
    pub name: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// What a concrete debugger backend needs to answer this pass's DAP request
/// subset — implemented by [`crate::YogDebugSession`] against a real
/// `ptrace` session, or by a fake in tests.
pub trait DebugSession {
    fn attach(&mut self, arguments: &Value) -> Result<(), String>;
    /// Sets breakpoints for `source_path` to exactly `lines` (clearing any
    /// previously set for that path but not in the new list), returning
    /// the lines actually verified/armed.
    fn set_breakpoints(&mut self, source_path: &str, lines: &[u32]) -> Result<Vec<u32>, String>;
    fn continue_(&mut self) -> Result<(), String>;
    fn next(&mut self) -> Result<(), String>;
    fn step_in(&mut self) -> Result<(), String>;
    fn stack_trace(&mut self) -> Result<Vec<StackFrameInfo>, String>;
    fn threads(&self) -> Vec<(i64, String)>;
    fn disconnect(&mut self) -> Result<(), String>;
}

/// Reads DAP requests from `reader`, dispatches them to a [`DebugSession`],
/// and writes DAP responses to `writer`. Runs until `disconnect` or EOF.
pub struct Server<S: DebugSession> {
    session: S,
    seq: i64,
}

impl<S: DebugSession> Server<S> {
    pub fn new(session: S) -> Self {
        Server { session, seq: 1 }
    }

    pub fn run<R: BufRead, W: Write>(
        &mut self,
        mut reader: R,
        mut writer: W,
    ) -> std::io::Result<()> {
        while let Some(message) = read_frame(&mut reader)? {
            if message.get("type").and_then(Value::as_str) != Some("request") {
                continue;
            }
            let command = message
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let request_seq = message.get("seq").and_then(Value::as_i64).unwrap_or(0);
            let arguments = message.get("arguments").cloned().unwrap_or(Value::Null);

            let (success, body) = self.dispatch(&command, &arguments);
            self.send_response(&mut writer, request_seq, &command, success, body)?;
            if command == "disconnect" {
                break;
            }
        }
        Ok(())
    }

    fn dispatch(&mut self, command: &str, arguments: &Value) -> (bool, Value) {
        match command {
            "initialize" => (true, json!({ "supportsConfigurationDoneRequest": true })),
            "attach" => respond(self.session.attach(arguments), |()| Value::Null),
            "setBreakpoints" => {
                let path = arguments
                    .get("source")
                    .and_then(|s| s.get("path"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let lines: Vec<u32> = arguments
                    .get("breakpoints")
                    .and_then(Value::as_array)
                    .map(|entries| {
                        entries
                            .iter()
                            .filter_map(|bp| bp.get("line").and_then(Value::as_u64))
                            .map(|l| l as u32)
                            .collect()
                    })
                    .unwrap_or_default();
                respond(self.session.set_breakpoints(&path, &lines), |verified| {
                    let breakpoints: Vec<Value> = verified
                        .iter()
                        .map(|line| json!({ "verified": true, "line": line }))
                        .collect();
                    json!({ "breakpoints": breakpoints })
                })
            }
            "continue" => respond(
                self.session.continue_(),
                |()| json!({ "allThreadsContinued": true }),
            ),
            "next" => respond(self.session.next(), |()| Value::Null),
            "stepIn" => respond(self.session.step_in(), |()| Value::Null),
            "stackTrace" => respond(self.session.stack_trace(), |frames| {
                let stack_frames: Vec<Value> = frames
                    .iter()
                    .map(|f| json!({ "id": f.id, "name": f.name, "line": f.line, "column": f.column, "source": { "path": f.file } }))
                    .collect();
                let total = stack_frames.len();
                json!({ "stackFrames": stack_frames, "totalFrames": total })
            }),
            "threads" => {
                let threads: Vec<Value> = self
                    .session
                    .threads()
                    .into_iter()
                    .map(|(id, name)| json!({ "id": id, "name": name }))
                    .collect();
                (true, json!({ "threads": threads }))
            }
            "disconnect" => respond(self.session.disconnect(), |()| Value::Null),
            other => error_body(format!("unsupported DAP command: {other}")),
        }
    }

    fn send_response<W: Write>(
        &mut self,
        writer: &mut W,
        request_seq: i64,
        command: &str,
        success: bool,
        body: Value,
    ) -> std::io::Result<()> {
        let seq = self.seq;
        self.seq += 1;
        let message = json!({
            "seq": seq,
            "type": "response",
            "request_seq": request_seq,
            "success": success,
            "command": command,
            "body": body,
        });
        write_frame(writer, &message)
    }
}

/// A response for `result`: success with `ok`'s body, or a failure
/// carrying the error.
fn respond<T>(result: Result<T, String>, ok: impl FnOnce(T) -> Value) -> (bool, Value) {
    match result {
        Ok(value) => (true, ok(value)),
        Err(e) => error_body(e),
    }
}

fn error_body(message: String) -> (bool, Value) {
    (false, json!({ "error": message }))
}

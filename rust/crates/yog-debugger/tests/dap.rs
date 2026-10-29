//! The debugger dispatches DAP requests over Substrate's transport.

use std::io::{Cursor, ErrorKind};

use serde_json::{Value, json};
use substrate_framing::read_frame;
use yog_debugger::dap::{DebugSession, Server, StackFrameInfo};

#[derive(Default)]
struct FakeSession;

impl DebugSession for FakeSession {
    fn attach(&mut self, _: &Value) -> Result<(), String> {
        Ok(())
    }

    fn set_breakpoints(&mut self, _: &str, lines: &[u32]) -> Result<Vec<u32>, String> {
        Ok(lines.to_vec())
    }

    fn continue_(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn next(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn step_in(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn stack_trace(&mut self) -> Result<Vec<StackFrameInfo>, String> {
        Ok(Vec::new())
    }

    fn threads(&self) -> Vec<(i64, String)> {
        vec![(1, "main".into())]
    }

    fn disconnect(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn lowercase_headers_dispatch_requests_and_write_dap_responses() {
    let request = br#"{"seq":7,"type":"request","command":"threads"}"#;
    let mut input = Cursor::new(
        format!(
            "Content-Type: application/vscode-jsonrpc; charset=utf-8\r\ncontent-length: {}\r\n\r\n{}",
            request.len(),
            std::str::from_utf8(request).unwrap()
        )
        .into_bytes(),
    );
    let mut output = Vec::new();

    Server::new(FakeSession)
        .run(&mut input, &mut output)
        .unwrap();

    let mut output = Cursor::new(output);
    assert_eq!(
        read_frame(&mut output).unwrap(),
        Some(json!({
            "seq": 1,
            "type": "response",
            "request_seq": 7,
            "success": true,
            "command": "threads",
            "body": { "threads": [{ "id": 1, "name": "main" }] }
        }))
    );
    assert_eq!(read_frame(&mut output).unwrap(), None);
}

#[test]
fn a_truncated_header_is_an_error_instead_of_a_clean_disconnect() {
    let mut input = Cursor::new(b"Content-Length: 50\r\n");
    let mut output = Vec::new();

    let error = Server::new(FakeSession)
        .run(&mut input, &mut output)
        .unwrap_err();

    assert_eq!(error.kind(), ErrorKind::UnexpectedEof);
    assert!(output.is_empty());
}

#[test]
fn a_truncated_body_is_rejected_without_dispatch() {
    let mut input = Cursor::new(b"Content-Length: 50\r\n\r\n{}");
    let mut output = Vec::new();

    let error = Server::new(FakeSession)
        .run(&mut input, &mut output)
        .unwrap_err();

    assert_eq!(error.kind(), ErrorKind::UnexpectedEof);
    assert!(output.is_empty());
}

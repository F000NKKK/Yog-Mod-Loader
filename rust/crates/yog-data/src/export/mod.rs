//! The binary form's constants for the other languages' readers — the
//! IDE's TypeScript (`yog-data-export`), so the tags are written once.

mod types;

use crate::binary::{self, tag};
use crate::{Diagnostic, Schema, Seg, Severity};
use yog_codegen::{Header, Literals, Regenerate};
use yog_codegen_typescript::TypeScript;

const HEADER: Header = Header {
    tool: "yog-data",
    source: "Yog-Mod-Loader/rust/crates/yog-data",
    regenerate: Regenerate::Command("cargo run -p yog-data --bin yog-data-export"),
};

/// A TypeScript module: the header, the tags, the strings' and frames' rules.
pub fn typescript() -> String {
    let magic = String::from_utf8_lossy(binary::MAGIC);
    let mut out = format!(
        "{}\n\
         /** What Yog Data starts with, and its version. */\n\
         export const MAGIC = {};\n\
         export const VERSION = {};\n\
         /** Nested deeper than this is malformed. */\n\
         export const MAX_DEPTH = {};\n\
         /** A string's varint is odd for a new string (its length), even for one already read (its index). */\n\
         export const NEW_STRING = {};\n\
         /** A stream's length: this many bytes, little-endian. */\n\
         export const STREAM_LEN_BYTES = {};\n\
         /** A frame's length (`yog_data::io`): this many bytes, little-endian; and the longest frame. */\n\
         export const FRAME_LEN_BYTES = {};\n\
         export const MAX_FRAME = {};\n\
         /** A result's two sides: `{{ ok }}` or `{{ err }}`. */\n\
         export const RESULT_OK = {};\n\
         export const RESULT_ERR = {};\n\
         \n/** A value's first byte: what it is. */\nexport const tag = {{\n",
        HEADER.comment::<TypeScript>(),
        TypeScript::string(&magic),
        binary::VERSION,
        binary::MAX_DEPTH,
        binary::NEW_STRING,
        binary::STREAM_LEN_BYTES,
        size_of::<crate::io::Len>(),
        crate::io::MAX_FRAME,
        TypeScript::string(substrate_data::schema::RESULT_OK),
        TypeScript::string(substrate_data::schema::RESULT_ERR),
    );
    for (name, value) in tag::ALL {
        out.push_str(&format!("  {name}: {value},\n"));
    }
    out.push_str("} as const;\n");
    // The records every tool writes.
    let shapes = [Severity::shape(), Seg::shape(), Diagnostic::shape()];
    out.push('\n');
    out.push_str(&types::declarations(&shapes));
    out
}

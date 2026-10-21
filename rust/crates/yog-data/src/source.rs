//! A source file as it's kept and as it's read: on disk binary Yog Data
//! (what the editor saves), to people its text — formatted as Yog's editors
//! write it, so a line in an error is the line the editor shows. A file
//! still in text is read as it is.

use crate::binary::{self, MAGIC};
use crate::text::SyntaxError;

/// Whether `bytes` are binary Yog Data.
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

/// A source file's text: its binary form written out, or its text as it is.
pub fn text(bytes: &[u8]) -> Result<String, String> {
    if !is_binary(bytes) {
        return String::from_utf8(bytes.to_vec()).map_err(|e| e.to_string());
    }
    let value = binary::decode(bytes).map_err(|e| e.to_string())?;
    Ok(format!("{}\n", crate::patch::format(&value, 0)))
}

/// Text as a source file keeps it: binary, its comments kept as notes
/// ([`crate::text::with_comments`]).
pub fn binary(text: &str) -> Result<Vec<u8>, SyntaxError> {
    crate::text::with_comments(text).map(|v| binary::encode(&v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_round_trips_through_its_binary_form() {
        let text = "{\n  // Hard.\n  hardness: 3,\n  drop: \"@item:ruby\"\n}";
        let bytes = binary(text).unwrap();
        assert!(is_binary(&bytes));
        let shown = super::text(&bytes).unwrap();
        assert!(
            shown.contains("$hardness") && shown.contains("@item:ruby"),
            "{shown}"
        );
        assert_eq!(
            binary(&shown).unwrap(),
            bytes,
            "its text is the same file again"
        );
        assert_eq!(
            super::text(text.as_bytes()).unwrap(),
            text,
            "text stays text"
        );
    }
}

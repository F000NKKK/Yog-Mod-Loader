use super::*;

#[test]
fn quotes_and_escapes() {
    assert_eq!(java_str(r#"a "b" \c"#), r#""a \"b\" \\c""#);
    assert_eq!(java_str("line\nnext"), r#""line\nnext""#);
    assert_eq!(java_str("\u{e9}"), "\"\\u00e9\"");
    assert_eq!(ts_str("\u{1f600}"), "\"\\ud83d\\ude00\"");
    assert_eq!(rust_str("\u{e9}\n"), "\"\u{e9}\\n\"");
}

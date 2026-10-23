use super::*;

#[test]
fn quotes_by_the_given_escapes() {
    let escape = |c: char| (c == '"').then(|| "\\\"".to_string());
    assert_eq!(quoted(r#"a "b""#, '"', escape), r#""a \"b\"""#);
    assert_eq!(utf16_units('\u{e9}', "\\u"), "\\u00e9");
    assert_eq!(utf16_units('\u{1f600}', "\\u"), "\\ud83d\\ude00");
}

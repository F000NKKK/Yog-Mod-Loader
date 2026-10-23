use super::*;

#[test]
fn literals_and_names() {
    assert_eq!(TypeScript::string("\u{1f600}"), "\"\\ud83d\\ude00\"");
    assert_eq!(TypeScript::ident("delete"), "delete_");
}

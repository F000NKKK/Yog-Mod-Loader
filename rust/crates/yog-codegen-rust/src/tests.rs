use super::*;

#[test]
fn literals_and_names() {
    assert_eq!(Rust::string("\u{e9}\n"), "\"\u{e9}\\n\"");
    assert_eq!(Rust::ident("type"), "r#type");
}

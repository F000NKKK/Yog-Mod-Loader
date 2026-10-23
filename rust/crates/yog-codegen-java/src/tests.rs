use super::*;

#[test]
fn literals_and_names() {
    assert_eq!(Java::string(r#"a "b" \c"#), r#""a \"b\" \\c""#);
    assert_eq!(Java::string("\u{e9}\n"), "\"\\u00e9\\n\"");
    assert_eq!(Java::ident("interface"), "interface_");
    assert_eq!(Java::ident("name"), "name");
}

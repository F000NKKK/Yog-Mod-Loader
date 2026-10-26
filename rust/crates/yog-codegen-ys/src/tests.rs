use super::*;

#[test]
fn literals_and_names() {
    assert_eq!(Ys::string("a\"{b}\\\n\u{1}é"), "\"a\\\"\\{b\\}\\\\\\n\\u{1}é\"");
    assert_eq!(Ys::ident("class"), "@class");
    assert_eq!(Ys::ident("Lamp"), "Lamp");
}

#[test]
fn constants_in_a_class() {
    assert_eq!(Ys::static_class("Blocks"), "public static class Blocks");
    assert_eq!(
        Ys::const_string("Lamp", "mod:lamp"),
        "public const string Lamp = \"mod:lamp\";"
    );
}

#[test]
fn values_and_texts() {
    assert_eq!(Ys::const_value("i64", "Rate", "4"), "public const i64 Rate = 4;");
    assert_eq!(
        Ys::text_getter("Title", "charge.title"),
        "public static Text Title => t\"charge.title\";"
    );
}

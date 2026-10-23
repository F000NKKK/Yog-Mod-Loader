use super::*;
use crate::Lang;

#[test]
fn blocks_indent_what_they_hold() {
    let mut c = Code::starting(Lang::Java, "// header\n");
    c.open("public final class A")
        .doc("A name.")
        .line("static final int X = 1;")
        .close("");
    assert_eq!(
        c.finish(),
        "// header\npublic final class A {\n    /** A name. */\n    static final int X = 1;\n}\n"
    );
}

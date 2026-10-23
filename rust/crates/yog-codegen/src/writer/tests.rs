use super::*;
use crate::syntax::{Blocks, Comments, Literals};

/// A language just for the test: braces, `#` comments, `'` strings.
struct Toy;

impl Comments for Toy {
    fn comment(text: &str) -> String {
        format!("# {text}")
    }
    fn doc(text: &str) -> String {
        format!("## {text}")
    }
}

impl Literals for Toy {
    fn string(text: &str) -> String {
        format!("'{text}'")
    }
}

impl Blocks for Toy {
    const INDENT: &'static str = "  ";
    fn open(head: &str) -> String {
        format!("{head} {{")
    }
    fn close(tail: &str) -> String {
        format!("}}{tail}")
    }
}

#[test]
fn blocks_indent_what_they_hold() {
    let mut c = Code::<Toy>::starting("# header\n");
    let s = c.str("x");
    c.open("block a")
        .doc("A name.")
        .line(format!("x = {s}"))
        .close(";");
    assert_eq!(
        c.finish(),
        "# header\nblock a {\n  ## A name.\n  x = 'x'\n};\n"
    );
}

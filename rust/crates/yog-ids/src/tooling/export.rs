//! The names for the other languages: TypeScript for the IDE, Java for the hosts.

use crate::groups;
use yog_codegen::{Code, Header, Regenerate, camel, pascal};
use yog_codegen_java::Java;
use yog_codegen_typescript::TypeScript;

const HEADER: Header = Header {
    tool: "yog-ids",
    source: "Yog-Mod-Loader/rust/crates/yog-ids",
    regenerate: Regenerate::Command("cargo run -p yog-ids --bin yog-ids-export"),
};

/// The hosts' class of the names, and its package.
pub const JAVA_CLASS: &str = "YogNames";
pub const JAVA_PACKAGE: &str = "dev.yog";

/// The Java expression naming `value` in class `class`
/// (`YogNames.EventBlock.BREAK`), if a name has it.
pub fn java_ref(class: &str, value: &str) -> Option<String> {
    groups().find_map(|(group, names)| {
        names
            .iter()
            .find(|(_, v)| *v == value)
            .map(|(name, _)| format!("{class}.{}.{name}", pascal(group)))
    })
}

/// The name a format goes by in the generated code (`Format::Block` → `block`).
fn format_word(f: crate::Format) -> String {
    format!("{f:?}")
}

/// A TypeScript module: one `as const` object per group.
pub fn typescript() -> String {
    let mut c = Code::<TypeScript>::starting(HEADER.comment::<TypeScript>());
    for (group, names) in groups() {
        c.blank().open(format!("export const {} =", camel(group)));
        for (name, value) in *names {
            c.line(format!("{name}: {},", c.str(value)));
        }
        c.close(" as const;");
    }
    c.blank()
        .doc("Yog's file formats: extension, noun, and whether files of it are Yog Data — binary on disk, text in the editor (`Format`).")
        .open("export const formats =");
    for f in crate::Format::ALL {
        c.line(format!(
            "{}: {{ ext: {}, noun: {}, isData: {} }},",
            camel(&format_word(*f).to_lowercase()),
            c.str(f.ext()),
            c.str(f.noun()),
            f.is_data()
        ));
    }
    c.close(" as const;");
    c.finish()
}

/// A Java class (`package`): a nested class of constants per group.
pub fn java(package: &str, class: &str) -> String {
    let mut c = Code::<Java>::starting(HEADER.comment::<Java>());
    c.line(format!("package {package};")).blank();
    c.doc("The names Yog's code shares (yog-ids).");
    c.open(format!("public final class {class}"));
    c.line(format!("private {class}() {{}}"));
    for (group, names) in groups() {
        let nested = pascal(group);
        c.blank()
            .open(format!("public static final class {nested}"));
        c.line(format!("private {nested}() {{}}")).blank();
        for (name, value) in *names {
            c.line(format!(
                "public static final String {name} = {};",
                c.str(value)
            ));
        }
        let all: Vec<&str> = names.iter().map(|(name, _)| *name).collect();
        c.blank().doc("Every name of the group, in order.");
        c.line(format!(
            "public static final java.util.List<String> ALL_VALUES = java.util.List.of({});",
            all.join(", ")
        ));
        c.close("");
    }
    c.blank().doc("The game's units (`units`).");
    c.open("public static final class Units");
    c.line("private Units() {}").blank();
    for (name, value) in crate::units::ALL {
        c.line(format!("public static final int {name} = {value};"));
    }
    c.close("");
    c.blank().doc("Yog's file formats' extensions (`Format`).");
    c.open("public static final class Ext");
    c.line("private Ext() {}").blank();
    for f in crate::Format::ALL {
        c.line(format!(
            "public static final String {} = {};",
            format_word(*f).to_uppercase(),
            c.str(f.ext())
        ));
    }
    c.close("").close("");
    c.finish()
}

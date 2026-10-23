//! The bus's messages for Java: a record per event (`dev.yog.bus.YogEvents`)
//! and per service (`YogServices`), from the same schemas, that encodes
//! itself and goes on the bus — the host fires events and calls services
//! through these instead of a JNI method each, and reads the replies.

mod events;
mod frames;
mod mirrors;
mod services;
mod types;
mod views;
mod words;

use std::fmt::Write;

use yog_codegen::{Header, Lang, camel, pascal};
use yog_data::Shape;
use yog_data::schema::StructShape;

use types::{TYPES, from_data, java_type, nullable, same, to_data};

/// What the generated file says it is.
pub(super) const HEADER: Header = Header {
    tool: "yog-event",
    source: "Yog-Mod-Loader/rust/crates/yog-event",
    regenerate: "cargo run -p yog-event --bin yog-event-export",
};

/// The imports, each added when the file uses it. Java's own types are
/// written in full (`java.util.List`): a message may be named `List`.
const IMPORTS: &[(&str, &str)] = &[
    ("YogBus.", "dev.yog.YogBus"),
    ("YogData.", "dev.yog.data.YogData"),
    ("YogFrame.", "dev.yog.data.YogFrame"),
    ("YogMessage", "dev.yog.YogMessage"),
];

/// One level of indentation.
const INDENT: &str = Lang::Java.indent();

/// A payload field's Java name: `player_name` → `playerName`, `interface` → `interface_`.
fn field(snake: &str) -> String {
    Lang::Java.ident(&camel(snake))
}

/// `key_binding` → `KeyBinding`.
pub(super) fn class_name(segment: &str) -> String {
    pascal(segment)
}

/// The file being written, and the payload records it still owes.
#[derive(Default)]
struct Out {
    text: String,
    records: Vec<StructShape>,
    /// The class's own members, written before its nested ones.
    members: String,
}

/// A record with `s`'s fields: `toData()` builds the payload, `fromData` /
/// `fromBytes` read one back; `body` adds the message's methods.
fn record(o: &mut Out, pad: &str, name: &str, s: &StructShape, body: &str) {
    let Out {
        text: out, records, ..
    } = o;
    let params: Vec<String> = s
        .fields
        .iter()
        .map(|f| format!("{} {}", java_type(&(f.shape)(), records), field(f.name)))
        .collect();
    if !s.doc.is_empty() {
        let _ = writeln!(out, "\n{pad}/** {} */", s.doc);
    }
    let _ = writeln!(out, "{pad}public record {name}({}) {{", params.join(", "));
    let _ = writeln!(
        out,
        "{pad}    public java.util.Map<String, Object> toData() {{\n{pad}        java.util.Map<String, Object> m = new java.util.LinkedHashMap<>();"
    );
    for f in &s.fields {
        let shape = (f.shape)();
        let field = field(f.name);
        let line = match nullable(&shape) {
            Some(inner) => format!(
                "if ({field} != null) m.put(\"{}\", {});",
                f.name,
                to_data(&inner, &field, 0)
            ),
            None => format!("m.put(\"{}\", {});", f.name, to_data(&shape, &field, 0)),
        };
        let _ = writeln!(out, "{pad}        {line}");
    }
    let _ = writeln!(out, "{pad}        return m;\n{pad}    }}\n");
    let args: Vec<String> = s
        .fields
        .iter()
        .map(|f| from_data(&(f.shape)(), &format!("m.get(\"{}\")", f.name), 0))
        .collect();
    let _ = writeln!(
        out,
        "{pad}    public static {name} fromData(Object data) {{\n{pad}        java.util.Map<String, Object> m = YogData.map(data);\n{pad}        return new {name}({});\n{pad}    }}\n",
        args.join(", ")
    );
    let _ = write!(
        out,
        "{pad}    public static {name} fromBytes(byte[] bytes) {{\n{pad}        return fromData(YogData.decode(bytes));\n{pad}    }}"
    );
    out.push_str(&frames::codec(name, s, pad));
    out.push_str(&views::view(name, s, pad));
    let body = body.replace('\n', &format!("\n{pad}"));
    let _ = writeln!(out, "{body}\n{pad}}}");
}

/// `address` in Java: its name in the hosts' `YogNames` (one source), or the
/// string itself if no name has it.
fn name(address: &str) -> String {
    yog_ids::export::java_ref(yog_ids::export::JAVA_CLASS, address)
        .unwrap_or_else(|| format!("{address:?}"))
}

/// Where a message goes: its own address (`Address.<constant>`, resolved
/// once), or its branch for the key field's value.
fn target(address: &str, key: Option<&str>, constant: &str) -> String {
    match key {
        Some(k) => format!("YogBus.keyed({}, {})", name(address), field(k)),
        None => format!("Address.{constant}"),
    }
}

/// Where a listener or provider of a message at `address` is: a keyed
/// one's branch for the `key` parameter it's given.
fn listen_target(address: &str, key: Option<&str>, constant: &str) -> String {
    match key {
        Some(_) => format!("YogBus.keyed({}, key)", name(address)),
        None => format!("Address.{constant}"),
    }
}

/// The `key` parameter a keyed message's listener or provider takes first.
fn key_param(key: Option<&str>) -> &'static str {
    if key.is_some() { "String key, " } else { "" }
}

/// A message to write: its place in the tree, its name, payload and methods.
struct Entry {
    path: &'static str,
    name: &'static str,
    shape: Shape,
    body: String,
}

/// Closes the classes of `open` down to `keep`.
fn close(out: &mut String, open: &mut Vec<String>, keep: usize) {
    while open.len() > keep {
        open.pop();
        let _ = writeln!(out, "{}}}", INDENT.repeat(open.len() + 1));
    }
}

/// Class `class` (documented `doc`) in `package`: `entries` nested by their paths.
fn file(
    package: &str,
    class: &str,
    doc: &str,
    mut o: Out,
    entries: &[Entry],
) -> (String, Vec<StructShape>) {
    let _ = writeln!(
        o.text,
        "\n/** {doc} */\npublic final class {class} {{\n    private {class}() {{}}"
    );
    let members = std::mem::take(&mut o.members);
    o.text.push_str(&members);
    // The class path open now (`Player`, `Session`), closed and opened as the paths go.
    let mut open: Vec<String> = Vec::new();
    let mut opened: Vec<String> = Vec::new();
    for e in entries {
        let path: Vec<&str> = e.path.split('/').collect();
        let common = open.iter().zip(&path).take_while(|(a, b)| a == *b).count();
        close(&mut o.text, &mut open, common);
        for (depth, seg) in path.iter().enumerate().skip(common) {
            // A class opened again would be a second class of one name.
            let whole = path[..=depth].join("/");
            assert!(
                !opened.contains(&whole),
                "the entries of `{whole}` aren't together"
            );
            opened.push(whole);
            let pad = INDENT.repeat(open.len() + 1);
            let class = class_name(seg);
            let _ = writeln!(
                o.text,
                "\n{pad}/** `{seg}`. */\n{pad}public static final class {class} {{\n{pad}    private {class}() {{}}"
            );
            open.push((*seg).to_owned());
        }
        let Shape::Struct(s) = &e.shape else { continue };
        // Java refuses a member type named like a class around it.
        assert!(
            open.iter().all(|seg| class_name(seg) != e.name),
            "{} is named like the class around it ({}): rename it",
            e.name,
            e.path
        );
        record(&mut o, &INDENT.repeat(open.len() + 1), e.name, s, &e.body);
    }
    close(&mut o.text, &mut open, 0);
    finish(package, o)
}

/// `class` holding `records` — and the records they hold in turn.
fn types_file(package: &str, records: Vec<StructShape>) -> String {
    let mut o = Out {
        records,
        ..Out::default()
    };
    let _ = writeln!(
        o.text,
        "\n/** The records the bus's messages share. */\npublic final class {TYPES} {{\n    private {TYPES}() {{}}"
    );
    let mut done = 0;
    while done < o.records.len() {
        let s = o.records[done].clone();
        record(&mut o, INDENT, s.name, &s, "");
        done += 1;
    }
    unique(&o.records);
    o.text.push_str(&words::classes(&o.records, INDENT));
    finish(package, o).0
}

/// The class closed, with its header and imports: its text, and the records it needs.
fn finish(package: &str, mut o: Out) -> (String, Vec<StructShape>) {
    o.text.push_str("}\n");
    let out = o.text;
    let names = format!(
        "{}.{}",
        yog_ids::export::JAVA_PACKAGE,
        yog_ids::export::JAVA_CLASS
    );
    let mut imports: Vec<String> = IMPORTS
        .iter()
        .filter(|(used, _)| out.contains(used))
        .map(|(_, class)| (*class).to_owned())
        .collect();
    imports.push(names);
    imports.sort();
    let imports: String = imports
        .iter()
        .map(|class| format!("import {class};\n"))
        .collect();
    let text = format!(
        "{}package {package};\n\n{imports}{out}",
        HEADER.line_comment()
    );
    (
        text.lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
        o.records,
    )
}

/// Panics when two different payload structs share a record name — they'd
/// be one class in `YogTypes`.
fn unique(records: &[StructShape]) {
    for (i, a) in records.iter().enumerate() {
        for b in &records[i + 1..] {
            assert!(
                a.name != b.name || same(a, b),
                "two payload structs are named {}: rename one",
                a.name
            );
        }
    }
}

/// Payloads that travel inside other bytes (entity data), not as messages:
/// Java builds them too.
const PAYLOADS: &[fn() -> Shape] = &[
    <crate::entity::Hanging as yog_data::Schema>::shape,
    // The content the runtime hands the hosts at registration (`content_blob`).
    <crate::content::Content as yog_data::Schema>::shape,
    // A mirror provider's turn (sent to its mirror's own address).
    <crate::mirror::Write as yog_data::Schema>::shape,
    // The host's block mirror's scope (inside a declaration's scope bytes).
    <crate::mirror::BlockScope as yog_data::Schema>::shape,
    // The host's mapping table (in its jar, read once).
    <crate::nanite::Mappings as yog_data::Schema>::shape,
    // Mods' nanites, read by the host before the game's classes load.
    <crate::nanite::Nanites as yog_data::Schema>::shape,
];

/// Every generated class: its name and text.
pub fn files(package: &str) -> [(&'static str, String); 4] {
    let (events, mut records) = events::events(package);
    let (services, more) = services::services(package);
    for r in more {
        if !records.iter().any(|have| same(have, &r)) {
            records.push(r);
        }
    }
    for payload in PAYLOADS {
        let Shape::Struct(r) = payload() else {
            continue;
        };
        if !records.iter().any(|have| same(have, &r)) {
            records.push(r);
        }
    }
    unique(&records);
    [
        (events::CLASS, events),
        (services::CLASS, services),
        (TYPES, types_file(package, records)),
        (mirrors::CLASS, mirrors::file(package)),
    ]
}

#[cfg(test)]
mod tests {
    use yog_data::{Schema, Shape};

    use super::unique;

    mod a {
        #[derive(yog_data::Schema)]
        pub struct Declare {
            pub name: String,
        }
    }

    mod b {
        #[derive(yog_data::Schema)]
        pub struct Declare {
            pub label: String,
        }
    }

    fn shape<T: Schema>() -> yog_data::schema::StructShape {
        let Shape::Struct(s) = T::shape() else {
            unreachable!("a derived struct")
        };
        s
    }

    #[test]
    #[should_panic(expected = "two payload structs are named Declare")]
    fn two_structs_of_one_name_are_refused() {
        unique(&[shape::<a::Declare>(), shape::<b::Declare>()]);
    }

    #[test]
    fn one_struct_seen_twice_is_fine() {
        unique(&[shape::<a::Declare>(), shape::<a::Declare>()]);
    }
}

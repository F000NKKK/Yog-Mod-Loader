//! `dev.yog.bus.YogEvents`: each event's record, emitted on the bus.

use std::fmt::Write;

use yog_data::schema::StructShape;

use crate::EventInfo;

use super::{Entry, Out, file, name, target};

/// The class it writes.
pub(super) const CLASS: &str = "YogEvents";

/// An event's methods: `emit` after the game did it, `emitPre` before.
fn event_body(e: &EventInfo) -> String {
    let mut body = String::new();
    // A keyed event's addresses are its key's branches, resolved as they come.
    if e.key.is_none() {
        let _ = write!(body, "\n\n    private static final class Address {{\n        static final int POST = YogBus.resolve({});", name(e.address));
        if let Some(p) = e.pre {
            let _ = write!(body, "\n        static final int PRE = YogBus.resolve({});", name(p));
        }
        body.push_str("\n    }\n\n    /** Whether anyone listens — skip building an event no one hears. */\n    public static boolean heard() {\n        return YogBus.heard(Address.POST);\n    }");
    }
    let _ = write!(
        body,
        "\n\n    public YogBus.Reply emit() {{\n        return YogBus.emit({}, toFrame());\n    }}",
        target(e.address, e.key, "POST")
    );
    body.push_str("\n\n    /** Emits it and reads back what the handlers made of it (itself when the reply isn't one). */\n    public ");
    body.push_str(e.name);
    body.push_str(" emitChanged() {\n        YogBus.Reply r = emit();\n        // Nothing back: no handler changed it.\n        if (r.status() != YogBus.OK || r.bytes().length == 0) return this;\n        try {\n            return fromFrame(r.bytes());\n        } catch (RuntimeException e) {\n            return this;\n        }\n    }");
    if let Some(p) = e.pre {
        let _ = write!(
            body,
            "\n\n    public boolean emitPre() {{\n        return YogBus.emit({}, toFrame()).status() != YogBus.CANCELLED;\n    }}",
            target(p, e.key, "PRE")
        );
    }
    body
}

/// `dev.yog.bus.YogEvents`.
pub(super) fn events(package: &str) -> (String, Vec<StructShape>) {
    let entries: Vec<Entry> = crate::all().into_iter().map(|e| Entry { path: e.plane, name: e.name, body: event_body(&e), shape: e.shape }).collect();
    file(
        package,
        CLASS,
        "The bus's events: a record per event; {@code emit} after the game did it, {@code emitPre} before (false: a handler cancelled it).",
        Out::default(),
        &entries,
    )
}

//! `dev.yog.bus.YogEvents`: each event's record, emitted on the bus.

use std::fmt::Write;

use yog_data::schema::StructShape;

use crate::EventInfo;

use super::{Entry, Out, file, key_param, listen_target, name, target};

/// The class it writes.
pub(super) const CLASS: &str = "YogEvents";

/// An event's methods: `emit` after the game did it, `emitPre` before.
fn event_body(e: &EventInfo) -> String {
    let mut body = String::new();
    // A keyed event's addresses are its key's branches, resolved as they come.
    if e.key.is_none() {
        let _ = write!(
            body,
            "\n\n    private static final class Address {{\n        static final int POST = YogBus.resolve({});",
            name(e.address)
        );
        if let Some(p) = e.pre {
            let _ = write!(
                body,
                "\n        static final int PRE = YogBus.resolve({});",
                name(p)
            );
        }
        body.push_str("\n    }\n\n    /** Whether anyone listens — skip building an event no one hears. */\n    public static boolean heard() {\n        return YogBus.heard(Address.POST);\n    }");
    }
    let _ = write!(
        body,
        "\n\n    /** Emits it; the reply, read in place, is what the handlers changed it to (empty: none did). */\n    public YogMessage emit() {{\n        return YogBus.emit({}, packedFrame());\n    }}",
        target(e.address, e.key, "POST")
    );
    body.push_str("\n\n    /** Emits it and reads back what the handlers made of it (itself when the reply isn't one). */\n    public ");
    body.push_str(e.name);
    body.push_str(" emitChanged() {\n        YogMessage r = emit();\n        // Nothing back: no handler changed it.\n        if (!r.ok() || r.isEmpty()) return this;\n        try {\n            return fromFrame(r);\n        } catch (RuntimeException e) {\n            return this;\n        }\n    }");
    if let Some(p) = e.pre {
        let _ = write!(
            body,
            "\n\n    public boolean emitPre() {{\n        return YogBus.emit({}, packedFrame()).status() != YogBus.CANCELLED;\n    }}",
            target(p, e.key, "PRE")
        );
    }
    body.push_str(&listeners(e));
    body
}

/// `on` (after the game did it: the listener may change it) and `onPre`
/// (before: the listener may cancel it), typed.
fn listeners(e: &EventInfo) -> String {
    let (n, key) = (e.name, key_param(e.key));
    let mut body = format!(
        "\n\n    /** Listens to it (higher {{@code priority}} first); what {{@code l}} returns, when not null, is what it becomes. */\n    public static void on({key}int priority, dev.yog.api.YogBus.Listener<{n}> l) {{\n        YogBus.on({}, priority, (in, out) -> {{\n            {n} changed = l.on(fromFrame(in));\n            if (changed != null) changed.pack(out.frame());\n            return YogBus.OK;\n        }});\n    }}",
        listen_target(e.address, e.key, "POST")
    );
    if let Some(p) = e.pre {
        let _ = write!(
            body,
            "\n\n    /** Listens before the game does it (higher {{@code priority}} first); {{@code l}} answering false cancels it. */\n    public static void onPre({key}int priority, dev.yog.api.YogBus.PreListener<{n}> l) {{\n        YogBus.on({}, priority, (in, out) -> l.allow(fromFrame(in)) ? YogBus.OK : YogBus.CANCELLED);\n    }}",
            listen_target(p, e.key, "PRE")
        );
    }
    body
}

/// `dev.yog.bus.YogEvents`.
pub(super) fn events(package: &str) -> (String, Vec<StructShape>) {
    let entries: Vec<Entry> = crate::all()
        .into_iter()
        .map(|e| Entry {
            path: e.plane,
            name: e.name,
            body: event_body(&e),
            shape: e.shape,
        })
        .collect();
    file(
        package,
        CLASS,
        "The bus's events: a record per event; {@code emit} after the game did it, {@code emitPre} before (false: a handler cancelled it).",
        Out::default(),
        &entries,
    )
}

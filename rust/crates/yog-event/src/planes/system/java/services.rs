//! `dev.yog.bus.YogServices`: each service's record, called on the bus.

use std::fmt::Write;

use yog_data::Shape;
use yog_data::schema::StructShape;

use crate::ServiceInfo;

use super::frames;
use super::types::{TYPES, boxed, java_type};
use super::{Entry, Out, file, key_param, listen_target, name, target};

/// The class it writes.
pub(super) const CLASS: &str = "YogServices";

/// A service's methods: `call` asks its provider and reads the reply — the
/// reply type's default when no one answers.
fn service_body(s: &ServiceInfo, o: &mut Out) -> String {
    let mut body = String::new();
    if s.key.is_none() {
        let _ = write!(
            body,
            "\n\n    private static final class Address {{\n        static final int AT = YogBus.resolve({});\n    }}",
            name(s.address)
        );
    }
    if let Some(host) = s.host {
        let _ = write!(
            body,
            "\n\n    /** The host's method answering it ({{@code NativeBridge}}), and where it's provided. */\n    public static final String HOST = \"{host}\";\n    public static final String ADDRESS = {};",
            name(s.address)
        );
    }
    let reply = java_type(&s.reply, &mut o.records);
    let _ = write!(
        body,
        "\n\n    public {reply} call() {{\n        YogMessage r = YogBus.call({}, packedFrame());\n        if (r.ok() && !r.isEmpty()) {{\n            try {{\n                YogFrame.Unpacker u = r.frame();\n                {reply} v = {};\n                u.finish();\n                return v;\n            }} catch (YogData.DecodeException e) {{\n                // Not a reply: as if no one answered.\n            }}\n        }}\n        return {};\n    }}",
        target(s.address, s.key, "AT"),
        frames::unpack(&s.reply, 0),
        frames::missing(&s.reply)
    );
    if let Shape::Struct(r) = &s.reply {
        let _ = write!(
            body,
            "\n\n    /** {{@link #call}}'s reply read where it is (valid until this thread's next bus call); null when no one answered. */\n    public {TYPES}.{n}.View callView() {{\n        YogMessage r = YogBus.call({}, packedFrame());\n        if (!r.ok() || r.isEmpty()) return null;\n        try {{\n            return new {TYPES}.{n}.View(r.frame());\n        }} catch (YogData.DecodeException e) {{\n            return null;\n        }}\n    }}",
            target(s.address, s.key, "AT"),
            n = r.name
        );
    }
    let _ = write!(
        body,
        "\n\n    /** Answers it: the request in, the reply out; false if someone answers it already. */\n    public static boolean provide({}dev.yog.api.YogBus.Provider<{n}, {}> provider) {{\n        return YogBus.provide({}, (in, out) -> {{\n            {reply} v = provider.answer(fromFrame(in));\n            YogFrame.Packer p = out.frame();\n            {}\n            return YogBus.OK;\n        }});\n    }}",
        key_param(s.key),
        boxed(&reply),
        listen_target(s.address, s.key, "AT"),
        frames::pack(&s.reply, "v", 0),
        n = s.name
    );
    if s.host.is_some() {
        let _ = write!(
            body,
            "\n\n    /** The host method's result {{@code out}} (its \"none\" already null) packed as the reply. */\n    public static void reply(Object out, YogFrame.Packer p) {{\n        {reply} v = {};\n        {}\n    }}",
            frames::from_host(&s.reply, "out"),
            frames::pack(&s.reply, "v", 0)
        );
    }
    body
}

/// `dev.yog.bus.YogServices`.
pub(super) fn services(package: &str) -> (String, Vec<StructShape>) {
    let mut o = Out::default();
    let mut services = crate::services();
    // A domain's services together: one class each.
    services.sort_by_key(|s| s.domain);
    let entries: Vec<Entry> = services
        .into_iter()
        .map(|s| Entry {
            path: s.domain,
            name: s.name,
            body: service_body(&s, &mut o),
            shape: s.shape,
        })
        .collect();
    file(
        package,
        CLASS,
        "The bus's services: a record per call; {@code call} asks whoever provides it.",
        o,
        &entries,
    )
}

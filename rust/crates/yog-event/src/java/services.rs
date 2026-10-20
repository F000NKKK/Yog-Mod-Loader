//! `dev.yog.bus.YogServices`: each service's record, called on the bus.

use std::fmt::Write;

use yog_data::schema::StructShape;

use crate::ServiceInfo;

use super::frames;
use super::types::java_type;
use super::{Entry, Out, class_name, file, name, target};

/// The class it writes.
pub(super) const CLASS: &str = "YogServices";

/// A service's methods: `call` asks its provider and reads the reply — the
/// reply type's default when no one answers.
fn service_body(s: &ServiceInfo, o: &mut Out) -> String {
    let mut body = String::new();
    if s.key.is_none() {
        let _ = write!(body, "\n\n    private static final class Address {{\n        static final int AT = YogBus.resolve({});\n    }}", name(s.address));
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
        "\n\n    public {reply} call() {{\n        YogBus.Reply r = YogBus.call({}, toFrame());\n        if (r.status() == YogBus.OK && r.bytes().length > 0) {{\n            try {{\n                YogFrame.Unpacker u = new YogFrame.Unpacker(r.bytes());\n                {reply} v = {};\n                u.finish();\n                return v;\n            }} catch (YogData.DecodeException e) {{\n                // Not a reply: as if no one answered.\n            }}\n        }}\n        return {};\n    }}",
        target(s.address, s.key, "AT"),
        frames::unpack(&s.reply, 0),
        frames::missing(&s.reply)
    );
    if s.host.is_some() {
        let _ = write!(
            body,
            "\n\n    /** The host method's result {{@code out}} (its \"none\" already null) as the reply's frame. */\n    public static byte[] replyFrame(Object out) {{\n        YogFrame.Packer p = new YogFrame.Packer();\n        {reply} v = {};\n        {}\n        return p.finish();\n    }}",
            frames::from_host(&s.reply, "out"),
            frames::pack(&s.reply, "v", 0)
        );
    }
    body
}

/// `World.SetBlock`: a service's record, from `YogServices`.
fn record_path(domain: &str, name: &str) -> String {
    domain.split('/').map(class_name).chain([name.to_owned()]).collect::<Vec<_>>().join(".")
}

/// `dev.yog.bus.YogServices`.
pub(super) fn services(package: &str) -> (String, Vec<StructShape>) {
    let mut o = Out::default();
    let mut services = crate::services();
    // A domain's services together: one class each.
    services.sort_by_key(|s| s.domain);
    let hosted: Vec<String> = services.iter().filter(|s| s.host.is_some()).map(|s| record_path(s.domain, s.name)).collect();
    let _ = write!(
        o.members,
        "\n    /** The actions the host answers ({{@code YogHostServices}}). */\n    public static final java.util.List<Class<? extends Record>> HOSTED = java.util.List.of({});\n",
        hosted.iter().map(|r| format!("{r}.class")).collect::<Vec<_>>().join(", ")
    );
    let entries: Vec<Entry> = services
        .into_iter()
        .map(|s| Entry { path: s.domain, name: s.name, body: service_body(&s, &mut o), shape: s.shape })
        .collect();
    file(package, CLASS, "The bus's services: a record per call; {@code call} asks whoever provides it.", o, &entries)
}

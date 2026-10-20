//! `dev.yog.bus.YogServices`: each service's record, called on the bus.

use std::fmt::Write;

use yog_data::schema::StructShape;

use crate::ServiceInfo;

use super::types::{from_data, java_type};
use super::{Entry, Out, file, name, target};

/// The class it writes.
pub(super) const CLASS: &str = "YogServices";

/// A service's methods: `call` asks its provider and reads the reply — the
/// reply type's default when no one answers.
fn service_body(s: &ServiceInfo, o: &mut Out) -> String {
    let mut body = String::new();
    if s.key.is_none() {
        let _ = write!(body, "\n\n    private static final class Address {{\n        static final int AT = YogBus.resolve({});\n    }}", name(s.address));
    }
    let reply = java_type(&s.reply, &mut o.records);
    let _ = write!(
        body,
        "\n\n    public {reply} call() {{\n        YogBus.Reply r = YogBus.call({}, YogData.encode(toData()));\n        Object v = null;\n        if (r.status() == YogBus.OK && r.bytes().length > 0) {{\n            try {{\n                v = YogData.decode(r.bytes());\n            }} catch (YogData.DecodeException e) {{\n                v = null;\n            }}\n        }}\n        return {};\n    }}",
        target(s.address, s.key, "AT"),
        from_data(&s.reply, "v", 0)
    );
    body
}

/// `dev.yog.bus.YogServices`.
pub(super) fn services(package: &str) -> (String, Vec<StructShape>) {
    let mut o = Out::default();
    let entries: Vec<Entry> = crate::services()
        .into_iter()
        .map(|s| Entry { path: s.domain, name: s.name, body: service_body(&s, &mut o), shape: s.shape })
        .collect();
    file(package, CLASS, "The bus's services: a record per call; {@code call} asks whoever provides it.", o, &entries)
}

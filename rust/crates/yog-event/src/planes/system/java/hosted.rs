//! `dev.yog.bus.YogHostedServices`: the host's side of the actions it
//! answers — each one's request read, its host method called (through a
//! handle adapted once, `YogHostServices.handle`), its reply packed. The
//! records (`YogServices`) are the API's, any Java's; this is the host's own
//! code, for only a host has host methods.

use std::fmt::Write;

use yog_codegen::{camel, pascal};
use yog_data::Shape;
use yog_data::schema::StructShape;

use crate::ServiceInfo;

use super::types::java_type;
use super::{Out, field, finish, record_path, services};

/// The class it writes.
pub(super) const CLASS: &str = "YogHostedServices";

/// What separates a record path's classes (`Client.Control.Attack`).
const DOT: char = '.';

pub(super) fn file(package: &str) -> String {
    let mut o = Out::default();
    let mut all = crate::services();
    all.sort_by_key(|s| s.domain);
    let hosted: Vec<&ServiceInfo> = all.iter().filter(|s| s.host.is_some()).collect();
    let entries: Vec<String> = hosted
        .iter()
        .map(|s| format!("java.util.Map.entry({}.{}.ADDRESS, {CLASS}::{})", services::CLASS, record_path(s.domain, s.name), method(s)))
        .collect();
    let _ = write!(
        o.text,
        "\n/** The host's answers to the actions it provides ({{@code YogServices}}'s records). */\npublic final class {CLASS} {{\n    private {CLASS}() {{}}\n\n    /** The actions the host answers, by address ({{@code YogHostServices}}). */\n    public static final java.util.List<java.util.Map.Entry<String, YogBus.Handler>> HOSTED = java.util.List.of({});\n",
        entries.join(", ")
    );
    for s in hosted {
        o.text.push_str(&serve(s, &mut o.records));
    }
    o.text.push('\n');
    finish(package, o).0
}

/// A service's method here: its record's path, camelCase (`clientControlAttack`).
fn method(s: &ServiceInfo) -> String {
    camel(&record_path(s.domain, s.name).replace(DOT, ""))
}

/// One action's answer: the request out of its frame, the host method
/// called with the request's components spread, the reply packed.
fn serve(s: &ServiceInfo, records: &mut Vec<StructShape>) -> String {
    let mut args = Vec::new();
    if let Shape::Struct(request) = &s.shape {
        spread(request, "r", &mut args, records);
    }
    let classes: String = args.iter().map(|(_, java)| format!(", {}", class_of(java))).collect();
    let exprs: Vec<&str> = args.iter().map(|(expr, _)| expr.as_str()).collect();
    let record = format!("{}.{}", services::CLASS, record_path(s.domain, s.name));
    let method = method(s);
    let handle = pascal(&method);
    format!(
        "\n    private static final class {handle} {{\n        static final java.lang.invoke.MethodHandle H = dev.yog.YogHostServices.handle({record}.HOST, java.lang.invoke.MethodType.methodType(Object.class{classes}));\n    }}\n\n    /** Answers `{record}`: the request out of its frame, the host method called, the reply packed. */\n    public static int {method}(YogMessage in, YogBus.Out sink) {{\n        if ({handle}.H == null) return YogBus.NO_HANDLER;\n        try {{\n            {record}.View r = new {record}.View(in.frame());\n            Object out = (Object) {handle}.H.invokeExact({});\n            {record}.reply(dev.yog.YogHostServices.none(out), sink.frame());\n            return YogBus.OK;\n        }} catch (YogData.DecodeException e) {{\n            return YogBus.BAD_PAYLOAD;\n        }} catch (Throwable t) {{\n            return dev.yog.YogHostServices.failed({record}.ADDRESS, t);\n        }}\n    }}\n",
        exprs.join(", ")
    )
}

/// The arguments record `s` (reached by `at`) spreads into, each with its
/// Java type: a nested record's components in its place.
fn spread(s: &StructShape, at: &str, out: &mut Vec<(String, String)>, records: &mut Vec<StructShape>) {
    for f in &s.fields {
        let shape = (f.shape)();
        let expr = format!("{at}.{}()", field(f.name));
        match &shape {
            Shape::Struct(inner) => spread(inner, &expr, out, records),
            _ => out.push((expr, java_type(&shape, records))),
        }
    }
}

/// A Java type's class literal: `int.class`, `java.util.List.class`.
fn class_of(java: &str) -> String {
    format!("{}.class", java.split('<').next().unwrap_or(java))
}

//! A mod's exports as bus services, and another mod's imports calling them —
//! here without a runtime: an export's provider answers encoded arguments
//! directly, and an import with no bus to call reports it.

use yog_api::yog_data::Schema;
use yog_api::{YogBytes, YogReply, import, yog_export};

#[yog_export]
pub fn scale(v: i32, by: i32) -> i32 {
    v * by
}

#[yog_export]
pub fn explode(code: u32) -> u32 {
    if code > 0 {
        panic!("the exporting mod failed");
    }
    code
}

#[yog_export]
#[derive(Debug, Clone, PartialEq)]
pub struct Pipe {
    pub id: u32,
    pub name: String,
}

#[yog_export]
pub fn rename(pipe: Pipe, name: String) -> Pipe {
    Pipe { name, ..pipe }
}

import! {
    from "other" {
        pub fn scale(v: i32, by: i32) -> i32;
    }
}

/// Runs export `name`'s provider on `args` (a map of its parameters): its status and reply.
fn serve(name: &str, args: &yog_api::yog_data::Value) -> (u32, Vec<u8>) {
    let exports = yog_api::__yog_export_registry().lock().unwrap();
    let entry = exports.iter().find(|e| e.name == name).expect("the export registered itself");
    let payload = yog_api::yog_data::binary::encode(args);
    let mut out = Vec::new();
    let st = unsafe { (entry.handler)(std::ptr::null_mut(), YogBytes::new(&payload), &YogReply::to_vec(&mut out)) };
    (st, out)
}

fn args(fields: &[(&str, yog_api::yog_data::Value)]) -> yog_api::yog_data::Value {
    yog_api::yog_data::Value::Map(fields.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect())
}

#[test]
fn exports_register_themselves_when_the_library_loads() {
    let names: Vec<&str> = yog_api::__yog_export_registry().lock().unwrap().iter().map(|e| e.name).collect();
    for n in ["scale", "explode", "rename"] {
        assert!(names.contains(&n), "{n} in {names:?}");
    }
    assert_eq!(yog_api::export_address("yog-pipes", "attach"), "yog-pipes:attach");
}

#[test]
fn an_export_answers_its_arguments_by_name() {
    let (st, out) = serve("scale", &args(&[("v", 21i32.write()), ("by", 2i32.write())]));
    assert_eq!(st, yog_abi::status::OK);
    assert_eq!(yog_api::yog_data::read_bytes::<i32>(&out).ok(), Some(42));
    let pipe = Pipe { id: 7, name: "old".into() };
    let (_, out) = serve("rename", &args(&[("pipe", pipe.write()), ("name", "new".to_owned().write())]));
    assert_eq!(yog_api::yog_data::read_bytes::<Pipe>(&out).ok(), Some(Pipe { id: 7, name: "new".into() }));
}

#[test]
fn a_panic_in_the_export_is_a_failed_call() {
    let (st, _) = serve("explode", &args(&[("code", 1u32.write())]));
    assert_eq!(st, yog_abi::status::FAILED);
    let (st, _) = serve("scale", &args(&[("v", 1i32.write())]));
    assert_eq!(st, yog_abi::status::BAD_PAYLOAD, "a missing argument");
}

#[test]
fn an_import_without_a_bus_says_so() {
    let r = std::panic::catch_unwind(|| other::scale(21, 2));
    let msg = *r.expect_err("no runtime to call").downcast::<String>().expect("a message");
    assert!(msg.contains("other:scale") && msg.contains("didn't answer"), "{msg}");
}

/// Energy stored, in FE.
#[yog_export(implements = "test:energy/storage")]
pub fn stored() -> u64 {
    0
}

#[test]
fn an_export_carries_its_description() {
    let exports = yog_api::__yog_export_registry().lock().unwrap();
    let entry = exports.iter().find(|e| e.name == "stored").expect("registered");
    assert_eq!(entry.doc, "Energy stored, in FE.");
    assert_eq!(entry.implements, ["test:energy/storage"]);
    let (_, answer) = (entry.shapes)();
    assert!(matches!(answer, yog_api::yog_data::Shape::Int { .. }));
}

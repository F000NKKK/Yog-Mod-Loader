//! A mod's exports as bus services, and another mod's imports calling them —
//! here without a runtime: an export's provider answers encoded arguments
//! directly, and an import with no bus to call answers `None`.

use yog_api::yog_data::Schema;
use yog_api::yog_data::frame::{Packer, read_frame};
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

/// Runs export `name`'s provider on `payload` (a frame of its arguments): its status and reply.
fn serve(name: &str, payload: &[u8]) -> (u32, Vec<u8>) {
    let exports = yog_api::__yog_export_registry().lock();
    let entry = exports
        .iter()
        .find(|e| e.name == name)
        .expect("the export registered itself");
    let mut out = Vec::new();
    let st = unsafe {
        (entry.handler)(
            std::ptr::null_mut(),
            YogBytes::new(payload),
            &YogReply::to_vec(&mut out),
        )
    };
    (st, out)
}

/// An export's arguments as a frame: a record of them, in order.
fn args(put: impl FnOnce(&mut Packer)) -> Vec<u8> {
    let mut p = Packer::new();
    let at = p.begin();
    put(&mut p);
    p.end(at);
    p.finish()
}

#[test]
fn exports_register_themselves_when_the_library_loads() {
    let names: Vec<&str> = yog_api::__yog_export_registry()
        .lock()
        .iter()
        .map(|e| e.name)
        .collect();
    for n in ["scale", "explode", "rename"] {
        assert!(names.contains(&n), "{n} in {names:?}");
    }
    assert_eq!(
        yog_api::export_address("yog-pipes", "attach"),
        "yog-pipes:attach"
    );
}

#[test]
fn an_export_answers_its_arguments_by_name() {
    let (st, out) = serve(
        "scale",
        &args(|p| {
            21i32.pack(p);
            2i32.pack(p);
        }),
    );
    assert_eq!(st, yog_abi::status::OK);
    assert_eq!(read_frame::<i32>(&out), Some(42));
    let pipe = Pipe {
        id: 7,
        name: "old".into(),
    };
    let (_, out) = serve(
        "rename",
        &args(|p| {
            pipe.pack(p);
            "new".to_owned().pack(p);
        }),
    );
    assert_eq!(
        read_frame::<Pipe>(&out),
        Some(Pipe {
            id: 7,
            name: "new".into()
        })
    );
}

#[test]
fn a_panic_in_the_export_is_a_failed_call() {
    // A lone argument crosses as itself.
    let (st, _) = serve("explode", &yog_api::Message::to_bytes(&1u32));
    assert_eq!(st, yog_abi::status::FAILED);
    let (st, out) = serve("explode", &yog_api::Message::to_bytes(&0u32));
    assert_eq!(st, yog_abi::status::OK);
    assert_eq!(read_frame::<u32>(&out), Some(0));
    let (st, _) = serve("scale", &args(|p| 1i32.pack(p)));
    assert_eq!(st, yog_abi::status::BAD_PAYLOAD, "a missing argument");
}

#[test]
fn an_import_without_a_bus_answers_none() {
    assert_eq!(other::scale(21, 2), None);
}

/// Energy stored, in FE.
#[yog_export(implements = "test:energy/storage")]
pub fn stored() -> u64 {
    0
}

#[test]
fn an_export_carries_its_description() {
    let exports = yog_api::__yog_export_registry().lock();
    let entry = exports
        .iter()
        .find(|e| e.name == "stored")
        .expect("registered");
    assert_eq!(entry.doc, "Energy stored, in FE.");
    assert_eq!(entry.implements, ["test:energy/storage"]);
    let (_, answer) = (entry.shapes)();
    assert!(matches!(answer, yog_api::yog_data::Shape::Int { .. }));
}

//! A mod's export called through another mod's import, in one process:
//! what `export_mod!` binds at load time, bound here by hand.

use yog_api::{import, yog_export};

#[yog_export]
pub fn scale(v: i32, by: i32) -> i32 {
    v * by
}

#[yog_export]
pub fn greet(name: String) -> String {
    format!("hello, {name}")
}

#[yog_export]
pub fn explode(code: u32) -> u32 {
    if code > 0 {
        panic!("the exporting mod failed");
    }
    code
}

import! {
    from "other" {
        pub fn scale(v: i32, by: i32) -> i32;
        pub fn greet(name: String) -> String;
        pub fn explode(code: u32) -> u32;
    }
}

/// Binds each import to the export of the same name, as `export_mod!` does.
fn bind() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let exports = yog_api::__yog_export_registry().lock().unwrap();
        for import in yog_api::__yog_import_registry().lock().unwrap().iter() {
            let export = exports.iter().find(|e| e.name == import.symbol).expect("each import has its export");
            let bind: unsafe extern "C" fn(*const std::os::raw::c_void) = unsafe { std::mem::transmute(import.bind_fn) };
            unsafe { bind(export.ptr as *const std::os::raw::c_void) };
        }
    });
}

#[test]
fn exports_register_themselves_when_the_library_loads() {
    let names: Vec<&str> = yog_api::__yog_export_registry().lock().unwrap().iter().map(|e| e.name).collect();
    for n in ["scale", "greet", "explode"] {
        assert!(names.contains(&n), "{n} in {names:?}");
    }
    let imports: Vec<(&str, &str)> = yog_api::__yog_import_registry().lock().unwrap().iter().map(|i| (i.mod_id, i.symbol)).collect();
    assert!(imports.contains(&("other", "greet")));
}

#[test]
fn calls_cross_with_their_arguments_and_results() {
    bind();
    assert_eq!(other::scale(21, 2), 42);
    assert_eq!(other::greet("Steve".into()), "hello, Steve");
    assert_eq!(other::explode(0), 0);
}

#[test]
fn a_panic_in_the_exporting_mod_stays_on_the_importing_side() {
    bind();
    let r = std::panic::catch_unwind(|| other::explode(1));
    let msg = *r.expect_err("the import reports the failure").downcast::<String>().expect("a message");
    assert!(msg.contains("other:explode") && msg.contains("failed in its mod"), "{msg}");
}

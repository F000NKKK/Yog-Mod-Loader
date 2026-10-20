//! The router: addresses, providers, event order, cancels, replies.

use std::os::raw::c_void;

use yog_abi::{YogBytes, YogReply, YogStatus, status};

use super::router::{Handler, Router};

fn no_java(_: i32, _: &[u8], _: &YogReply) -> YogStatus {
    status::NO_HANDLER
}

/// Replies with its payload and `ud` (one byte) appended.
unsafe extern "C" fn append_ud(ud: *mut c_void, payload: YogBytes, reply: *const YogReply) -> YogStatus {
    unsafe {
        let mut out = payload.as_slice().to_vec();
        out.push(ud as usize as u8);
        let r = &*reply;
        (r.write)(r.sink, out.as_ptr(), out.len());
    }
    status::OK
}

unsafe extern "C" fn cancel(_: *mut c_void, _: YogBytes, _: *const YogReply) -> YogStatus {
    status::CANCELLED
}

fn native(ud: u8, f: yog_abi::YogHandlerFn) -> Handler {
    Handler::Native { owner: None, ud: ud as usize, f }
}

#[test]
fn an_address_resolves_to_the_same_handle_and_back() {
    let r = Router::default();
    let a = r.resolve("yog:world/set_block");
    assert_eq!(r.resolve("yog:world/set_block"), a);
    assert_ne!(r.resolve("yog:world/get_block"), a);
    assert_eq!(r.resolve(""), yog_abi::NO_ADDRESS);
}

#[test]
fn a_call_reaches_its_one_provider() {
    let r = Router::default();
    let a = r.resolve("test:echo");
    assert_eq!(r.call_vec(a, b"x", no_java).0, status::NO_HANDLER);
    assert!(r.provide(a, native(7, append_ud)));
    assert!(!r.provide(a, native(8, append_ud)), "one provider per address");
    assert!(r.provided(a));
    assert_eq!(r.call_vec(a, b"x", no_java), (status::OK, b"x\x07".to_vec()));
}

#[test]
fn an_event_runs_by_priority_each_seeing_the_last_reply() {
    let r = Router::default();
    let e = r.resolve("test:event");
    r.on(e, 0, native(1, append_ud));
    r.on(e, 10, native(2, append_ud));
    r.on(e, 0, native(3, append_ud));
    assert_eq!(r.emit_vec(e, b"", no_java), (status::OK, vec![2, 1, 3]));
}

#[test]
fn a_cancel_stops_the_event() {
    let r = Router::default();
    let e = r.resolve("test:cancel");
    r.on(e, 5, native(1, append_ud));
    r.on(e, 0, native(0, cancel));
    r.on(e, -5, native(9, append_ud));
    assert_eq!(r.emit_vec(e, b"", no_java), (status::CANCELLED, vec![1]));
}

#[test]
fn java_handlers_run_through_the_java_runner() {
    fn upper(handler: i32, payload: &[u8], reply: &YogReply) -> YogStatus {
        assert_eq!(handler, 42);
        unsafe { YogReply::send(reply, &payload.to_ascii_uppercase()) };
        status::OK
    }
    let r = Router::default();
    let a = r.resolve("java:upper");
    assert!(r.provide(a, Handler::Java(42)));
    assert_eq!(r.call_vec(a, b"yog", upper), (status::OK, b"YOG".to_vec()));
}

#[test]
fn an_event_is_heard_once_someone_listens() {
    let r = Router::default();
    let e = r.resolve("test:quiet");
    assert!(!r.heard(e));
    r.on(e, 0, native(1, append_ud));
    assert!(r.heard(e));
}

#[test]
fn the_keys_listened_to_under_an_address_are_its_branches() {
    let r = Router::default();
    let base = "test:packet";
    for key in ["b:sync", "a:ping"] {
        let at = r.resolve(&yog_event::keyed(base, key));
        r.on(at, 0, native(1, append_ud));
    }
    r.resolve(&yog_event::keyed(base, "c:unheard"));
    assert_eq!(r.listened_keys(base), ["a:ping", "b:sync"]);
}

#[test]
fn declared_commands_are_kept_once_each() {
    use yog_event::{Message, Service, command};
    super::register_duties();
    let router = &*super::ROUTER;
    let call = |address: &str, payload: Vec<u8>| router.call_vec(router.resolve(address), &payload, no_java);
    for schema in [None, Some("int".to_owned()), None] {
        let d = command::CommandSpec { name: "hex".into(), schema };
        assert_eq!(call(command::CommandSpec::ADDRESS, d.to_bytes()).0, status::OK);
    }
    let (st, reply) = call(command::Declared::ADDRESS, command::Declared {}.to_bytes());
    assert_eq!(st, status::OK);
    let all = Vec::<command::CommandSpec>::from_bytes(&reply).expect("the declared commands");
    let schemas: Vec<Option<&str>> = all.iter().filter(|d| d.name == "hex").map(|d| d.schema.as_deref()).collect();
    assert_eq!(schemas, [None, Some("int")]);
}

/// An action's function reaches the bus once the caller is installed.
#[test]
fn action_functions_reach_the_bus() {
    unsafe extern "C" fn answer(_ud: *mut std::os::raw::c_void, _payload: YogBytes, reply: *const YogReply) -> YogStatus {
        unsafe { YogReply::send(reply, &Message::to_bytes(&true)) };
        status::OK
    }
    use yog_event::{Message, Service};
    super::install_caller();
    let router = &super::ROUTER;
    router.provide(router.resolve(<yog_event::chat::Broadcast as Service>::ADDRESS), Handler::Native { owner: None, ud: 0, f: answer });
    assert!(yog_event::chat::broadcast("hello"));
}

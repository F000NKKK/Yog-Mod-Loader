//! Nanites with a real JVM, in order (the mapping table is read once a
//! process): reflection (`YogReflect`: a constructor, a private method and
//! field by name, a missing member, a held object let go; the same by a
//! table's names), then injections (a mixin made by `YogNaniteGen`, one
//! fired through `YogInjections` over the bus to a handler here, which
//! cancels it with a value). Needs a JDK, and for the injections Mixin's
//! and ASM's jars (Gradle's cache, or `YOG_MIXIN_CP`):
//! `cargo test -p yog-runtime nanite_jvm -- --ignored`.

use std::path::{Path, PathBuf};

use std::os::raw::c_void;

use jni::JNIEnv;
use jni::objects::JString;
use yog_abi::{YogBytes, YogReply, YogStatus, status};
use yog_data::Value;
use yog_event::Message;
use yog_event::nanite::Injected;

use crate::bus::{Handler, ROUTER};
use yog_event::nanite::{ClassMap, Mappings, MemberMap};

const PROBE: &str = "dev/yog/nanite/InjectProbe";
const POINT: &str = "test:point";
const ANSWER: i64 = 99;

/// The handler: the second argument seen, the method cancelled with `ANSWER`.
unsafe extern "C" fn answer(
    _: *mut c_void,
    payload: YogBytes,
    reply: *const YogReply,
) -> YogStatus {
    let Some(mut e) = Injected::from_bytes(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    assert_eq!(e.args, vec![Value::Str("x".into()), Value::Int(3)]);
    e.value = Value::Int(ANSWER);
    e.cancelled = true;
    e.returned = true;
    unsafe { YogReply::send(reply, &e.to_bytes()) };
    status::OK
}

/// A modify handler: the value plus 100.
unsafe extern "C" fn add_hundred(
    _: *mut c_void,
    payload: YogBytes,
    reply: *const YogReply,
) -> YogStatus {
    let Some(mut e) = Injected::from_bytes(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    if let Value::Int(v) = e.value {
        e.value = Value::Int(v + 100);
        e.returned = true;
    }
    unsafe { YogReply::send(reply, &e.to_bytes()) };
    status::OK
}

/// A redirect handler: the call answered with 7.
unsafe extern "C" fn answer_seven(
    _: *mut c_void,
    payload: YogBytes,
    reply: *const YogReply,
) -> YogStatus {
    let Some(mut e) = Injected::from_bytes(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    e.value = Value::Int(7);
    e.returned = true;
    unsafe { YogReply::send(reply, &e.to_bytes()) };
    status::OK
}

/// An inject's handler: its first local doubled.
unsafe extern "C" fn double_local(
    _: *mut c_void,
    payload: YogBytes,
    reply: *const YogReply,
) -> YogStatus {
    let Some(mut e) = Injected::from_bytes(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    if let Some(Value::Int(v)) = e.locals.first().cloned() {
        e.locals[0] = Value::Int(v * 2);
    }
    unsafe { YogReply::send(reply, &e.to_bytes()) };
    status::OK
}

fn string(env: &mut JNIEnv, method: &str) -> String {
    let out = env
        .call_static_method(PROBE, method, "()Ljava/lang/String;", &[])
        .and_then(|v| v.l());
    if env.exception_check().unwrap_or(false) {
        env.exception_describe().ok();
    }
    env.get_string(&JString::from(out.unwrap())).unwrap().into()
}

#[test]
#[ignore = "needs a JDK; run with --ignored"]
fn nanites_reflect_and_inject() {
    let Some(jvm) = crate::test_jvm_shared::shared() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let common = root.join("../../../java-common/src/main/java/dev/yog");
    let sources: Vec<PathBuf> = [
        "nanite/YogReflect.java",
        "nanite/YogObjects.java",
        "nanite/YogMappings.java",
        "data/YogJava.java",
        "data/YogData.java",
        "data/YogFrame.java",
        "YogNames.java",
        "YogBus.java",
        "YogMessage.java",
        "YogAtoms.java",
        "bus/YogTypes.java",
        "bus/YogEvents.java",
        "nanite/YogNaniteGen.java",
        "nanite/YogNaniteBodies.java",
        "nanite/YogNaniteAnnotations.java",
        "nanite/YogNaniteLocals.java",
        "nanite/YogNanites.java",
        "nanite/YogEarly.java",
        "util/YogNativeRuntime.java",
        "nanite/YogInjections.java",
    ]
    .map(|f| common.join(f))
    .into_iter()
    .chain([
        root.join("tests/java/dev/yog/YogLog.java"),
        root.join("tests/java/dev/yog/YogPlatform.java"),
        root.join("tests/java/dev/yog/nanite/ReflectProbe.java"),
        root.join("tests/java/dev/yog/nanite/InjectProbe.java"),
    ])
    .collect();
    assert!(jvm.compile(&sources), "javac failed");
    write_table(&jvm.classes);
    let vm = &jvm.vm;
    let mut env = vm.attach_current_thread().unwrap();
    let out = env
        .call_static_method(
            "dev/yog/nanite/ReflectProbe",
            "run",
            "()Ljava/lang/String;",
            &[],
        )
        .and_then(|v| v.l());
    if env.exception_check().unwrap_or(false) {
        env.exception_describe().ok();
    }
    let out: String = env.get_string(&JString::from(out.unwrap())).unwrap().into();
    assert_eq!(out, "ababab,7,42,true,1,0,true");
    let mapped = env
        .call_static_method(
            "dev/yog/nanite/ReflectProbe",
            "runMapped",
            "()Ljava/lang/String;",
            &[],
        )
        .and_then(|v| v.l())
        .unwrap();
    let mapped: String = env.get_string(&JString::from(mapped)).unwrap().into();
    assert_eq!(
        mapped, "xx,7",
        "named members found by their run-time names"
    );
    if jvm.mixin.is_none() {
        return;
    }
    let _ = crate::globals::JAVA_VM
        .set(unsafe { jni::JavaVM::from_raw(jvm.vm.get_java_vm_pointer()) }.unwrap());
    crate::bus::jvm_support::register_natives(&mut env).unwrap();
    let made = string(&mut env, "generate");
    assert_eq!(
        made,
        "dev/yog/nanite/gen/Nanite0|Lorg/spongepowered/asm/mixin/Mixin;|[targets, [dev.yog.nanite.InjectProbe], remap, false]\
         |yog$nanite$0|(Ljava/lang/String;ILorg/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable;)V\
         |Lorg/spongepowered/asm/mixin/injection/Inject;|[counted(Ljava/lang/String;I)I]"
    );

    let at = ROUTER.resolve(&yog_event::keyed(yog_ids::event_nanite::INJECT, POINT));
    ROUTER.on(
        at,
        0,
        Handler::Native {
            owner: None,
            ud: 0,
            f: answer,
        },
    );
    assert_eq!(string(&mut env, "fire"), format!("true,{ANSWER}"));

    // A modify_arg adds 100 to the argument; a redirect answers 7.
    let on = |point: &str, f: yog_abi::YogHandlerFn| {
        let at = ROUTER.resolve(&yog_event::keyed(yog_ids::event_nanite::INJECT, point));
        ROUTER.on(
            at,
            0,
            Handler::Native {
                owner: None,
                ud: 0,
                f,
            },
        );
    };
    on("test:modify", add_hundred);
    on("test:redirect", answer_seven);
    assert_eq!(string(&mut env, "modifyAndRedirect"), "105,7");
    assert_eq!(
        string(&mut env, "redirectUnanswered"),
        "50",
        "unanswered: the call as it was"
    );

    // An added method answered by a handler; an added field; an interface.
    on("test:added", answer_seven);
    assert_eq!(
        string(&mut env, "added"),
        "7,yog$probe$chargeI,[java/lang/Runnable]"
    );

    // A field's read answered 7 (then read as it is: 5); its write answered
    // (not stored: still 5), then not (stored: 11).
    on("test:read", answer_seven);
    on("test:write", answer_seven);
    assert_eq!(string(&mut env, "fields"), "7,5,5,11");

    // A local asked for and written: 21 doubled by a handler here.
    on("test:locals", double_local);
    assert_eq!(string(&mut env, "locals"), "42");
}

/// A table renaming the probe: `NamedProbe` is `ReflectProbe` at run time,
/// its `double` `twice`, its `hidden` `secret`.
fn write_table(classes: &Path) {
    let member = |named: &str, desc: &str, runtime: &str| MemberMap {
        named: named.into(),
        desc: desc.into(),
        runtime: runtime.into(),
    };
    let table = Mappings {
        runtime: yog_ids::mapping_ns::INTERMEDIARY.into(),
        classes: vec![ClassMap {
            named: "dev/yog/nanite/NamedProbe".into(),
            runtime: "dev/yog/nanite/ReflectProbe".into(),
            fields: vec![member("hidden", "", "secret")],
            methods: vec![member(
                "double",
                "(Ljava/lang/String;I)Ljava/lang/String;",
                "twice",
            )],
        }],
    };
    std::fs::write(
        classes.join(yog_ids::mapping_file::NAME),
        yog_data::to_bytes(&table),
    )
    .unwrap();
}

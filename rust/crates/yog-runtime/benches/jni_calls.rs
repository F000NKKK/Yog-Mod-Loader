//! Rust → Java call cost on a real JVM: the by-name `call_static_method`
//! path the runtime used to take for every host call vs. the per-call-site
//! cached method ids of `jcall!`.
//!
//! Needs a JDK (`JAVA_HOME`, else `/usr/lib/jvm/java-21-openjdk-amd64`):
//! the stand-in host classes in `benches/java/` are compiled with its
//! `javac` and the JVM is loaded from its `libjvm`. Skips if there's none.

use std::path::{Path, PathBuf};

use criterion::{Criterion, criterion_group, criterion_main};
use jni::JNIEnv;
use jni::objects::JValue;
use yog_abi::YogStr;

#[path = "../src/jni/call.rs"]
#[allow(dead_code)]
mod jcall;
use jcall::jcall;

#[path = "../src/testing/jvm.rs"]
mod test_jvm;
use test_jvm::jdk;

/// Compiles the stand-in host classes; returns the class path.
fn compile_stubs(jdk: &Path) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/java");
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("jni-bench-classes");
    let sources: Vec<PathBuf> = ["NativeBridge.java", "render/NativeDraw.java"]
        .iter()
        .map(|f| src.join("dev/yog").join(f))
        .collect();
    assert!(test_jvm::javac(jdk, &out, &sources), "javac failed");
    out
}

/// What every host call used to do: look up by name, then call.
fn by_name_rect(env: &mut JNIEnv) {
    let _ = env.call_static_method(
        "dev/yog/render/NativeDraw",
        "drawRect",
        "(FFFFI)V",
        &[
            JValue::Float(1.0),
            JValue::Float(2.0),
            JValue::Float(30.0),
            JValue::Float(40.0),
            JValue::Int(-1),
        ],
    );
}

fn by_name_text(env: &mut JNIEnv, text: &str) {
    let jt = env.new_string(text).unwrap();
    let _ = env.call_static_method(
        "dev/yog/render/NativeDraw",
        "drawText",
        "(Ljava/lang/String;FFIZ)V",
        &[
            JValue::Object(&jt),
            JValue::Float(4.0),
            JValue::Float(8.0),
            JValue::Int(-1),
            JValue::Bool(1),
        ],
    );
    let _ = env.delete_local_ref(jt);
}

fn by_name_get_block(env: &mut JNIEnv, dim: &str) -> Option<String> {
    let jd = env.new_string(dim).unwrap();
    let ret = env
        .call_static_method(
            "dev/yog/NativeBridge",
            "getBlock",
            "(Ljava/lang/String;III)Ljava/lang/String;",
            &[
                JValue::Object(&jd),
                JValue::Int(1),
                JValue::Int(64),
                JValue::Int(-3),
            ],
        )
        .and_then(jni::objects::JValueGen::l)
        .ok()?;
    let ret = jni::objects::JString::from(ret);
    let s = env.get_string(&ret).ok().map(String::from);
    let _ = env.delete_local_ref(ret);
    let _ = env.delete_local_ref(jd);
    s
}

fn by_name_teleport(env: &mut JNIEnv, player: &str) -> bool {
    let jp = env.new_string(player).unwrap();
    let ok = env
        .call_static_method(
            "dev/yog/NativeBridge",
            "teleport",
            "(Ljava/lang/String;DDD)Z",
            &[
                JValue::Object(&jp),
                JValue::Double(0.5),
                JValue::Double(64.0),
                JValue::Double(-3.5),
            ],
        )
        .and_then(jni::objects::JValueGen::z)
        .unwrap_or(false);
    let _ = env.delete_local_ref(jp);
    ok
}

/// Calls before a path is measured: past HotSpot's compile thresholds, so
/// what's measured is the steady state a game runs in, not the first calls.
const WARM_CALLS: usize = 100_000;

/// Runs `f` until the JIT has compiled the Java it reaches.
fn warm(mut f: impl FnMut()) {
    for _ in 0..WARM_CALLS {
        f();
    }
}

fn bench(c: &mut Criterion) {
    let Some(jdk) = jdk() else {
        eprintln!("jni_calls: no JDK (set JAVA_HOME) — skipped");
        return;
    };
    let vm = test_jvm::start(&jdk, &compile_stubs(&jdk));
    let mut env = vm.attach_current_thread().unwrap();
    let text = YogStr::from_str("Hello, Yog!");
    let dim = YogStr::from_str("minecraft:overworld");

    warm(|| by_name_rect(&mut env));
    warm(|| by_name_text(&mut env, "Hello, Yog!"));
    warm(|| {
        by_name_get_block(&mut env, "minecraft:overworld");
    });
    warm(|| {
        by_name_teleport(&mut env, "Steve");
    });

    let mut g = c.benchmark_group("jni");
    g.bench_function("drawRect/by_name", |b| b.iter(|| by_name_rect(&mut env)));
    g.bench_function("drawRect/cached", |b| {
        b.iter(|| unsafe {
            let _: Option<()> = jcall!(
                &mut env,
                "dev/yog/render/NativeDraw",
                "drawRect",
                [F(1.0), F(2.0), F(30.0), F(40.0), I(-1)]
            );
        });
    });
    g.bench_function("drawText/by_name", |b| {
        b.iter(|| by_name_text(&mut env, "Hello, Yog!"));
    });
    g.bench_function("drawText/cached", |b| {
        b.iter(|| unsafe {
            let _: Option<()> = jcall!(
                &mut env,
                "dev/yog/render/NativeDraw",
                "drawText",
                [S(text), F(4.0), F(8.0), I(-1), Z(true)]
            );
        });
    });
    g.bench_function("getBlock/by_name", |b| {
        b.iter(|| by_name_get_block(&mut env, "minecraft:overworld"));
    });
    g.bench_function("getBlock/cached", |b| {
        b.iter(|| unsafe {
            let s: Option<String> = jcall!(
                &mut env,
                "dev/yog/NativeBridge",
                "getBlock",
                [S(dim), I(1), I(64), I(-3)]
            );
            s
        });
    });
    g.bench_function("teleport/by_name", |b| {
        b.iter(|| by_name_teleport(&mut env, "Steve"));
    });
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);

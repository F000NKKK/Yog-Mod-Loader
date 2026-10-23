//! A whole call on the bus with a real JVM, as a mod makes it: the payload
//! packed, through the arena to the host's Java (`YogBus.java`, the
//! generated records) and back, the reply read. Rust → Java with the
//! messages mods ask for most (a point, an inventory, a 32³ block area),
//! Java → Rust, and Rust → Rust for scale. The JIT is warmed first: what's
//! measured is the steady state a game runs in.
//!
//! Needs a JDK (`JAVA_HOME`, else `/usr/lib/jvm/java-21-openjdk-amd64`):
//! `cargo bench -p yog-runtime --features bench --bench bus_calls`.

use std::hint::black_box;
use std::os::raw::c_void;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use jni::JNIEnv;
use jni::objects::JValue;
use yog_abi::{YogBytes, YogReply, YogStatus, status};
use yog_core::{BlockArea, Point, SlotStack};
use yog_data::frame::{read_frame, read_view, to_frame};
use yog_runtime::testing::bench_support::jvm;

/// Calls before a path is measured: past HotSpot's compile thresholds.
const WARM_CALLS: usize = 50_000;
/// Fewer for the big replies: each is a lot of work already.
const WARM_BIG_CALLS: usize = 2_000;
/// A block inside the area, read back.
const SOME_BLOCK: usize = 30_000;

/// Compiles the host's bus classes and the probe; the class path.
fn compile(jdk: &Path) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let java = root.join("../../../java-common/src/main/java/dev/yog");
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("bus-bench-classes");
    let sources: Vec<PathBuf> = [
        "YogBus.java",
        "YogMessage.java",
        "YogAtoms.java",
        "YogNames.java",
        "data/YogFrame.java",
        "data/YogData.java",
        "bus/YogTypes.java",
    ]
    .map(|f| java.join(f))
    .into_iter()
    // The tests' stand-in logger: no SLF4J here.
    .chain([
        root.join("tests/java/dev/yog/YogLog.java"),
        root.join("benches/java/bus/dev/yog/BusProbe.java"),
    ])
    .collect();
    assert!(jvm::javac(jdk, &out, &sources), "javac failed");
    out
}

/// A Rust provider: the point moved, as the host's Java one does.
unsafe extern "C" fn point_back(
    _ud: *mut c_void,
    payload: YogBytes,
    reply: *const YogReply,
) -> YogStatus {
    let Some(p) = read_frame::<Point>(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    unsafe { YogReply::send(reply, &to_frame(&Point { x: p.x + 1.0, ..p })) };
    status::OK
}

fn probe(env: &mut JNIEnv, method: &str, address: &str) {
    let a = env.new_string(address).expect("a string");
    env.call_static_method(
        "dev/yog/BusProbe",
        method,
        "(Ljava/lang/String;)V",
        &[JValue::Object(&a)],
    )
    .expect("the probe");
}

fn warm(n: usize, mut f: impl FnMut()) {
    for _ in 0..n {
        f();
    }
}

fn bench(c: &mut Criterion) {
    let Some(jdk) = jvm::jdk() else {
        eprintln!("bus_calls: no JDK (set JAVA_HOME) — skipped");
        return;
    };
    let vm = jvm::start(&jdk, &compile(&jdk));
    jvm::adopt(&vm);
    let mut env = vm.attach_current_thread().expect("attached");
    jvm::register_natives(&mut env).expect("natives");
    for (method, address) in [
        ("providePoint", "bench:point"),
        ("provideInventory", "bench:inventory"),
        ("provideArea", "bench:area"),
    ] {
        probe(&mut env, method, address);
    }
    jvm::provide_native("bench:rust_point", point_back);
    let (point, inventory, area, rust_point) = (
        jvm::resolve("bench:point"),
        jvm::resolve("bench:inventory"),
        jvm::resolve("bench:area"),
        jvm::resolve("bench:rust_point"),
    );
    let request = to_frame(&Point {
        x: 12.5,
        y: 64.0,
        z: -3.25,
    });
    let mut reply = Vec::new();

    let mut call_point = |to| {
        jvm::call(to, &request, &mut reply);
        read_frame::<Point>(&reply).expect("a point")
    };
    warm(WARM_CALLS, || {
        call_point(point);
        call_point(rust_point);
    });
    let mut g = c.benchmark_group("bus");
    g.bench_function("point/rust→rust", |b| {
        b.iter(|| black_box(call_point(rust_point)))
    });
    g.bench_function("point/rust→java", |b| {
        b.iter(|| black_box(call_point(point)))
    });

    let mut reply = Vec::new();
    warm(WARM_CALLS, || {
        jvm::call(inventory, &[], &mut reply);
    });
    g.bench_function("inventory/rust→java, read whole", |b| {
        b.iter(|| {
            jvm::call(inventory, &[], &mut reply);
            black_box(read_frame::<Vec<SlotStack>>(&reply).expect("slots"))
        })
    });
    g.bench_function("inventory/rust→java, one slot viewed", |b| {
        b.iter(|| {
            jvm::call(inventory, &[], &mut reply);
            black_box(
                read_view::<Vec<SlotStack>>(&reply)
                    .expect("slots")
                    .get(40)
                    .map(|s| s.stack.count),
            )
        })
    });

    warm(WARM_BIG_CALLS, || {
        jvm::call(area, &[], &mut reply);
    });
    g.bench_function("area_32/rust→java, read whole", |b| {
        b.iter(|| {
            jvm::call(area, &[], &mut reply);
            black_box(read_frame::<BlockArea>(&reply).expect("an area"))
        })
    });
    g.bench_function("area_32/rust→java, one block viewed", |b| {
        b.iter(|| {
            jvm::call(area, &[], &mut reply);
            black_box(
                read_view::<BlockArea>(&reply)
                    .expect("an area")
                    .blocks
                    .get(SOME_BLOCK),
            )
        })
    });

    // Java → Rust: Java's own loop, timed whole.
    let address = env.new_string("bench:rust_point").expect("a string");
    let mut java_calls = |n: u64| {
        let start = Instant::now();
        env.call_static_method(
            "dev/yog/BusProbe",
            "callPoints",
            "(Ljava/lang/String;J)D",
            &[JValue::Object(&address), JValue::Long(n as i64)],
        )
        .expect("calls");
        start.elapsed()
    };
    java_calls(WARM_CALLS as u64);
    g.bench_function("point/java→rust", |b| {
        b.iter_custom(|n| java_calls(n).max(Duration::from_nanos(1)))
    });
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);

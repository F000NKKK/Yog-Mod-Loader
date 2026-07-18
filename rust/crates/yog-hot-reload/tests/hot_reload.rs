use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use yog_hot_reload::{HotReloader, ModuleGeneration, ModuleRegistry};

fn cdylib_ext() -> &'static str {
    if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    }
}

/// Compiles the fixture into its own distinct output file each call, so
/// "old" and "new" generations are genuinely separate `dlopen`s rather than
/// the same path reloaded (which some dynamic linkers just refcount).
fn build_fixture(tag: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest_dir.join("tests/fixtures/fixture.rs");
    let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let out = out_dir.join(format!("libyog_hot_reload_fixture_{tag}.{}", cdylib_ext()));

    let status = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args(["--crate-type=cdylib", "-o"])
        .arg(&out)
        .arg(&src)
        .status()
        .expect("failed to invoke rustc for fixture build");
    assert!(status.success(), "rustc failed to build the fixture cdylib");
    out
}

#[derive(Default)]
struct RecordingRegistry {
    retargets: Mutex<Vec<(String, ModuleGeneration, ModuleGeneration)>>,
    purges: Mutex<Vec<ModuleGeneration>>,
}

impl ModuleRegistry for RecordingRegistry {
    fn retarget(&self, mod_id: &str, from: ModuleGeneration, to: ModuleGeneration) {
        self.retargets.lock().unwrap().push((mod_id.to_string(), from, to));
    }

    fn purge(&self, generation: ModuleGeneration) {
        self.purges.lock().unwrap().push(generation);
    }
}

#[test]
fn defers_hard_unload_until_in_flight_call_completes() {
    let v1 = build_fixture("v1");
    let v2 = build_fixture("v2");

    let reloader = HotReloader::new(RecordingRegistry::default());
    let old_module = unsafe { reloader.load_initial("test-mod", &v1) }.expect("loading v1");
    let old_generation = old_module.generation();

    // Simulate a call still executing inside the old module while reload happens.
    let guard = old_module.enter().expect("old module should still be loaded");

    let new_module = unsafe { reloader.reload("test-mod", &v2) }.expect("reloading to v2");
    let new_generation = new_module.generation();

    assert_eq!(reloader.active("test-mod").unwrap().generation(), new_generation);
    assert_eq!(reloader.registry_retargets(), vec![("test-mod".to_string(), old_generation, new_generation)]);
    assert!(old_module.is_loaded(), "old module must stay loaded while a call is in flight");
    assert!(reloader.registry_purges().is_empty(), "purge must not run before it's safe to unload");

    drop(guard);

    assert!(!old_module.is_loaded(), "old module should hard-unload once the in-flight call finishes");
    assert_eq!(reloader.registry_purges(), vec![old_generation]);
}

#[test]
fn hard_unloads_immediately_when_nothing_is_in_flight() {
    let v1 = build_fixture("v1-immediate");
    let v2 = build_fixture("v2-immediate");

    let reloader = HotReloader::new(RecordingRegistry::default());
    let old_module = unsafe { reloader.load_initial("immediate-mod", &v1) }.expect("loading v1");
    let old_generation = old_module.generation();

    unsafe { reloader.reload("immediate-mod", &v2) }.expect("reloading to v2");

    assert!(!old_module.is_loaded(), "with nothing in flight, unload should happen right away");
    assert_eq!(reloader.registry_purges(), vec![old_generation]);
}

// Small accessors so the test can inspect the fake registry through the
// `HotReloader` without `HotReloader` itself needing to expose its
// internals for production use.
trait TestRegistryAccess {
    fn registry_retargets(&self) -> Vec<(String, ModuleGeneration, ModuleGeneration)>;
    fn registry_purges(&self) -> Vec<ModuleGeneration>;
}

impl TestRegistryAccess for HotReloader<RecordingRegistry> {
    fn registry_retargets(&self) -> Vec<(String, ModuleGeneration, ModuleGeneration)> {
        self.registry().retargets.lock().unwrap().clone()
    }

    fn registry_purges(&self) -> Vec<ModuleGeneration> {
        self.registry().purges.lock().unwrap().clone()
    }
}

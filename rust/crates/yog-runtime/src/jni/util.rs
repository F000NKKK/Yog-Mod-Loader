//! JNI helpers.
use crate::globals::JAVA_VM;

// ── JNI helpers ──────────────────────────────────────────────────────────────

/// Runs `f`, a panic in it caught and logged: one never crosses into Java
/// (it would take the game down).
pub(crate) fn guard(label: &str, f: impl FnOnce()) {
    guarded(label, (), f);
}

/// [`guard`] for what answers Java: `fallback` when `f` panicked.
pub(crate) fn guarded<T>(label: &str, fallback: T, f: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| {
        yog_logging::error!("a panic handling `{label}` (answered as nothing happened)");
        fallback
    })
}

pub(crate) fn attached_env() -> Option<jni::AttachGuard<'static>> {
    JAVA_VM.get()?.attach_current_thread().ok()
}

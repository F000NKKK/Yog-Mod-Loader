//! JNI helpers.

use super::*;

// ── JNI helpers ──────────────────────────────────────────────────────────────

pub(super) fn guard(label: &str, f: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
        yog_logging::error!("a mod panicked handling `{}` (ignored)", label);
    }
}

pub(super) fn get_env() -> Option<jni::AttachGuard<'static>> {
    JAVA_VM.get()?.attach_current_thread().ok()
}


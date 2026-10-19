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



/// A host string reply handed to a mod (`None`/`null` → [`YogOwnedStr::NONE`]).
pub(super) fn owned(s: Option<String>) -> YogOwnedStr {
    s.map_or(YogOwnedStr::NONE, YogOwnedStr::from_string)
}

/// The first `N` tab-separated values of a host reply.
pub(super) fn parse_tsv<T: std::str::FromStr, const N: usize>(s: &str) -> Option<[T; N]> {
    let values: Vec<T> = s
        .split(yog_ids::sep::FIELD)
        .take(N)
        .map(|v| v.parse().ok())
        .collect::<Option<_>>()?;
    values.try_into().ok()
}

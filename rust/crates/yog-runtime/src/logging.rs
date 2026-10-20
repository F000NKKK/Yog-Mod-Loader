//! Mods' logging (ABI 3.8).

use super::*;

// ── Logging (ABI 3.8) ────────────────────────────────────────────────────────

thread_local! {
    /// Set while a record is on its way to Java: a record the call itself
    /// logs (a JNI failure) goes to stderr instead of looping.
    static LOGGING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Writes a record to the game's log (`YogLog.log`); before there's a JVM, or from inside
/// a failing log call, to stderr.
pub(super) fn host_log(level: yog_logging::Level, message: &str) {
    let sent = !LOGGING.with(|busy| busy.replace(true)) && {
        let r: Option<()> = unsafe { host!("dev/yog/YogLog", "log", [JArg::I(level as i32), JArg::S(YogStr::from_str(message))]) };
        LOGGING.with(|busy| busy.set(false));
        r.is_some()
    };
    if !sent {
        eprintln!("[yog] [{level:?}] {message}");
    }
}

/// `YogApi::log`: a mod's record.
pub(super) unsafe extern "C" fn api_log(level: u32, message: YogStr) {
    host_log(yog_logging::Level::from_u32(level), unsafe { message.as_str() });
}

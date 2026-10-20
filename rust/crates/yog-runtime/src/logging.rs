//! The game's log: where the runtime's records and mods' (the `log/record` event) go.
use std::io::Write as _;

use yog_abi::YogStr;

thread_local! {
    /// Set while a record is on its way to Java: a record the call itself
    /// logs (a JNI failure) goes to stderr instead of looping.
    static LOGGING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Writes a record to the game's log (`YogLog.log`); before there's a JVM, or from inside
/// a failing log call, to stderr.
pub(super) fn host_log(level: yog_logging::Level, message: &str) {
    let sent = !LOGGING.with(|busy| busy.replace(true)) && {
        let r: Option<()> = unsafe {
            host!(
                "dev/yog/YogLog",
                "log",
                [I(level as i32), S(YogStr::from_str(message))]
            )
        };
        LOGGING.with(|busy| busy.set(false));
        r.is_some()
    };
    if !sent {
        // The log's own last resort: the process's standard error.
        let _ = writeln!(std::io::stderr(), "[yog] [{level:?}] {message}");
    }
}

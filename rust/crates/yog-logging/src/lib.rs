//! Yog logging — lightweight logging macros with a consistent `[yog]` format.
//!
//! The macros forward to [`log`], which does the formatting. Because they expand
//! to a single call that only formats its arguments when actually invoked, there
//! is no cost at a call site that is compiled out or behind a disabled level.
//!
//! ```
//! yog_logging::info!("loaded {} mods", 3);
//! yog_logging::warn!("slow tick: {}ms", 51);
//! yog_logging::error!("failed: {}", "boom");
//! ```

use std::io::Write;

/// Severity of a log record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    #[inline]
    const fn tag(self) -> &'static str {
        match self {
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

/// Write one formatted record. INFO/WARN go to stdout, ERROR to stderr so it
/// interleaves with the host (Minecraft) console the way operators expect.
#[doc(hidden)]
#[inline]
pub fn log(level: Level, args: std::fmt::Arguments<'_>) {
    if matches!(level, Level::Error) {
        let mut out = std::io::stderr().lock();
        let _ = writeln!(out, "[yog] [{}] {}", level.tag(), args);
    } else {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "[yog] [{}] {}", level.tag(), args);
    }
}

/// Log at INFO level.
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log($crate::Level::Info, ::core::format_args!($($arg)*))
    };
}

/// Log at WARN level.
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log($crate::Level::Warn, ::core::format_args!($($arg)*))
    };
}

/// Log at ERROR level.
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log($crate::Level::Error, ::core::format_args!($($arg)*))
    };
}

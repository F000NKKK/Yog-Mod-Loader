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
use std::sync::OnceLock;

/// Severity of a log record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Level {
    Info = 0,
    Warn = 1,
    Error = 2,
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

    /// The level a host passes as a number ([`Level`] as `u32`); unknown
    /// numbers are errors, so nothing is quieter than it was meant to be.
    pub const fn from_u32(n: u32) -> Level {
        match n {
            0 => Level::Info,
            1 => Level::Warn,
            _ => Level::Error,
        }
    }
}

/// Where records go once a host has a log of its own (the game's): the
/// runtime and every mod install it once at start, so their records land in
/// the game's log with everything else.
pub type Sink = fn(Level, &str);

static SINK: OnceLock<Sink> = OnceLock::new();

/// Sends every record from now on to `sink` instead of stdout/stderr. The
/// first call wins: a process has one log.
pub fn set_sink(sink: Sink) {
    let _ = SINK.set(sink);
}

/// Formats and writes one record — to the installed [`Sink`], or, before
/// there is one, to stderr (errors) or stdout.
#[doc(hidden)]
#[inline]
pub fn log(level: Level, args: std::fmt::Arguments<'_>) {
    if let Some(sink) = SINK.get() {
        sink(level, &args.to_string());
        return;
    }
    let line = format_args!("[yog] [{}] {}", level.tag(), args);
    // Nowhere else to report a failed write of a log line.
    if matches!(level, Level::Error) {
        let _ = writeln!(std::io::stderr().lock(), "{line}");
    } else {
        let _ = writeln!(std::io::stdout().lock(), "{line}");
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

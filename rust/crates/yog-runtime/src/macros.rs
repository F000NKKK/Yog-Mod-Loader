//! The runtime's macros, declared before every module that uses them.

/// Reads a Java string, returning `$or` (default: `Default::default()`)
/// from the enclosing function when it can't be read.
macro_rules! jstr {
    ($env:expr_2021, $s:expr_2021) => {
        jstr!($env, $s, Default::default())
    };
    ($env:expr_2021, $s:expr_2021, $or:expr_2021) => {
        match $env.get_string(&$s) {
            Ok(s) => String::from(s),
            Err(_) => return $or,
        }
    };
}

/// `<class>.<name>(args...)` on this thread's env, `Option<R>` — see
/// [`jcall!`]; `None` without a JVM too.
macro_rules! host {
    ($class:literal, $name:literal, [$($arg:expr_2021),* $(,)?]) => {
        match $crate::get_env() {
            Some(mut env) => $crate::jcall::jcall!(&mut env, $class, $name, [$($arg),*]),
            None => None,
        }
    };
}

/// `NativeBridge.<name>(args...)` — see [`host!`].
macro_rules! bridge {
    ($name:literal, [$($arg:expr_2021),* $(,)?]) => {
        host!("dev/yog/NativeBridge", $name, [$($arg),*])
    };
}

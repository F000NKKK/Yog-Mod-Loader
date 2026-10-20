//! The runtime's macros, declared before every module that uses them.

/// The `yog-hot-reload` host contract: `retarget` needs no work here because
/// `LoadedModule::is_retiring()` already makes a superseded generation's
/// dispatch-table entries inert (every `handlers().<field>` loop checks it)
/// the instant `HotReloader::reload` calls `retire` on the old module —
/// well before this is even invoked. `purge` is the actual safety-net
/// sweep, dropping every entry tagged with a retired generation right
/// before its library is hard-unloaded.
/// Runs `$body` with `$list` bound to each per-event handler list of `$h`.
macro_rules! for_each_handler_list {
    ($h:expr_2021, |$list:ident| $body:expr_2021) => {{
        {
            let $list = &mut $h.item_insert;
            $body;
        }
        {
            let $list = &mut $h.hud_render;
            $body;
        }
        {
            let $list = &mut $h.body;
            $body;
        }
        {
            let $list = &mut $h.camera;
            $body;
        }
        {
            let $list = &mut $h.travel;
            $body;
        }
        {
            let $list = &mut $h.world_render;
            $body;
        }
    }};
}

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

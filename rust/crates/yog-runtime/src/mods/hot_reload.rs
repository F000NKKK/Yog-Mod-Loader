//! Hot reload (dev builds): a mod rebuilt while the game runs, loaded as a
//! new generation and registered again.

use std::path::Path;

use libloading::Symbol;
use yog_abi::{ABI_VERSION, YogApi};

use crate::globals::API_TABLE;
use crate::mods::handlers::hot_reloader;
use crate::mods::loading::{AbiVersionFn, extract_yog, invoke_mod_register};

/// Rebuilds and re-registers `mod_id` from a freshly built `.yog` at
/// `new_yog_path`: extracts the current platform's native, loads it as a
/// new generation via the shared `HotReloader` (which retires the previous
/// generation per its hybrid soft/hard-unload policy — hard-unloaded right
/// away if nothing's mid-call, deferred otherwise), then calls its
/// `yog_mod_register` again so its handlers get tagged under the new
/// generation. How this actually gets triggered — a keybind, a file-watch,
/// an IDE-driven socket — is a separate, later concern; this only needs to
/// work correctly once called.
pub(crate) fn trigger_hot_reload(mod_id: &str, new_yog_path: &Path) -> bool {
    let Some(native_path) = extract_yog(new_yog_path) else {
        yog_logging::error!(
            "hot-reload {}: no native for {} in {}",
            mod_id,
            yog_ids::platform::host_tag(),
            new_yog_path.display()
        );
        return false;
    };

    let api: &'static YogApi = &API_TABLE;

    let module = match unsafe { hot_reloader().reload(mod_id, &native_path) } {
        Ok(m) => m,
        Err(e) => {
            yog_logging::error!(
                "hot-reload {}: failed to load {}: {}",
                mod_id,
                native_path.display(),
                e
            );
            return false;
        }
    };

    let abi = module.with_library(|lib| unsafe {
        let abi: Symbol<AbiVersionFn> = lib.get(b"yog_abi_version").ok()?;
        Some(abi())
    });
    match abi.flatten() {
        Some(v) if yog_abi::compatible(v) => {}
        other => {
            yog_logging::error!(
                "hot-reload {}: ABI {:?} incompatible with runtime ABI {} — mod has no active handlers until a compatible reload succeeds",
                mod_id,
                other,
                ABI_VERSION
            );
            return false;
        }
    }

    if let Err(e) = invoke_mod_register(&module, api, mod_id) {
        yog_logging::error!("hot-reload {}: {}", mod_id, e);
        return false;
    }

    yog_logging::info!("hot-reloaded mod {}", mod_id);
    // Its scripts too: the host keeps their state and loads the new classes
    // (no host answering, as in tests: nothing to reload).
    yog_event::script::reload(mod_id, new_yog_path.to_string_lossy().into_owned());
    true
}

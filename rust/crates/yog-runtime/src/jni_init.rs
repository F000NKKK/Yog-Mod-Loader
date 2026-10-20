//! JNI: the runtime's start (`nativeInit`).

use super::*;

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeInit<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    mods_dir: JString<'l>,
) {
    if let Ok(vm) = env.get_java_vm() {
        let _ = JAVA_VM.set(vm);
    }
    // From here on the runtime's records go to the game's log.
    yog_logging::set_sink(host_log);

    let dir = env
        .get_string(&mods_dir)
        .map(String::from)
        .unwrap_or_default();

    // Publish HANDLERS *before* any mod registers — its address is then
    // `'static`-stable for the runtime's entire lifetime, unlike the old
    // design (a temporary heap `Box` moved into `HANDLERS` only after
    // `nativeInit` returned, silently invalidating the `ctx` pointer any
    // later re-registration — e.g. a hot-reload — would have needed).
    let handlers_lock = HANDLERS.get_or_init(|| RwLock::new(RuntimeHandlers::default()));
    let handlers_ptr = handlers_lock as *const RwLock<RuntimeHandlers>;

    let api: &'static YogApi = API_TABLE.get_or_init(|| build_api_table(handlers_ptr));
    crate::bus::register_duties();

    let _ = HOT_RELOADER.get_or_init(|| HotReloader::new(RuntimeModuleRegistry));
    // A book's entries an advancement unlocks wait for this client's player to have it.
    yog_book::set_unlocked(client_advancement_done);
    // A book's recipe shows a tag's items in turn.
    yog_book::set_tag_items(client_tag_items);

    guard("mod loading", || {
        load_mods(Path::new(&dir), api);
    });

    #[cfg(feature = "dev")]
    control_socket::start_if_requested();

    yog_logging::info!("runtime initialised — the gate is open.");
}

/// Rebuilds and re-registers `mod_id` from a freshly built `.yog` at
/// `new_yog_path`: extracts the current platform's native, loads it as a
/// new generation via the shared `HotReloader` (which retires the previous
/// generation per its hybrid soft/hard-unload policy — hard-unloaded right
/// away if nothing's mid-call, deferred otherwise), then calls its
/// `yog_mod_register` again so its handlers get tagged under the new
/// generation. How this actually gets triggered — a keybind, a file-watch,
/// an IDE-driven socket — is a separate, later concern; this only needs to
/// work correctly once called.
#[cfg(feature = "dev")]
pub(crate) fn trigger_hot_reload(mod_id: &str, new_yog_path: &Path) -> bool {
    let Some(native_path) = extract_yog(new_yog_path) else {
        yog_logging::error!(
            "hot-reload {}: no native for {} in {}",
            mod_id,
            platform_tag(),
            new_yog_path.display()
        );
        return false;
    };

    let api = API_TABLE.get().expect("yog: nativeInit not called yet");

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
    true
}

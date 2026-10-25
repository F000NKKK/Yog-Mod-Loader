//! Loading mods.
use crate::api::machine::YOG_MOD_IDS;
use crate::globals::CURRENT_REGISTRATION;
use crate::globals::MOD_INFOS;
use crate::mods::handlers::hot_reloader;
use libloading::Symbol;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use yog_abi::ABI_VERSION;
use yog_abi::YogApi;
use yog_formats::manifest::Built;
use yog_hot_reload::LoadedModule;

// ── Mod loading ───────────────────────────────────────────────────────────────

pub(crate) type AbiVersionFn = unsafe extern "C" fn() -> u32;
/// Second arg is a null-terminated C string: the mod's `id` from its manifest.
pub(crate) type RegisterFn = unsafe extern "C" fn(*const YogApi, *const std::os::raw::c_char);

pub(crate) fn load_mods(dir: &Path, api: &YogApi) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => {
            yog_logging::info!("no mods directory at {} — none loaded", dir.display());
            return;
        }
    };

    // First pass: what each `.yog` says about its mod.
    struct ModMeta {
        info: Built,
        yog_path: PathBuf,
    }
    let metas: Vec<ModMeta> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| yog_ids::Format::Mod.is(path))
        .map(|yog_path| ModMeta {
            info: read_mod_info(&yog_path),
            yog_path,
        })
        .collect();

    // Topological sort by dependencies (Kahn's algorithm)
    let ids: Vec<String> = metas.iter().map(|m| m.info.id.clone()).collect();
    // Known before any registers: a mod's optional integrations with a Yog
    // mod loaded after it still see it.
    YOG_MOD_IDS.write().extend(ids.iter().cloned());
    let deps_list: Vec<Vec<String>> = metas.iter().map(|m| m.info.dependencies.clone()).collect();
    let sorted = match topo_sort_mods(&ids, &deps_list) {
        Some(v) => v,
        None => {
            yog_logging::error!("circular dependency detected — loading in file order");
            (0..metas.len()).collect()
        }
    };

    let mut loaded: Vec<LoadedModule> = Vec::new();
    for idx in sorted {
        let meta = &metas[idx];
        // No native at all: a Yog Script mod — the host loads its scripts.
        if meta.info.platforms.is_empty() {
            yog_logging::info!(
                "{}: no native (its Yog Script loads in the host)",
                meta.info.id
            );
            MOD_INFOS.lock().push(meta.info.clone());
            continue;
        }
        let lib_path = match extract_yog(&meta.yog_path) {
            Some(p) => p,
            None => {
                yog_logging::error!(
                    "no native for {} in {}",
                    yog_ids::platform::host_tag(),
                    meta.yog_path.display()
                );
                continue;
            }
        };
        if let Some(module) = load_mod_lib(&lib_path, api, &meta.info.id) {
            loaded.push(module);
            #[cfg(feature = "dev")]
            crate::mods::registrations::record(&meta.info.id, &meta.yog_path);
            MOD_INFOS.lock().push(meta.info.clone());
        }
    }
    yog_logging::info!("loaded {} mod(s) from {}", loaded.len(), dir.display());
}

/// Kahn's algorithm — returns indices in dependency order, or None on cycle.
pub(crate) fn topo_sort_mods(ids: &[String], deps_list: &[Vec<String>]) -> Option<Vec<usize>> {
    let n = ids.len();
    let name_to_idx: HashMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let mut in_degree = vec![0u32; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, deps) in deps_list.iter().enumerate() {
        for dep in deps {
            if let Some(&j) = name_to_idx.get(dep.as_str()) {
                adj[j].push(i);
                in_degree[i] += 1;
            }
        }
    }
    let mut queue: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(u) = queue.pop() {
        order.push(u);
        for &v in &adj[u] {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push(v);
            }
        }
    }
    if order.len() == n { Some(order) } else { None }
}

/// Loads `path` as a fresh generation of `mod_id` via the shared
/// `HotReloader`, validates its ABI, and calls its `yog_mod_register` —
/// tagging every handler it registers with the returned `LoadedModule` (see
/// `current_registration()`). On any failure the module (if it got as far
/// as being tracked by the `HotReloader`) is retired immediately: nothing
/// can be executing inside a library that never successfully registered,
/// so this always hard-unloads right away rather than lingering.
pub(crate) fn load_mod_lib(path: &Path, api: &YogApi, mod_id: &str) -> Option<LoadedModule> {
    let module = match unsafe { hot_reloader().load_initial(mod_id, path) } {
        Ok(m) => m,
        Err(e) => {
            yog_logging::error!("failed to load {}: {}", path.display(), e);
            return None;
        }
    };

    let abi = module.with_library(|lib| unsafe {
        let abi: Symbol<AbiVersionFn> = lib.get(b"yog_abi_version").ok()?;
        Some(abi())
    });
    match abi.flatten() {
        Some(v) if yog_abi::compatible(v) => {}
        Some(v) => {
            yog_logging::error!(
                "{}: ABI {} incompatible with runtime ABI {}",
                path.display(),
                v,
                ABI_VERSION
            );
            module.retire(|| {});
            return None;
        }
        None => {
            yog_logging::error!("{} is not a Yog mod (no yog_abi_version)", path.display());
            module.retire(|| {});
            return None;
        }
    }

    if let Err(e) = invoke_mod_register(&module, api, mod_id) {
        yog_logging::error!("{}: {}", path.display(), e);
        module.retire(|| {});
        return None;
    }

    Some(module)
}

/// Calls a loaded module's `yog_mod_register`, tagging everything it
/// registers with `module` via `CURRENT_REGISTRATION` for the duration of
/// the call — used for both a mod's initial load and a later hot-reload.
pub(crate) fn invoke_mod_register(
    module: &LoadedModule,
    api: &YogApi,
    mod_id: &str,
) -> Result<(), String> {
    module
        .with_library(|lib| unsafe {
            let register: Symbol<RegisterFn> = lib
                .get(b"yog_mod_register")
                .map_err(|_| "missing yog_mod_register".to_string())?;
            let mod_id_c = std::ffi::CString::new(mod_id)
                .map_err(|_| format!("mod id {mod_id:?} holds a NUL"))?;
            *CURRENT_REGISTRATION.lock() = Some(module.clone());
            register(api as *const YogApi, mod_id_c.as_ptr());
            *CURRENT_REGISTRATION.lock() = None;
            Ok(())
        })
        .unwrap_or_else(|| Err("module was unloaded before registration could run".to_string()))
}

/// What a mod file says about its mod: a `.yog`'s manifest; a bare native
/// library (or a `.yog` without one) goes by its file's stem.
pub(crate) fn read_mod_info(path: &Path) -> Built {
    let built = yog_ids::Format::Mod.is(path).then(|| {
        let file = std::fs::File::open(path).ok()?;
        let mut archive = zip::ZipArchive::new(file).ok()?;
        let mut entry = archive.by_name(yog_ids::archive::MANIFEST).ok()?;
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes).ok()?;
        yog_data::read_bytes::<Built>(&bytes).ok()
    });
    built.flatten().unwrap_or_else(|| {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Built {
            id: stem.clone(),
            name: stem,
            ..Built::default()
        }
    })
}

pub(crate) fn extract_yog(path: &Path) -> Option<PathBuf> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let prefix = format!(
        "{}/{}/",
        yog_ids::archive::NATIVES,
        yog_ids::platform::host_tag()
    );
    let mut entry_name = None;
    for i in 0..archive.len() {
        let f = archive.by_index(i).ok()?;
        let name = f.name().ok()?;
        if name.starts_with(&prefix) && !name.ends_with('/') {
            entry_name = Some(name.to_string());
            break;
        }
    }
    let entry_name = entry_name?;
    let ext = Path::new(&entry_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    // A fresh name per extraction: a hot reload must never truncate the
    // library the game still has mapped, and `dlopen` of an already-loaded
    // path would just hand back the old module.
    static EXTRACTIONS: yog_sync::atomic::AtomicU32 = yog_sync::atomic::AtomicU32::new(0);
    let n = EXTRACTIONS.fetch_add(1, yog_sync::atomic::Ordering::Relaxed);
    let out =
        std::env::temp_dir().join(format!("yog-{}-{}-{}.{}", stem, std::process::id(), n, ext));
    let mut entry = archive.by_name(&entry_name).ok()?;
    let mut out_file = std::fs::File::create(&out).ok()?;
    std::io::copy(&mut entry, &mut out_file).ok()?;
    Some(out)
}

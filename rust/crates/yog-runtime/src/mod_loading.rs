//! Loading mods.

use super::*;

// ── Mod loading ───────────────────────────────────────────────────────────────

pub(super) fn platform_tag() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

pub(super) type AbiVersionFn = unsafe extern "C" fn() -> u32;
/// Second arg is a null-terminated C string: the mod's `id` from its manifest.
pub(super) type RegisterFn = unsafe extern "C" fn(*const YogApi, *const std::os::raw::c_char);

pub(super) fn load_mods(dir: &Path, api: &YogApi) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => {
            yog_logging::info!("no mods directory at {} — none loaded", dir.display());
            return;
        }
    };

    // First pass: collect (mod_id, dependencies, archive_path)
    struct ModMeta {
        id: String,
        deps: Vec<String>,
        yog_path: PathBuf,
    }
    let mut metas: Vec<ModMeta> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yog") {
            continue;
        }
        let info = read_mod_info(&path);
        let deps = read_mod_deps(&path);
        metas.push(ModMeta {
            id: info[0].clone(),
            deps,
            yog_path: path,
        });
    }

    // Topological sort by dependencies (Kahn's algorithm)
    let ids: Vec<String> = metas.iter().map(|m| m.id.clone()).collect();
    // Known before any registers: a mod's optional integrations with a Yog
    // mod loaded after it still see it.
    YOG_MOD_IDS.write().unwrap_or_else(|e| e.into_inner()).extend(ids.iter().cloned());
    let deps_list: Vec<Vec<String>> = metas.iter().map(|m| m.deps.clone()).collect();
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
        let lib_path = match extract_yog(&meta.yog_path) {
            Some(p) => p,
            None => {
                yog_logging::error!(
                    "no native for {} in {}",
                    platform_tag(),
                    meta.yog_path.display()
                );
                continue;
            }
        };
        if let Some(module) = load_mod_lib(&lib_path, api, &meta.id) {
            loaded.push(module);
            #[cfg(feature = "dev")]
            registrations::record(&meta.id, &meta.yog_path);
            MOD_INFOS
                .locked()
                .push(read_mod_info(&meta.yog_path));
        }
    }
    yog_logging::info!("loaded {} mod(s) from {}", loaded.len(), dir.display());
}

/// Kahn's algorithm — returns indices in dependency order, or None on cycle.
pub(super) fn topo_sort_mods(ids: &[String], deps_list: &[Vec<String>]) -> Option<Vec<usize>> {
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
pub(super) fn load_mod_lib(path: &Path, api: &YogApi, mod_id: &str) -> Option<LoadedModule> {
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
pub(super) fn invoke_mod_register(module: &LoadedModule, api: &YogApi, mod_id: &str) -> Result<(), String> {
    module
        .with_library(|lib| unsafe {
            let register: Symbol<RegisterFn> = lib
                .get(b"yog_mod_register")
                .map_err(|_| "missing yog_mod_register".to_string())?;
            let mod_id_c = std::ffi::CString::new(mod_id).map_err(|_| format!("mod id {mod_id:?} holds a NUL"))?;
            *CURRENT_REGISTRATION.locked() = Some(module.clone());
            register(api as *const YogApi, mod_id_c.as_ptr());
            *CURRENT_REGISTRATION.locked() = None;
            Ok(())
        })
        .unwrap_or_else(|| Err("module was unloaded before registration could run".to_string()))
}

/// Metadata for a mod file: [id, name, version, authors, description,
/// name_key, description_key].
/// For .yog archives this comes from the bundled yog.toml; bare native libs
/// fall back to the file stem.
pub(super) fn read_mod_info(path: &Path) -> [String; 7] {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut info = [
        stem.clone(),
        stem,
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    ];
    if path.extension().and_then(|e| e.to_str()) != Some("yog") {
        return info;
    }
    let Some(text) = (|| {
        let file = std::fs::File::open(path).ok()?;
        let mut archive = zip::ZipArchive::new(file).ok()?;
        let mut entry = archive.by_name("yog.toml").ok()?;
        let mut text = String::new();
        std::io::Read::read_to_string(&mut entry, &mut text).ok()?;
        Some(text)
    })() else {
        return info;
    };
    // Minimal parse — enough for id/name/version/description/authors without
    // pulling in a TOML dependency. Source manifests keep these under [mod];
    // the packaged manifest (written by yog-cli) has them at top level, so
    // keys before any section header count too.
    let mut in_mod = true;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_mod = line == "[mod]";
            continue;
        }
        if !in_mod {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let unquote = |v: &str| v.trim().trim_matches('"').to_string();
        match key {
            "id" => info[0] = unquote(value),
            "name" => info[1] = unquote(value),
            "version" => info[2] = unquote(value),
            "description" => info[4] = unquote(value),
            "name_key" => info[5] = unquote(value),
            "description_key" => info[6] = unquote(value),
            "authors" => {
                let list = value.trim_start_matches('[').trim_end_matches(']');
                let authors: Vec<String> = list
                    .split(',')
                    .map(unquote)
                    .filter(|a| !a.is_empty())
                    .collect();
                info[3] = authors.join(", ");
            }
            _ => {}
        }
    }
    info
}

/// Parse `[dependencies]` section from yog.toml inside a .yog archive.
pub(super) fn read_mod_deps(path: &Path) -> Vec<String> {
    if path.extension().and_then(|e| e.to_str()) != Some("yog") {
        return Vec::new();
    }
    let text = (|| {
        let file = std::fs::File::open(path).ok()?;
        let mut archive = zip::ZipArchive::new(file).ok()?;
        let mut entry = archive.by_name("yog.toml").ok()?;
        let mut text = String::new();
        std::io::Read::read_to_string(&mut entry, &mut text).ok()?;
        Some(text)
    })();
    let Some(text) = text else { return Vec::new() };
    let mut deps = Vec::new();
    let mut in_deps = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_deps = line == "[dependencies]";
            continue;
        }
        if !in_deps {
            continue;
        }
        // `mod_id = "1.0"` or `mod_id = "*"` — we only care about the key (mod_id)
        if let Some((key, _)) = line.split_once('=') {
            deps.push(key.trim().to_string());
        }
    }
    deps
}

pub(super) fn extract_yog(path: &Path) -> Option<PathBuf> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let prefix = format!("natives/{}/", platform_tag());
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
    static EXTRACTIONS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = EXTRACTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let out =
        std::env::temp_dir().join(format!("yog-{}-{}-{}.{}", stem, std::process::id(), n, ext));
    let mut entry = archive.by_name(&entry_name).ok()?;
    let mut out_file = std::fs::File::create(&out).ok()?;
    std::io::copy(&mut entry, &mut out_file).ok()?;
    Some(out)
}

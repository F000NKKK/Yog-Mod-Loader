//! The runtime's static globals: the JVM, the mods, the tables.

use super::*;

// ── Static globals ────────────────────────────────────────────────────────────

/// Cached JVM handle for any-thread callbacks.
pub(super) static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();
/// Loaded mod libraries, keyed by mod id — owns the safe `dlopen`/`dlclose`
/// swap lifecycle (see `yog-hot-reload`) instead of a flat keep-alive `Vec`,
/// so a reload can find "the current generation of this mod" and safely
/// retire the previous one.
pub(super) static HOT_RELOADER: OnceLock<HotReloader<RuntimeModuleRegistry>> = OnceLock::new();
/// The mod currently inside its own `yog_mod_register` call. Every `api_*`
/// registration function reads this to tag what it stores with a
/// `LoadedModule` handle, instead of requiring mods to pass a mod id
/// through the stable C ABI (an ABI break, and edits to every mod's
/// source). Valid because a mod's entire registration always happens
/// synchronously inside one `yog_mod_register` call — see `load_mod_lib`
/// and, in dev builds, `trigger_hot_reload`.
pub(super) static CURRENT_REGISTRATION: Mutex<Option<LoadedModule>> = Mutex::new(None);
/// Metadata of loaded .yog mods: [id, name, version, authors, description,
/// name_key, description_key] (the keys empty unless the mod's yog.toml
/// gives its name/description as `.yres` texts).
pub(super) static MOD_INFOS: Mutex<Vec<[String; 7]>> = Mutex::new(Vec::new());
/// Inter-mod symbol registry: mod_id → (symbol_name → function pointer).
/// Filled during `yog_mod_register` (export calls), read during import calls.
pub(super) static INTEROP_SYMBOLS: OnceLock<Mutex<HashMap<String, HashMap<String, usize>>>> = OnceLock::new();
/// Stable server table (populated once in nativeInit, then read-only).
pub(super) static SERVER: OnceLock<YogServer> = OnceLock::new();
/// Stable api table. Mods capture this pointer at registration (yog-api's
/// `installed_mods()` calls through it later, e.g. from UI render) — so it
/// must outlive `nativeInit`, not live on its stack. `ctx` inside points at
/// `HANDLERS`' own `RwLock`, published *before* any mod registers (unlike
/// the old design, where it pointed at a temporary pre-move `Box` that
/// went stale the instant `nativeInit` finished) — so it stays valid for a
/// hot-reload's later re-registration too.
pub(super) static API_TABLE: OnceLock<YogApi> = OnceLock::new();
/// All registered handlers + content. A `RwLock`, not a plain value: unlike
/// the original one-shot-then-immutable design, a hot-reload needs to
/// append fresh entries for a reloaded mod after this has long been
/// published. Read sites (`handlers()`) take a read guard; registration
/// (initial or reload) takes a write guard — both deref straight through to
/// `RuntimeHandlers`, so existing `handlers().field` call sites are
/// unaffected by this type change.
pub(super) static HANDLERS: OnceLock<RwLock<RuntimeHandlers>> = OnceLock::new();

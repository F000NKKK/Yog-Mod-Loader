//! The runtime's static globals: the JVM, the mods, the tables.

use crate::handlers::RuntimeHandlers;
use crate::handlers::RuntimeModuleRegistry;
use crate::intermod::build_api_table;
use jni::JavaVM;
use yog_abi::YogApi;
use yog_hot_reload::HotReloader;
use yog_hot_reload::LoadedModule;
use yog_sync::LazyLock;
use yog_sync::Mutex;
use yog_sync::OnceLock;
use yog_sync::RwLock;

// ── Static globals ────────────────────────────────────────────────────────────

/// Cached JVM handle for any-thread callbacks.
pub(super) static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();
/// Loaded mod libraries, keyed by mod id — owns the safe `dlopen`/`dlclose`
/// swap lifecycle (see `yog-hot-reload`) instead of a flat keep-alive `Vec`,
/// so a reload can find "the current generation of this mod" and safely
/// retire the previous one.
pub(super) static HOT_RELOADER: LazyLock<HotReloader<RuntimeModuleRegistry>> =
    LazyLock::new(|| HotReloader::new(RuntimeModuleRegistry));
/// The mod currently inside its own `yog_mod_register` call. Every `api_*`
/// registration function reads this to tag what it stores with a
/// `LoadedModule` handle, instead of requiring mods to pass a mod id
/// through the stable C ABI (an ABI break, and edits to every mod's
/// source). Valid because a mod's entire registration always happens
/// synchronously inside one `yog_mod_register` call — see `load_mod_lib`
/// and, in dev builds, `trigger_hot_reload`.
pub(super) static CURRENT_REGISTRATION: Mutex<Option<LoadedModule>> = Mutex::new(None);
/// What the loaded `.yog`s say about their mods.
pub(super) static MOD_INFOS: Mutex<Vec<yog_formats::manifest::Built>> = Mutex::new(Vec::new());
/// Stable api table. Mods capture this pointer at registration (yog-api's
/// `installed_mods()` calls through it later, e.g. from UI render) — so it
/// must outlive `nativeInit`, not live on its stack. `ctx` inside points at
/// `HANDLERS`' own `RwLock`, published *before* any mod registers, so it
/// stays valid for a hot-reload's later re-registration too.
pub(super) static API_TABLE: LazyLock<YogApi> = LazyLock::new(build_api_table);
/// All registered handlers + content. A `RwLock`, not a plain value: a
/// hot-reload appends a reloaded mod's entries after this has long been
/// published. Read sites (`handlers()`) take a read guard; registration
/// (initial or reload) takes a write guard — both deref straight through to
/// `RuntimeHandlers`.
pub(super) static HANDLERS: LazyLock<RwLock<RuntimeHandlers>> = LazyLock::new(Default::default);

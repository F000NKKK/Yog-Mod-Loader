//! Safe `dlopen`/`dlclose` swap primitives for reloading a mod's native
//! library without leaving a dangling reference into retired code anywhere.
//!
//! Deliberately has no idea what a "mod" or a "registry entry" *is* — that
//! knowledge belongs to the host (`yog-runtime`, eventually), which
//! implements [`ModuleRegistry`] to repoint/purge whatever it actually
//! stores (event handlers, item registrations, ...). This crate only owns
//! the generic, testable-in-isolation part: tracking in-flight calls into a
//! loaded library and deferring the actual `dlclose` until it's provably
//! safe.
//!
//! The reload flow, end to end:
//! 1. Load the new `.so` alongside the old one — both are valid at once.
//! 2. [`HotReloader::reload`] calls [`ModuleRegistry::retarget`] so the host
//!    repoints every callback it owns from the old generation to the new
//!    one. No *new* call can reach the old module after this point.
//! 3. The old module is marked retiring. If nothing is currently executing
//!    inside it (no open [`CallGuard`]), it is `dlclose`'d immediately. If
//!    something is mid-call, it's unloaded lazily — the moment the last
//!    guard drops — since its in-flight count can only drain from here, not
//!    refill.
//! 4. Right before the actual `dlclose`, [`ModuleRegistry::purge`] runs as a
//!    final safety net, dropping anything still tagged with the retired
//!    generation even though `retarget` should already have moved it all.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use libloading::Library;

/// One load of a given mod — every reload gets a new, strictly increasing
/// generation, even for the same mod id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleGeneration(u64);

/// Hands out strictly increasing [`ModuleGeneration`]s.
#[derive(Default)]
pub struct GenerationAllocator(AtomicU64);

impl GenerationAllocator {
    pub fn new() -> Self {
        Self(AtomicU64::new(1))
    }

    pub fn next(&self) -> ModuleGeneration {
        ModuleGeneration(self.0.fetch_add(1, Ordering::SeqCst))
    }
}

struct Inner {
    generation: ModuleGeneration,
    library: Mutex<Option<Library>>,
    active: AtomicUsize,
    retiring: AtomicBool,
    on_retire: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

/// Runs `on_retire` (if any) exactly once, then drops the library — the
/// actual `dlclose` — but only once retiring has been requested and nothing
/// is currently executing inside it. Safe to call redundantly: holding the
/// `library` lock across the active-count check serializes concurrent
/// callers (a `CallGuard` drop racing `LoadedModule::retire`) so only one
/// of them ever performs the unload.
fn try_unload(inner: &Inner) {
    if !inner.retiring.load(Ordering::SeqCst) {
        return;
    }
    let mut library = inner.library.lock().unwrap();
    if library.is_none() {
        return;
    }
    if inner.active.load(Ordering::SeqCst) != 0 {
        return;
    }
    if let Some(on_retire) = inner.on_retire.lock().unwrap().take() {
        on_retire();
    }
    *library = None;
}

/// A loaded native, tracking how many calls are currently executing inside
/// it so it can be unloaded the instant that's safe. Cheap to clone (an
/// `Arc` underneath) — every holder shares the same in-flight count and
/// retirement state.
#[derive(Clone)]
pub struct LoadedModule(Arc<Inner>);

impl LoadedModule {
    /// Loads a native library and tags it with `generation`.
    ///
    /// # Safety
    /// Inherits `libloading::Library::new`'s safety requirements: the
    /// native's load-time initializers (and later, its registered
    /// callbacks) must not violate Rust's invariants.
    pub unsafe fn load(path: &Path, generation: ModuleGeneration) -> Result<Self, libloading::Error> {
        let library = unsafe { Library::new(path) }?;
        Ok(LoadedModule(Arc::new(Inner {
            generation,
            library: Mutex::new(Some(library)),
            active: AtomicUsize::new(0),
            retiring: AtomicBool::new(false),
            on_retire: Mutex::new(None),
        })))
    }

    pub fn generation(&self) -> ModuleGeneration {
        self.0.generation
    }

    /// Whether the underlying native is still `dlopen`'d — `false` once it's
    /// been hard-unloaded (immediately or after a deferred retirement).
    pub fn is_loaded(&self) -> bool {
        self.0.library.lock().unwrap().is_some()
    }

    /// Whether this generation has been superseded by a newer one — `true`
    /// from the moment `retire` is called, even if the physical `dlclose`
    /// is still deferred (a call may be in flight). Callers dispatching a
    /// *new* call should check this first and skip modules for which it's
    /// `true`, rather than only relying on [`is_loaded`](Self::is_loaded):
    /// a retiring-but-still-loaded module is exactly the state where
    /// starting a fresh call would work today but is no longer correct
    /// policy — the whole point of retiring is to stop starting new calls.
    pub fn is_retiring(&self) -> bool {
        self.0.retiring.load(Ordering::SeqCst)
    }

    /// Looks up a symbol while the module is still loaded. Returns `None`
    /// once it has been unloaded, instead of the dangling-pointer crash
    /// calling into a `dlclose`'d library would otherwise cause.
    pub fn with_library<T>(&self, f: impl FnOnce(&Library) -> T) -> Option<T> {
        self.0.library.lock().unwrap().as_ref().map(f)
    }

    /// Marks the start of a call into this module's code. Returns `None` if
    /// the module has already been unloaded — callers must not proceed with
    /// the call in that case. Every JNI entry point that dispatches into a
    /// mod's registered callback should wrap that call in one of these (the
    /// integration point with `yog-runtime`'s existing `catch_unwind`-based
    /// `guard()` helper).
    pub fn enter(&self) -> Option<CallGuard> {
        if self.0.library.lock().unwrap().is_none() {
            return None;
        }
        self.0.active.fetch_add(1, Ordering::SeqCst);
        // Re-check: a retirement could have raced us between the loaded
        // check and the increment. `try_unload` is idempotent and safe to
        // invoke here too, so either this guard legitimately delays
        // unloading, or it's dropped by the caller immediately and unload
        // proceeds right after.
        Some(CallGuard(self.0.clone()))
    }

    /// Marks this module retiring: no new [`enter`](Self::enter) calls will
    /// find it loaded going forward from the host's perspective once its
    /// callbacks are retargeted, and it will be `dlclose`'d — running
    /// `on_hard_unload` immediately beforehand — the moment nothing is
    /// executing inside it (immediately, if that's already true).
    pub fn retire(&self, on_hard_unload: impl FnOnce() + Send + 'static) {
        *self.0.on_retire.lock().unwrap() = Some(Box::new(on_hard_unload));
        self.0.retiring.store(true, Ordering::SeqCst);
        try_unload(&self.0);
    }
}

/// RAII guard marking one in-flight call into a [`LoadedModule`]. Dropping
/// it may trigger that module's deferred hard-unload if it was retired
/// while this guard was outstanding and this was the last one.
pub struct CallGuard(Arc<Inner>);

impl Drop for CallGuard {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        try_unload(&self.0);
    }
}

/// The contract a host implements so [`HotReloader`] can repoint/purge
/// whatever it actually stores — event handlers, item/block registrations,
/// network channel handlers, and so on — none of which this crate knows
/// about.
pub trait ModuleRegistry {
    /// Repoint every stored callback tagged with `from` to its equivalent
    /// in `to` (the newly loaded generation of the same mod), or drop it if
    /// `to` no longer provides an equivalent.
    fn retarget(&self, mod_id: &str, from: ModuleGeneration, to: ModuleGeneration);

    /// Drop every remaining entry tagged with `generation`. Called once,
    /// right before that generation's library is actually `dlclose`'d, as a
    /// final safety net even though `retarget` should already have moved
    /// everything off of it.
    fn purge(&self, generation: ModuleGeneration);
}

/// Orchestrates the load/reload lifecycle for a set of mods, keyed by mod
/// id, against a host-supplied [`ModuleRegistry`].
pub struct HotReloader<R: ModuleRegistry> {
    registry: Arc<R>,
    generations: GenerationAllocator,
    loaded: Mutex<HashMap<String, LoadedModule>>,
}

impl<R: ModuleRegistry + Send + Sync + 'static> HotReloader<R> {
    pub fn new(registry: R) -> Self {
        HotReloader { registry: Arc::new(registry), generations: GenerationAllocator::new(), loaded: Mutex::new(HashMap::new()) }
    }

    /// The first load of a mod — nothing to retarget/retire yet.
    ///
    /// # Safety
    /// See [`LoadedModule::load`].
    pub unsafe fn load_initial(&self, mod_id: impl Into<String>, path: &Path) -> Result<LoadedModule, libloading::Error> {
        let generation = self.generations.next();
        let module = unsafe { LoadedModule::load(path, generation)? };
        self.loaded.lock().unwrap().insert(mod_id.into(), module.clone());
        Ok(module)
    }

    /// Loads `new_path` as the next generation of `mod_id`, retargets the
    /// registry onto it, and retires the previous generation (hard-unloaded
    /// immediately or deferred, per its in-flight call count).
    ///
    /// # Safety
    /// See [`LoadedModule::load`].
    pub unsafe fn reload(&self, mod_id: &str, new_path: &Path) -> Result<LoadedModule, libloading::Error> {
        let new_generation = self.generations.next();
        let new_module = unsafe { LoadedModule::load(new_path, new_generation)? };

        let old_module = {
            let mut loaded = self.loaded.lock().unwrap();
            let old = loaded.get(mod_id).cloned();
            loaded.insert(mod_id.to_string(), new_module.clone());
            old
        };

        if let Some(old_module) = old_module {
            let old_generation = old_module.generation();
            self.registry.retarget(mod_id, old_generation, new_generation);
            let registry = self.registry.clone();
            old_module.retire(move || registry.purge(old_generation));
        }

        Ok(new_module)
    }

    /// The currently-active module for `mod_id`, if any.
    pub fn active(&self, mod_id: &str) -> Option<LoadedModule> {
        self.loaded.lock().unwrap().get(mod_id).cloned()
    }

    /// Every currently-active module, keyed by mod id.
    pub fn all_active(&self) -> Vec<(String, LoadedModule)> {
        self.loaded.lock().unwrap().iter().map(|(id, module)| (id.clone(), module.clone())).collect()
    }

    /// The host's [`ModuleRegistry`] implementation.
    pub fn registry(&self) -> &R {
        &self.registry
    }
}

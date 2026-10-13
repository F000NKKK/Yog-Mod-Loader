//! Registering a screen with the game, handles to it, the command context
//! and navigation between screens.

use std::any::{Any, TypeId};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::host::Host;
use crate::{ScreenMeta, View, ViewModel};

type Job<V> = Box<dyn FnOnce(&mut V) + Send>;
type Reader<V> = Box<dyn Fn(&mut dyn FnMut(&V)) -> bool + Send + Sync>;

fn lock<T: ?Sized>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) struct Shared<V> {
    meta: ScreenMeta,
    queue: Mutex<Vec<Job<V>>>,
    reader: OnceLock<Reader<V>>,
}

/// A cloneable, thread-safe handle to a registered screen's view model.
pub struct ScreenHandle<V> {
    shared: Arc<Shared<V>>,
}

impl<V> Clone for ScreenHandle<V> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl<V: ViewModel> ScreenHandle<V> {
    pub(crate) fn new(meta: ScreenMeta) -> Self {
        Self {
            shared: Arc::new(Shared {
                meta,
                queue: Mutex::new(Vec::new()),
                reader: OnceLock::new(),
            }),
        }
    }

    /// Changes the view model from any thread (a network handler, a
    /// command, ...). Queued: runs on the render thread before the next
    /// frame or event, then the bindings update. Never blocks.
    pub fn update(&self, f: impl FnOnce(&mut V) + Send + 'static) {
        lock(&self.shared.queue).push(Box::new(f));
    }

    /// Reads the view model — `None` while the screen is busy (rendering or
    /// handling input, e.g. when called from one of its own commands).
    pub fn read<R>(&self, f: impl FnOnce(&V) -> R) -> Option<R> {
        let reader = self.shared.reader.get()?;
        let mut f = Some(f);
        let mut out = None;
        reader(&mut |vm| {
            if let Some(f) = f.take() {
                out = Some(f(vm));
            }
        });
        out
    }

    /// The screen's UI id.
    pub fn id(&self) -> &'static str {
        self.shared.meta.id
    }

    /// Opens the screen.
    pub fn open(&self) {
        let m = self.shared.meta;
        yog_api::open_ui(m.id, m.modal, m.pause);
    }

    /// Closes the screen if it's open.
    pub fn close(&self) {
        yog_api::close_ui(self.shared.meta.id);
    }

    pub(crate) fn take_queue(&self) -> Vec<Job<V>> {
        std::mem::take(&mut *lock(&self.shared.queue))
    }

    pub(crate) fn set_reader(&self, reader: Reader<V>) {
        let _ = self.shared.reader.set(reader);
    }
}

pub(crate) enum Request {
    Close,
    Navigate(String),
    Back,
    Focus(String),
    Invalidate,
}

/// What a command or view-model hook can ask of its screen. Requests run
/// right after the call returns.
pub struct Cx<'a, V> {
    reqs: &'a mut Vec<Request>,
    handle: &'a ScreenHandle<V>,
}

impl<'a, V: ViewModel> Cx<'a, V> {
    pub(crate) fn new(reqs: &'a mut Vec<Request>, handle: &'a ScreenHandle<V>) -> Self {
        Self { reqs, handle }
    }

    /// Closes this screen.
    pub fn close(&mut self) {
        self.reqs.push(Request::Close);
    }

    /// Opens the screen `ui_id`; Esc there comes back here.
    pub fn navigate(&mut self, ui_id: &str) {
        self.reqs.push(Request::Navigate(ui_id.to_owned()));
    }

    /// Opens the yog-kit screen `W` of this mod after running `init` on its
    /// view model; Esc there comes back here.
    pub fn navigate_to<W: View>(&mut self, init: impl FnOnce(&mut W::Vm) + Send + 'static) {
        if let Some(h) = handle_of::<W>() {
            h.update(init);
        }
        self.navigate(W::META.id);
    }

    /// Back to the screen this one was navigated from, else closes it.
    pub fn back(&mut self) {
        self.reqs.push(Request::Back);
    }

    /// Moves the keyboard focus to the widget with `id`.
    pub fn focus(&mut self, id: &str) {
        self.reqs.push(Request::Focus(id.to_owned()));
    }

    /// A handle to this screen (keep it for updates from other threads).
    pub fn handle(&self) -> ScreenHandle<V> {
        self.handle.clone()
    }

    /// Re-evaluates the bindings (they already are after every command).
    pub fn invalidate(&mut self) {
        self.reqs.push(Request::Invalidate);
    }
}

// ── navigation (per mod: each mod links its own yog-kit) ─────────────────────

static NAV: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
/// A navigation is closing the current screen — keep the stack.
static NAVIGATING: AtomicBool = AtomicBool::new(false);
static SCREENS: Mutex<Vec<ScreenMeta>> = Mutex::new(Vec::new());
#[allow(clippy::type_complexity)]
static HANDLES: Mutex<Vec<(TypeId, Box<dyn Any + Send>)>> = Mutex::new(Vec::new());

fn open_id(id: &str) {
    let meta = lock(&SCREENS).iter().find(|m| m.id == id).copied();
    let (modal, pause) = meta.map_or((true, false), |m| (m.modal, m.pause));
    yog_api::open_ui(id, modal, pause);
}

pub(crate) fn navigate(from: &'static str, to: &str) {
    lock(&NAV).push(from);
    NAVIGATING.store(true, Ordering::SeqCst);
    open_id(to);
}

/// Opens the previous screen; `false` when there's none.
pub(crate) fn go_back() -> bool {
    let Some(prev) = lock(&NAV).pop() else {
        return false;
    };
    NAVIGATING.store(true, Ordering::SeqCst);
    open_id(prev);
    true
}

/// A screen closed: unless it's a navigation, the history is over.
pub(crate) fn screen_closed() {
    if !NAVIGATING.swap(false, Ordering::SeqCst) {
        lock(&NAV).clear();
    }
}

/// The handle of the registered screen `W`.
pub fn handle_of<W: View>() -> Option<ScreenHandle<W::Vm>> {
    lock(&HANDLES)
        .iter()
        .find(|(t, _)| *t == TypeId::of::<W>())
        .and_then(|(_, h)| h.downcast_ref::<ScreenHandle<W::Vm>>().cloned())
}

/// Hosts the compiled view `W` as a Yog screen.
pub struct Screen<W: View>(PhantomData<W>);

impl<W: View> Screen<W> {
    /// Registers `W` (rendering + input) with the view model `vm`; called
    /// by the generated `register()`.
    pub fn register(registry: &mut yog_api::Registry, vm: W::Vm) -> ScreenHandle<W::Vm> {
        let handle = ScreenHandle::<W::Vm>::new(W::META);
        let host = Arc::new(Mutex::new(Host::<W>::new(vm, handle.clone(), true)));
        let weak = Arc::downgrade(&host);
        handle.set_reader(Box::new(move |f| {
            let Some(host) = weak.upgrade() else {
                return false;
            };
            let Ok(h) = host.try_lock() else {
                return false;
            };
            f(&h.vm);
            true
        }));
        lock(&SCREENS).push(W::META);
        lock(&HANDLES).push((TypeId::of::<W>(), Box::new(handle.clone())));

        let render = host.clone();
        registry.on_ui_render(W::META.id, move |gfx| {
            let mut h = lock(&render);
            let (w, hh) = gfx.screen_size();
            h.frame(w as f32, hh as f32);
            h.ui.render(gfx);
        });
        registry.register_ui(W::META.id, move |_, ev| lock(&host).on_event(ev));
        handle
    }
}

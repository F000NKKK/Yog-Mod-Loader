//! The bus's router: addresses by name, who provides each, who listens to
//! each event — and running a call or an event through them.

use std::collections::HashMap;
use std::os::raw::c_void;
use yog_sync::{Arc, RwLock};

use yog_abi::{YogAddress, YogBytes, YogHandlerFn, YogReply, YogStatus, status};
use yog_hot_reload::{CallGuard, LoadedModule, ModuleGeneration};

/// Who handles a message.
#[derive(Clone)]
pub(crate) enum Handler {
    /// Native code: a mod's (`owner`, dropped with it) or the runtime's own.
    Native {
        owner: Option<LoadedModule>,
        ud: usize,
        f: YogHandlerFn,
    },
    /// A handler in the Java host's table (`YogBus.java`), by its id.
    Java(i32),
}

/// An event's subscriber.
#[derive(Clone)]
struct Subscriber {
    priority: i32,
    handler: Handler,
}

/// Runs a Java handler: its status, its reply written to `reply`.
pub(crate) type JavaRunner = fn(handler: i32, payload: &[u8], reply: &YogReply) -> YogStatus;

/// Addresses, providers and subscribers. The tables are copied when they
/// change (registration) and shared when they're read (every call): a call
/// takes a snapshot without copying a handler or a list.
#[derive(Default)]
pub(crate) struct Router {
    providers: RwLock<Arc<HashMap<YogAddress, Handler>>>,
    subscribers: RwLock<HashMap<YogAddress, Arc<Vec<Subscriber>>>>,
}

impl Router {
    /// `name`'s address, made on first sight: its atom (`yog_event::resolve`),
    /// the number every binary of the process gives it.
    pub(crate) fn resolve(&self, name: &str) -> YogAddress {
        yog_event::resolve(name)
    }

    /// Makes `handler` the provider of `address`; `false` if there is one.
    pub(crate) fn provide(&self, address: YogAddress, handler: Handler) -> bool {
        let mut providers = self.providers.write();
        if providers.contains_key(&address) {
            return false;
        }
        Arc::make_mut(&mut providers).insert(address, handler);
        true
    }

    pub(crate) fn provided(&self, address: YogAddress) -> bool {
        self.providers.read().contains_key(&address)
    }

    /// Subscribes `handler` to `event`, after those of its priority or higher.
    pub(crate) fn on(&self, event: YogAddress, priority: i32, handler: Handler) {
        let mut subs = self.subscribers.write();
        let list = Arc::make_mut(subs.entry(event).or_default());
        let at = list
            .iter()
            .position(|s| s.priority < priority)
            .unwrap_or(list.len());
        list.insert(at, Subscriber { priority, handler });
    }

    /// Whether anyone listens to `event` (an emitter can skip building its payload).
    pub(crate) fn heard(&self, event: YogAddress) -> bool {
        self.subscribers
            .read()
            .get(&event)
            .is_some_and(|list| !list.is_empty())
    }

    /// The keys someone listens to under keyed event `address` (the
    /// channels of `…/net/packet/<channel>`), sorted.
    pub(crate) fn listened_keys(&self, address: &str) -> Vec<String> {
        let branch = yog_event::keyed(address, "");
        let mut keys: Vec<String> = self
            .subscribers
            .read()
            .iter()
            .filter(|(_, list)| !list.is_empty())
            .filter_map(|(at, _)| {
                yog_data::atom::local::name(*at)?
                    .strip_prefix(branch.as_str())
                    .map(str::to_owned)
            })
            .collect();
        keys.sort();
        keys
    }

    /// Calls `to`'s provider: its status, its reply written to `reply`.
    pub(crate) fn call(
        &self,
        to: YogAddress,
        payload: &[u8],
        reply: &YogReply,
        java: JavaRunner,
    ) -> YogStatus {
        let providers = Arc::clone(&self.providers.read());
        match providers.get(&to) {
            Some(handler) => run(handler, payload, reply, java),
            None => status::NO_HANDLER,
        }
    }

    /// Runs `event`'s subscribers in order, each seeing the last one's reply
    /// (when it wrote one) as its payload; stops at a cancel. The event as
    /// the handlers left it goes to `reply` — nothing when none changed it.
    pub(crate) fn emit(
        &self,
        event: YogAddress,
        payload: &[u8],
        reply: &YogReply,
        java: JavaRunner,
    ) -> YogStatus {
        let Some(subs) = self.subscribers.read().get(&event).cloned() else {
            return status::OK;
        };
        // The event as it stands and the next handler's reply: two buffers
        // traded, never made again.
        let (mut current, mut next) = (Vec::new(), Vec::new());
        let mut changed = false;
        let mut st = status::OK;
        // A mod's handlers one after another are entered once.
        let mut entered: Option<(ModuleGeneration, CallGuard<'_>)> = None;
        for s in subs.iter() {
            let payload_now = if changed { current.as_slice() } else { payload };
            next.clear();
            let out = YogReply::to_vec(&mut next);
            st = match &s.handler {
                Handler::Java(id) => java(*id, payload_now, &out),
                Handler::Native { owner: None, ud, f } => unsafe {
                    f(*ud as *mut c_void, YogBytes::new(payload_now), &out)
                },
                Handler::Native {
                    owner: Some(m),
                    ud,
                    f,
                } => {
                    if entered.as_ref().is_none_or(|(g, _)| *g != m.generation()) {
                        entered = None;
                        match m.enter() {
                            Some(guard) => entered = Some((m.generation(), guard)),
                            // Gone: as if it weren't there.
                            None => continue,
                        }
                    }
                    let _running = super::running::enter(m.generation());
                    unsafe { f(*ud as *mut c_void, YogBytes::new(payload_now), &out) }
                }
            };
            if !next.is_empty() {
                std::mem::swap(&mut current, &mut next);
                changed = true;
            }
            if st == status::CANCELLED {
                break;
            }
        }
        drop(entered);
        if changed {
            unsafe { YogReply::send(reply, &current) };
        }
        if st == status::CANCELLED {
            st
        } else {
            status::OK
        }
    }

    /// [`Router::call`] with the reply as a `Vec` (the runtime's own calls, tests).
    pub(crate) fn call_vec(
        &self,
        to: YogAddress,
        payload: &[u8],
        java: JavaRunner,
    ) -> (YogStatus, Vec<u8>) {
        let mut out = Vec::new();
        let st = self.call(to, payload, &YogReply::to_vec(&mut out), java);
        (st, out)
    }

    /// [`Router::emit`] with the reply as a `Vec`: the event as the
    /// handlers left it, the payload itself when none changed it.
    #[cfg(test)]
    pub(crate) fn emit_vec(
        &self,
        event: YogAddress,
        payload: &[u8],
        java: JavaRunner,
    ) -> (YogStatus, Vec<u8>) {
        let mut out = Vec::new();
        let st = self.emit(event, payload, &YogReply::to_vec(&mut out), java);
        if out.is_empty() {
            out = payload.to_vec();
        }
        (st, out)
    }

    /// Drops the Java host's handlers `ids` (a retired generation of a mod's
    /// classes, `YogBus.retire`): as providers and as listeners.
    pub(crate) fn drop_java(&self, ids: &[i32]) {
        let gone = |h: &Handler| matches!(h, Handler::Java(id) if ids.contains(id));
        Arc::make_mut(&mut self.providers.write()).retain(|_, h| !gone(h));
        for list in self.subscribers.write().values_mut() {
            Arc::make_mut(list).retain(|s| !gone(&s.handler));
        }
    }

    /// Drops what the module of `generation` registered.
    pub(crate) fn purge(&self, generation: ModuleGeneration) {
        let owned_by = |h: &Handler| matches!(h, Handler::Native { owner: Some(m), .. } if m.generation() == generation);
        Arc::make_mut(&mut self.providers.write()).retain(|_, h| !owned_by(h));
        for list in self.subscribers.write().values_mut() {
            Arc::make_mut(list).retain(|s| !owned_by(&s.handler));
        }
    }
}

/// A handler's status for `payload`, its reply written to `reply`.
fn run(handler: &Handler, payload: &[u8], reply: &YogReply, java: JavaRunner) -> YogStatus {
    match handler {
        Handler::Java(id) => java(*id, payload, reply),
        Handler::Native { owner, ud, f } => {
            // A mod's code runs only while its library is loaded.
            let _guard = match owner {
                Some(m) => match m.enter() {
                    Some(g) => Some((g, super::running::enter(m.generation()))),
                    None => return status::NO_HANDLER,
                },
                None => None,
            };
            unsafe { f(*ud as *mut c_void, YogBytes::new(payload), reply) }
        }
    }
}

//! The bus's router: addresses by name, who provides each, who listens to
//! each event — and running a call or an event through them.

use std::collections::HashMap;
use std::os::raw::c_void;
use std::sync::RwLock;

use yog_abi::{YogAddress, YogBytes, YogHandlerFn, YogReply, YogStatus, status};
use yog_hot_reload::{LoadedModule, ModuleGeneration};
use yog_sync::ReadWrite;

/// Who handles a message.
#[derive(Clone)]
pub(crate) enum Handler {
    /// Native code: a mod's (`owner`, dropped with it) or the runtime's own.
    Native { owner: Option<LoadedModule>, ud: usize, f: YogHandlerFn },
    /// A handler in the Java host's table (`YogBus.java`), by its id.
    Java(i32),
}

/// An event's subscriber.
#[derive(Clone)]
struct Subscriber {
    priority: i32,
    handler: Handler,
}

#[derive(Default)]
struct Names {
    by_name: HashMap<String, YogAddress>,
    names: Vec<String>,
}

/// Runs a Java handler: its status and reply.
pub(crate) type JavaRunner = fn(handler: i32, payload: &[u8]) -> (YogStatus, Vec<u8>);

/// Addresses, providers and subscribers.
#[derive(Default)]
pub(crate) struct Router {
    names: RwLock<Names>,
    providers: RwLock<HashMap<YogAddress, Handler>>,
    subscribers: RwLock<HashMap<YogAddress, Vec<Subscriber>>>,
}

impl Router {
    /// `name`'s address, made on first sight.
    pub(crate) fn resolve(&self, name: &str) -> YogAddress {
        if name.is_empty() {
            return yog_abi::NO_ADDRESS;
        }
        if let Some(&a) = self.names.read_locked().by_name.get(name) {
            return a;
        }
        let mut names = self.names.write_locked();
        if let Some(&a) = names.by_name.get(name) {
            return a;
        }
        let a = names.names.len() as YogAddress;
        names.names.push(name.to_owned());
        names.by_name.insert(name.to_owned(), a);
        a
    }

    /// Makes `handler` the provider of `address`; `false` if there is one.
    pub(crate) fn provide(&self, address: YogAddress, handler: Handler) -> bool {
        let mut providers = self.providers.write_locked();
        if providers.contains_key(&address) {
            return false;
        }
        providers.insert(address, handler);
        true
    }

    pub(crate) fn provided(&self, address: YogAddress) -> bool {
        self.providers.read_locked().contains_key(&address)
    }

    /// Subscribes `handler` to `event`, after those of its priority or higher.
    pub(crate) fn on(&self, event: YogAddress, priority: i32, handler: Handler) {
        let mut subs = self.subscribers.write_locked();
        let list = subs.entry(event).or_default();
        let at = list.iter().position(|s| s.priority < priority).unwrap_or(list.len());
        list.insert(at, Subscriber { priority, handler });
    }

    /// Whether anyone listens to `event` (an emitter can skip building its payload).
    pub(crate) fn heard(&self, event: YogAddress) -> bool {
        self.subscribers.read_locked().get(&event).is_some_and(|list| !list.is_empty())
    }

    /// The keys someone listens to under keyed event `address` (the
    /// channels of `…/net/packet/<channel>`), sorted.
    pub(crate) fn listened_keys(&self, address: &str) -> Vec<String> {
        let branch = yog_event::keyed(address, "");
        let names = self.names.read_locked();
        let mut keys: Vec<String> = self
            .subscribers
            .read_locked()
            .iter()
            .filter(|(_, list)| !list.is_empty())
            .filter_map(|(at, _)| names.names.get(*at as usize)?.strip_prefix(branch.as_str()).map(str::to_owned))
            .collect();
        keys.sort();
        keys
    }

    /// Calls `to`'s provider.
    pub(crate) fn call(&self, to: YogAddress, payload: &[u8], java: JavaRunner) -> (YogStatus, Vec<u8>) {
        let Some(handler) = self.providers.read_locked().get(&to).cloned() else {
            return (status::NO_HANDLER, Vec::new());
        };
        run(&handler, payload, java)
    }

    /// Runs `event`'s subscribers in order, each seeing the last one's reply
    /// (when it wrote one) as its payload; stops at a cancel.
    pub(crate) fn emit(&self, event: YogAddress, payload: &[u8], java: JavaRunner) -> (YogStatus, Vec<u8>) {
        let subs = self.subscribers.read_locked().get(&event).cloned().unwrap_or_default();
        let mut current = payload.to_vec();
        for s in &subs {
            let (st, reply) = run(&s.handler, &current, java);
            if !reply.is_empty() {
                current = reply;
            }
            if st == status::CANCELLED {
                return (st, current);
            }
        }
        (status::OK, current)
    }

    /// Drops what the module of `generation` registered.
    pub(crate) fn purge(&self, generation: ModuleGeneration) {
        let owned_by = |h: &Handler| matches!(h, Handler::Native { owner: Some(m), .. } if m.generation() == generation);
        self.providers.write_locked().retain(|_, h| !owned_by(h));
        for list in self.subscribers.write_locked().values_mut() {
            list.retain(|s| !owned_by(&s.handler));
        }
    }
}

/// A handler's status and reply for `payload`.
fn run(handler: &Handler, payload: &[u8], java: JavaRunner) -> (YogStatus, Vec<u8>) {
    match handler {
        Handler::Java(id) => java(*id, payload),
        Handler::Native { owner, ud, f } => {
            // A mod's code runs only while its library is loaded.
            let _guard = match owner {
                Some(m) => match m.enter() {
                    Some(g) => Some(g),
                    None => return (status::NO_HANDLER, Vec::new()),
                },
                None => None,
            };
            let mut out = Vec::new();
            let reply = YogReply::to_vec(&mut out);
            let st = unsafe { f(*ud as *mut c_void, YogBytes::new(payload), &reply) };
            (st, out)
        }
    }
}

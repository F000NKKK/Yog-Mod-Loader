//! Injections (`#[inject]`, `docs/design/nanites.md`): what a mod hooks,
//! known before the game's classes load — the runtime asks the mod's
//! library for them (`yog_mod_nanites`) — and its handlers, subscribed when
//! the mod registers to the point's keyed event.

use yog_event::nanite::{Inject, Injected, Nanites, point};
use yog_sync::Mutex;

use super::{Java, Member, Nanite};
use crate::Registry;

/// A declared injection: its member, where in it, its handler.
pub struct InjectEntry {
    pub member: &'static Member,
    /// `yog_ids::mixin_at`.
    pub at: &'static str,
    /// What an `INVOKE`/`FIELD`/`NEW` at names, else empty.
    pub target: &'static str,
    pub ordinal: i32,
    /// `yog_ids::mixin_shift`.
    pub shift: &'static str,
    pub cancellable: bool,
    /// Among the point's handlers (higher first).
    pub priority: i32,
    pub handler: fn(&mut Hook),
}

impl InjectEntry {
    /// What the host makes of it.
    pub fn spec(&self) -> Inject {
        let (method, desc) = self.member.signature();
        let class = self.member.class();
        Inject {
            point: point(class, method, &desc, self.at, self.target),
            class: class.to_owned(),
            method: method.to_owned(),
            desc,
            at: self.at.to_owned(),
            target: self.target.to_owned(),
            ordinal: self.ordinal,
            shift: self.shift.to_owned(),
            cancellable: self.cancellable,
        }
    }
}

/// This mod's injections (each `#[inject]`'s `ctor` adds its own).
#[doc(hidden)]
pub fn __yog_nanite_registry() -> &'static Mutex<Vec<InjectEntry>> {
    static REG: Mutex<Vec<InjectEntry>> = Mutex::new(Vec::new());
    &REG
}

/// Every injection of this mod, as the runtime asks before the game loads.
#[doc(hidden)]
pub fn __yog_nanites() -> Vec<u8> {
    let injects = __yog_nanite_registry()
        .lock()
        .iter()
        .map(InjectEntry::spec)
        .collect();
    yog_data::to_bytes(&Nanites { injects })
}

/// A handler to subscribe: its point, priority, function.
type Subscription = (String, i32, fn(&mut Hook));

/// Each injection's handler on its point (when the mod registers).
pub fn subscribe(registry: &mut Registry) {
    let entries: Vec<Subscription> = __yog_nanite_registry()
        .lock()
        .iter()
        .map(|e| (e.spec().point, e.priority, e.handler))
        .collect();
    for (key, priority, handler) in entries {
        registry.change_key_at::<Injected>(&key, priority, move |e, _| handler(&mut Hook(e)));
    }
}

/// An injection point reached, as a handler sees it.
pub struct Hook<'a>(&'a mut Injected);

impl<'a> Hook<'a> {
    /// The point reached, to be changed.
    pub fn new(e: &'a mut Injected) -> Self {
        Hook(e)
    }

    /// The object the method runs on (none in a static one).
    pub fn this<T: Nanite>(&self) -> Option<T> {
        T::from_value(self.0.this.clone())
    }

    /// Argument `i`.
    pub fn arg<T: Java>(&self, i: usize) -> Option<T> {
        T::from_value(self.0.args.get(i)?.clone())
    }

    /// How many arguments there are.
    pub fn args(&self) -> usize {
        self.0.args.len()
    }

    /// The value returned, at a `RETURN`.
    pub fn value<T: Java>(&self) -> Option<T> {
        T::from_value(self.0.value.clone())
    }

    /// Stops the method (a cancellable injection's).
    pub fn cancel(&mut self) {
        self.0.cancelled = true;
    }

    /// Stops the method returning `value`.
    pub fn set_return<T: Java>(&mut self, value: T) {
        self.0.value = value.to_value();
        self.0.cancelled = true;
        self.0.returned = true;
    }

    /// Whether a handler before this one stopped it.
    pub fn cancelled(&self) -> bool {
        self.0.cancelled
    }
}

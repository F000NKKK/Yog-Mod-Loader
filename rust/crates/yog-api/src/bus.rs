//! Events through the bus: one subscription for every event type, and the
//! one trampoline that hands its payload to a mod's handler.

use std::marker::PhantomData;
use std::os::raw::c_void;
use std::sync::Arc;

use yog_abi::{YogBytes, YogHandlerFn, YogReply, YogStatus, YogStr, priority, status};
use yog_event::{Event, EventPhase, keyed};

use crate::registry::{Registry, guard};
use crate::server::CServer;
use yog_core::Server;

/// What a handler decides about an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// Let it happen; the next handler sees it.
    Continue,
    /// Stop it (before it happens): the game doesn't do it, later handlers don't see it.
    Cancel,
}

/// What a handler may return: nothing (`Continue`), whether to let it
/// happen (`false` cancels), or a [`Flow`].
pub trait IntoFlow {
    fn into_flow(self) -> Flow;
}

impl IntoFlow for () {
    fn into_flow(self) -> Flow {
        Flow::Continue
    }
}

impl IntoFlow for bool {
    fn into_flow(self) -> Flow {
        if self { Flow::Continue } else { Flow::Cancel }
    }
}

impl IntoFlow for Flow {
    fn into_flow(self) -> Flow {
        self
    }
}

/// What a handler is handed besides the event: when it runs, and the server.
pub struct EventCx {
    phase: EventPhase,
    server: CServer,
}

impl EventCx {
    /// Before the game does it (`Pre`, can cancel) or after (`Post`).
    pub fn phase(&self) -> EventPhase {
        self.phase
    }

    pub fn is_pre(&self) -> bool {
        self.phase == EventPhase::Pre
    }

    /// The server (actions and queries on the world).
    pub fn server(&self) -> &dyn Server {
        &self.server
    }
}

/// What an event subscription keeps: the handler (shared by its pre and post
/// subscriptions), and which phase this one is.
pub(crate) struct Subscription<E, F> {
    handler: Arc<F>,
    phase: EventPhase,
    event: PhantomData<fn(&E)>,
}


/// The bus's handler for subscription `ud`: the payload read as `E`, the
/// mod's handler run; a cancel before the event stops it.
unsafe extern "C" fn on_event<E: Event, F, R>(ud: *mut c_void, payload: YogBytes, _reply: *const YogReply) -> YogStatus
where
    F: Fn(&E, &EventCx) -> R + Send + Sync,
    R: IntoFlow,
{
    let s = unsafe { &*(ud as *const Subscription<E, F>) };
    let Some(event) = E::from_bytes(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    let flow = guard(Flow::Continue, || (s.handler)(&event, &s.cx()).into_flow());
    if s.phase == EventPhase::Pre && flow == Flow::Cancel { status::CANCELLED } else { status::OK }
}

/// The bus's handler for changing subscription `ud`: the mod's handler
/// changes the event, and the changed event is the reply the next handler
/// (and the game) sees.
unsafe extern "C" fn on_change<E: Event, F>(ud: *mut c_void, payload: YogBytes, reply: *const YogReply) -> YogStatus
where
    F: Fn(&mut E, &EventCx) + Send + Sync,
{
    let s = unsafe { &*(ud as *const Subscription<E, F>) };
    let Some(mut event) = E::from_bytes(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    guard((), || (s.handler)(&mut event, &s.cx()));
    unsafe { YogReply::send(reply, &event.to_bytes()) };
    status::OK
}

impl<E, F> Subscription<E, F> {
    pub(crate) fn new(handler: Arc<F>, phase: EventPhase) -> Self {
        Self { handler, phase, event: PhantomData }
    }

    pub(crate) fn handler(&self) -> &F {
        &self.handler
    }

    pub(crate) fn cx(&self) -> EventCx {
        EventCx { phase: self.phase, server: CServer }
    }
}

impl Registry {
    /// Runs `handler` on every `E`, before the game does it (if `E` can be
    /// stopped — [`Flow::Cancel`] / `false` stops it) and after.
    pub fn on<E: Event + 'static, R: IntoFlow + 'static>(&mut self, handler: impl Fn(&E, &EventCx) -> R + Send + Sync + 'static) {
        self.both::<E, _, R>(E::PRE.map(str::to_owned), E::ADDRESS.to_owned(), handler);
    }

    /// [`Registry::on`] for a keyed event's `key` only (`E::KEY`): the ticks
    /// of one entity type, the presses of one key binding.
    pub fn on_key<E: Event + 'static, R: IntoFlow + 'static>(&mut self, key: &str, handler: impl Fn(&E, &EventCx) -> R + Send + Sync + 'static) {
        self.both::<E, _, R>(E::PRE.map(|p| keyed(p, key)), keyed(E::ADDRESS, key), handler);
    }

    /// Runs `handler` before the game does `E` — it can cancel it. Nothing
    /// for an event that can't be stopped.
    pub fn before<E: Event + 'static, R: IntoFlow + 'static>(&mut self, handler: impl Fn(&E, &EventCx) -> R + Send + Sync + 'static) {
        if let Some(pre) = E::PRE {
            self.at::<E, _, R>(pre, EventPhase::Pre, Arc::new(handler));
        }
    }

    /// [`Registry::before`] for a keyed event's `key` only.
    pub fn before_key<E: Event + 'static, R: IntoFlow + 'static>(&mut self, key: &str, handler: impl Fn(&E, &EventCx) -> R + Send + Sync + 'static) {
        if let Some(pre) = E::PRE {
            self.at::<E, _, R>(&keyed(pre, key), EventPhase::Pre, Arc::new(handler));
        }
    }

    /// Runs `handler` after the game did `E`.
    pub fn after<E: Event + 'static, R: IntoFlow + 'static>(&mut self, handler: impl Fn(&E, &EventCx) -> R + Send + Sync + 'static) {
        self.at::<E, _, R>(E::ADDRESS, EventPhase::Post, Arc::new(handler));
    }

    /// [`Registry::after`] at `priority` among `E`'s handlers (higher first).
    pub(crate) fn after_at<E: Event + 'static, R: IntoFlow + 'static, F>(&self, priority: i32, handler: F)
    where
        F: Fn(&E, &EventCx) -> R + Send + Sync + 'static,
    {
        self.subscribe_at::<E, F>(E::ADDRESS, priority, EventPhase::Post, Arc::new(handler), on_event::<E, F, R>);
    }

    /// Lets `handler` change every `E` before it's used (a loot table's
    /// items); each handler sees the one before's result.
    pub fn change<E: Event + 'static>(&mut self, handler: impl Fn(&mut E, &EventCx) + Send + Sync + 'static) {
        self.changing::<E, _>(E::ADDRESS, priority::DEFAULT, Arc::new(handler));
    }

    /// [`Registry::change`] for a keyed event's `key` only, at `priority`
    /// among its handlers (higher first).
    pub(crate) fn change_key_at<E: Event + 'static>(&mut self, key: &str, priority: i32, handler: impl Fn(&mut E, &EventCx) + Send + Sync + 'static) {
        self.changing::<E, _>(&keyed(E::ADDRESS, key), priority, Arc::new(handler));
    }

    /// `handler` at `pre` (if any) and `post`.
    fn both<E: Event + 'static, F, R>(&mut self, pre: Option<String>, post: String, handler: F)
    where
        F: Fn(&E, &EventCx) -> R + Send + Sync + 'static,
        R: IntoFlow + 'static,
    {
        let handler = Arc::new(handler);
        if let Some(pre) = pre {
            self.at::<E, F, R>(&pre, EventPhase::Pre, Arc::clone(&handler));
        }
        self.at::<E, F, R>(&post, EventPhase::Post, handler);
    }

    /// `handler` on `E` at `address`, in `phase`.
    fn at<E: Event + 'static, F, R>(&mut self, address: &str, phase: EventPhase, handler: Arc<F>)
    where
        F: Fn(&E, &EventCx) -> R + Send + Sync + 'static,
        R: IntoFlow + 'static,
    {
        self.subscribe::<E, F>(address, phase, handler, on_event::<E, F, R>);
    }

    fn changing<E: Event + 'static, F>(&mut self, address: &str, priority: i32, handler: Arc<F>)
    where
        F: Fn(&mut E, &EventCx) + Send + Sync + 'static,
    {
        self.subscribe_at::<E, F>(address, priority, EventPhase::Post, handler, on_change::<E, F>);
    }

    fn subscribe<E: 'static, F: Send + Sync + 'static>(&mut self, address: &str, phase: EventPhase, handler: Arc<F>, run: YogHandlerFn) {
        self.subscribe_at::<E, F>(address, priority::DEFAULT, phase, handler, run);
    }

    fn subscribe_at<E: 'static, F: Send + Sync + 'static>(&self, address: &str, priority: i32, phase: EventPhase, handler: Arc<F>, run: YogHandlerFn) {
        let api = self.api();
        let bus = unsafe { &*api.bus };
        let sub = Box::into_raw(Box::new(Subscription::<E, F>::new(handler, phase)));
        unsafe {
            let at = (bus.resolve)(bus.ctx, YogStr::from_str(address));
            (bus.on)(bus.ctx, at, priority, sub.cast::<c_void>(), run);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yog_event::{block, player};

    /// Runs `on_event` for `handler` in `phase` on `bytes`.
    fn deliver_bytes<E: Event, R: IntoFlow, F: Fn(&E, &EventCx) -> R + Send + Sync>(handler: F, phase: EventPhase, bytes: &[u8]) -> YogStatus {
        let sub = Subscription::<E, F>::new(Arc::new(handler), phase);
        unsafe { on_event::<E, F, R>((&sub as *const Subscription<E, F>).cast_mut().cast::<c_void>(), YogBytes::new(bytes), std::ptr::null()) }
    }

    fn deliver<E: Event, R: IntoFlow, F: Fn(&E, &EventCx) -> R + Send + Sync>(handler: F, phase: EventPhase, event: &E) -> YogStatus {
        deliver_bytes(handler, phase, &event.to_bytes())
    }

    fn chat(message: &str) -> player::chat::Message {
        player::chat::Message { player_name: "Dev".into(), message: message.into(), dimension: "minecraft:overworld".into() }
    }

    #[test]
    fn events_arrive_read_with_their_phase_and_a_pre_false_cancels() {
        let allow = |e: &player::chat::Message, _: &EventCx| e.message != "cancel me";
        assert_eq!(deliver(allow, EventPhase::Pre, &chat("hi")), status::OK);
        assert_eq!(deliver(allow, EventPhase::Pre, &chat("cancel me")), status::CANCELLED);
        assert_eq!(deliver(allow, EventPhase::Post, &chat("cancel me")), status::OK, "only a pre-event can be cancelled");
    }

    #[test]
    fn a_panicking_handler_allows_the_action() {
        let handler = |_: &block::Break, _: &EventCx| -> bool { panic!("mod bug") };
        let event = block::Break { player_name: "Dev".into(), block_id: "minecraft:stone".into(), pos: yog_core::BlockPos { x: 1, y: 2, z: 3 }, dimension: "minecraft:overworld".into() };
        assert_eq!(deliver(handler, EventPhase::Pre, &event), status::OK);
    }

    #[test]
    fn a_payload_that_isnt_the_event_is_refused() {
        let handler = |_: &player::chat::Message, _: &EventCx| true;
        assert_eq!(deliver_bytes(handler, EventPhase::Post, b"junk"), status::BAD_PAYLOAD);
    }

    /// Runs `on_change` for `handler` on `event`: its status and reply.
    fn change<E: Event, F: Fn(&mut E, &EventCx) + Send + Sync>(handler: F, event: &E) -> (YogStatus, Vec<u8>) {
        let sub = Subscription::<E, F>::new(Arc::new(handler), EventPhase::Post);
        let mut out = Vec::<u8>::new();
        let reply = YogReply::to_vec(&mut out);
        let ud = (&sub as *const Subscription<E, F>).cast_mut().cast::<c_void>();
        let st = unsafe { on_change::<E, F>(ud, YogBytes::new(&event.to_bytes()), &reply) };
        (st, out)
    }

    #[test]
    fn a_changing_handler_replies_with_the_changed_event() {
        use yog_event::Message;
        use yog_event::world::loot::{Roll, Stack};
        let more = |loot: &mut Roll, _: &EventCx| loot.items.push(Stack::new("minecraft:diamond", 1));
        let (st, out) = change(more, &Roll { table: "minecraft:chests/igloo_chest".into(), ..Default::default() });
        assert_eq!(st, status::OK);
        let changed = Roll::from_bytes(&out).expect("the reply is the event");
        assert_eq!(changed.items, vec![Stack::new("minecraft:diamond", 1)]);
    }

    #[test]
    fn a_handler_returning_nothing_lets_it_happen() {
        let handler = |_: &player::chat::Message, _: &EventCx| {};
        assert_eq!(deliver(handler, EventPhase::Pre, &chat("hi")), status::OK);
        let cancel = |_: &player::chat::Message, _: &EventCx| Flow::Cancel;
        assert_eq!(deliver(cancel, EventPhase::Pre, &chat("hi")), status::CANCELLED);
    }
}

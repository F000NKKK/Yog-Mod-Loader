//! Events through the bus: one subscription for every event type, and the
//! one trampoline that hands its payload to a mod's handler.

use std::marker::PhantomData;
use std::os::raw::c_void;
use std::sync::Arc;

use yog_abi::{YogBytes, YogReply, YogServer, YogStatus, YogStr, status};
use yog_event::{Event, EventPhase};

use crate::registry::{CServer, Registry, guard};
use yog_core::Server;

/// What an event subscription keeps: the handler (shared by its pre and post
/// subscriptions), the server it hands it, and which phase this one is.
struct Subscription<E, F> {
    handler: Arc<F>,
    server: *const YogServer,
    phase: EventPhase,
    event: PhantomData<fn(&E)>,
}

/// The default order among a event's handlers.
const DEFAULT_PRIORITY: i32 = 0;

/// The bus's handler for subscription `ud`: the payload read as `E`, the
/// mod's handler run; its `false` in the pre phase cancels the event.
unsafe extern "C" fn on_event<E: Event, F>(ud: *mut c_void, payload: YogBytes, _reply: *const YogReply) -> YogStatus
where
    F: Fn(&E, EventPhase, &dyn Server) -> bool + Send + Sync,
{
    let s = unsafe { &*(ud as *const Subscription<E, F>) };
    let Some(event) = E::from_bytes(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    let keep = guard(true, || (s.handler)(&event, s.phase, &CServer(s.server)));
    if s.phase == EventPhase::Pre && !keep { status::CANCELLED } else { status::OK }
}

impl Registry {
    /// Runs `handler` on every `E`: before the game does it (`Pre` — `false`
    /// cancels it), if `E` can be stopped, and after (`Post`).
    pub fn on<E: Event + 'static, F>(&mut self, handler: F)
    where
        F: Fn(&E, EventPhase, &dyn Server) -> bool + Send + Sync + 'static,
    {
        let handler = Arc::new(handler);
        self.subscribe::<E, F>(E::ADDRESS, EventPhase::Post, Arc::clone(&handler));
        if let Some(pre) = E::PRE {
            self.subscribe::<E, F>(pre, EventPhase::Pre, handler);
        }
    }

    fn subscribe<E: Event + 'static, F>(&mut self, address: &str, phase: EventPhase, handler: Arc<F>)
    where
        F: Fn(&E, EventPhase, &dyn Server) -> bool + Send + Sync + 'static,
    {
        let api = self.api();
        let bus = unsafe { &*api.bus };
        let sub = Box::into_raw(Box::new(Subscription { handler, server: api.server, phase, event: PhantomData::<fn(&E)> }));
        unsafe {
            let at = (bus.resolve)(bus.ctx, YogStr::from_str(address));
            (bus.on)(bus.ctx, at, DEFAULT_PRIORITY, sub.cast::<c_void>(), on_event::<E, F>);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yog_event::{BlockBreakEvent, ChatEvent};

    /// Runs `on_event` for `handler` in `phase` on `bytes`.
    fn deliver_bytes<E: Event, F>(handler: F, phase: EventPhase, bytes: &[u8]) -> YogStatus
    where
        F: Fn(&E, EventPhase, &dyn Server) -> bool + Send + Sync,
    {
        let sub = Subscription { handler: Arc::new(handler), server: std::ptr::null(), phase, event: PhantomData::<fn(&E)> };
        unsafe { on_event::<E, F>((&sub as *const Subscription<E, F>).cast_mut().cast::<c_void>(), YogBytes::new(bytes), std::ptr::null()) }
    }

    fn deliver<E: Event, F>(handler: F, phase: EventPhase, event: &E) -> YogStatus
    where
        F: Fn(&E, EventPhase, &dyn Server) -> bool + Send + Sync,
    {
        deliver_bytes(handler, phase, &event.to_bytes())
    }

    fn chat(message: &str) -> ChatEvent {
        ChatEvent { player_name: "Dev".into(), message: message.into(), dimension: "minecraft:overworld".into() }
    }

    #[test]
    fn events_arrive_read_with_their_phase_and_a_pre_false_cancels() {
        let allow = |e: &ChatEvent, _: EventPhase, _: &dyn Server| e.message != "cancel me";
        assert_eq!(deliver(allow, EventPhase::Pre, &chat("hi")), status::OK);
        assert_eq!(deliver(allow, EventPhase::Pre, &chat("cancel me")), status::CANCELLED);
        assert_eq!(deliver(allow, EventPhase::Post, &chat("cancel me")), status::OK, "only a pre-event can be cancelled");
    }

    #[test]
    fn a_panicking_handler_allows_the_action() {
        let handler = |_: &BlockBreakEvent, _: EventPhase, _: &dyn Server| -> bool { panic!("mod bug") };
        let event = BlockBreakEvent { player_name: "Dev".into(), block_id: "minecraft:stone".into(), pos: yog_core::BlockPos { x: 1, y: 2, z: 3 }, dimension: "minecraft:overworld".into() };
        assert_eq!(deliver(handler, EventPhase::Pre, &event), status::OK);
    }

    #[test]
    fn a_payload_that_isnt_the_event_is_refused() {
        let handler = |_: &ChatEvent, _: EventPhase, _: &dyn Server| true;
        assert_eq!(deliver_bytes(handler, EventPhase::Post, b"junk"), status::BAD_PAYLOAD);
    }
}

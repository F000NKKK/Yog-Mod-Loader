//! The registry mod authors use to subscribe to events.

use crate::events::{BlockBreakEvent, ChatEvent};

type Handler<E> = Box<dyn Fn(&E) + Send + Sync + 'static>;

/// Collects the event handlers registered by mods. The Yog runtime owns one of
/// these and dispatches incoming events from the Java host into it.
#[derive(Default)]
pub struct Registry {
    block_break: Vec<Handler<BlockBreakEvent>>,
    chat: Vec<Handler<ChatEvent>>,
}

impl Registry {
    /// Subscribe to block-break events.
    pub fn on_block_break<F>(&mut self, handler: F)
    where
        F: Fn(&BlockBreakEvent) + Send + Sync + 'static,
    {
        self.block_break.push(Box::new(handler));
    }

    /// Subscribe to chat events.
    pub fn on_chat<F>(&mut self, handler: F)
    where
        F: Fn(&ChatEvent) + Send + Sync + 'static,
    {
        self.chat.push(Box::new(handler));
    }

    // --- dispatch: called by the runtime, not by mod authors ---

    pub fn dispatch_block_break(&self, event: &BlockBreakEvent) {
        for handler in &self.block_break {
            handler(event);
        }
    }

    pub fn dispatch_chat(&self, event: &ChatEvent) {
        for handler in &self.chat {
            handler(event);
        }
    }
}

/// Implemented by every Yog mod. The runtime calls [`Mod::register`] once at
/// startup so the mod can subscribe to the events it cares about.
pub trait Mod {
    fn register(registry: &mut Registry);
}

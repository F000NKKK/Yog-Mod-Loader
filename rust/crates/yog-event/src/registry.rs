//! The registry mod authors use to subscribe to events.

use yog_core::Server;

use crate::events::{BlockBreakEvent, ChatEvent, PlayerJoinEvent, PlayerLeaveEvent};

type Handler<E> = Box<dyn Fn(&E, &dyn Server) + Send + Sync + 'static>;
type Listener = Box<dyn Fn(&dyn Server) + Send + Sync + 'static>;

/// Collects the event handlers registered by mods. The Yog runtime owns one of
/// these and dispatches incoming events from the Java host into it.
///
/// Every handler receives a [`Server`] handle so it can act on the game, not
/// just observe it.
#[derive(Default)]
pub struct Registry {
    block_break: Vec<Handler<BlockBreakEvent>>,
    chat: Vec<Handler<ChatEvent>>,
    player_join: Vec<Handler<PlayerJoinEvent>>,
    player_leave: Vec<Handler<PlayerLeaveEvent>>,
    server_started: Vec<Listener>,
    server_stopping: Vec<Listener>,
}

impl Registry {
    /// Subscribe to block-break events.
    pub fn on_block_break<F>(&mut self, handler: F)
    where
        F: Fn(&BlockBreakEvent, &dyn Server) + Send + Sync + 'static,
    {
        self.block_break.push(Box::new(handler));
    }

    /// Subscribe to chat events.
    pub fn on_chat<F>(&mut self, handler: F)
    where
        F: Fn(&ChatEvent, &dyn Server) + Send + Sync + 'static,
    {
        self.chat.push(Box::new(handler));
    }

    /// Subscribe to player-join events.
    pub fn on_player_join<F>(&mut self, handler: F)
    where
        F: Fn(&PlayerJoinEvent, &dyn Server) + Send + Sync + 'static,
    {
        self.player_join.push(Box::new(handler));
    }

    /// Subscribe to player-leave events.
    pub fn on_player_leave<F>(&mut self, handler: F)
    where
        F: Fn(&PlayerLeaveEvent, &dyn Server) + Send + Sync + 'static,
    {
        self.player_leave.push(Box::new(handler));
    }

    /// Subscribe to the "server started" lifecycle event.
    pub fn on_server_started<F>(&mut self, listener: F)
    where
        F: Fn(&dyn Server) + Send + Sync + 'static,
    {
        self.server_started.push(Box::new(listener));
    }

    /// Subscribe to the "server stopping" lifecycle event.
    pub fn on_server_stopping<F>(&mut self, listener: F)
    where
        F: Fn(&dyn Server) + Send + Sync + 'static,
    {
        self.server_stopping.push(Box::new(listener));
    }

    // --- dispatch: called by the runtime, not by mod authors ---

    pub fn dispatch_block_break(&self, event: &BlockBreakEvent, server: &dyn Server) {
        for handler in &self.block_break {
            handler(event, server);
        }
    }

    pub fn dispatch_chat(&self, event: &ChatEvent, server: &dyn Server) {
        for handler in &self.chat {
            handler(event, server);
        }
    }

    pub fn dispatch_player_join(&self, event: &PlayerJoinEvent, server: &dyn Server) {
        for handler in &self.player_join {
            handler(event, server);
        }
    }

    pub fn dispatch_player_leave(&self, event: &PlayerLeaveEvent, server: &dyn Server) {
        for handler in &self.player_leave {
            handler(event, server);
        }
    }

    pub fn dispatch_server_started(&self, server: &dyn Server) {
        for listener in &self.server_started {
            listener(server);
        }
    }

    pub fn dispatch_server_stopping(&self, server: &dyn Server) {
        for listener in &self.server_stopping {
            listener(server);
        }
    }
}

/// Implemented by every Yog mod. The runtime calls [`Mod::register`] once at
/// startup so the mod can subscribe to the events it cares about.
pub trait Mod {
    fn register(registry: &mut Registry);
}

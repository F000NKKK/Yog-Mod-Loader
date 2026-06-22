//! The registration hub mod authors use, composing every Yog domain.
//!
//! Lives in the facade because this is where domains come together: events
//! (`yog-event`), commands (`yog-command`), and the [`Server`] handle
//! (`yog-core`) all meet here.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use yog_command::CommandContext;
use yog_core::Server;
use yog_event::{
    AttackEntityEvent, BlockBreakEvent, ChatEvent, EntityDamageEvent, EntityDeathEvent,
    PlayerJoinEvent, PlayerLeaveEvent, UseBlockEvent, UseItemEvent,
};
use yog_network::PacketEvent;
use yog_registry::{BlockDef, ItemDef};

type Handler<E> = Box<dyn Fn(&E, &dyn Server) + Send + Sync + 'static>;
type Listener = Box<dyn Fn(&dyn Server) + Send + Sync + 'static>;
type CommandHandler =
    Box<dyn Fn(&CommandContext, &dyn Server) -> Option<String> + Send + Sync + 'static>;

/// A pending scheduled task (one-shot or repeating).
struct ScheduledTask {
    /// Ticks remaining until the handler fires.
    remaining: u64,
    /// If `Some(n)`, re-schedule with `remaining = n` after firing.
    period: Option<u64>,
    handler: Arc<dyn Fn(&dyn Server) + Send + Sync + 'static>,
}

/// Collects everything a mod registers. The Yog runtime owns one of these and
/// drives it from the Java host. Every handler receives a [`Server`] handle so
/// it can act on the game, not just observe it.
#[derive(Default)]
pub struct Registry {
    block_break: Vec<Handler<BlockBreakEvent>>,
    chat: Vec<Handler<ChatEvent>>,
    player_join: Vec<Handler<PlayerJoinEvent>>,
    player_leave: Vec<Handler<PlayerLeaveEvent>>,
    use_item: Vec<Handler<UseItemEvent>>,
    use_block: Vec<Handler<UseBlockEvent>>,
    attack_entity: Vec<Handler<AttackEntityEvent>>,
    entity_damage: Vec<Handler<EntityDamageEvent>>,
    entity_death: Vec<Handler<EntityDeathEvent>>,
    server_started: Vec<Listener>,
    server_stopping: Vec<Listener>,
    server_tick: Vec<Listener>,
    commands: HashMap<String, CommandHandler>,
    items: Vec<ItemDef>,
    blocks: Vec<BlockDef>,
    server_packets: HashMap<String, Vec<Handler<PacketEvent>>>,
    client_packets: HashMap<String, Vec<Handler<PacketEvent>>>,
    /// Pending scheduled tasks — uses interior mutability so `dispatch_server_tick`
    /// can advance the queue without a write lock on the whole Registry.
    scheduled: Mutex<Vec<ScheduledTask>>,
}

impl Registry {
    // ── events ──────────────────────────────────────────────────────────────

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

    /// Subscribe to item-use (right-click) events.
    pub fn on_use_item<F>(&mut self, handler: F)
    where
        F: Fn(&UseItemEvent, &dyn Server) + Send + Sync + 'static,
    {
        self.use_item.push(Box::new(handler));
    }

    /// Subscribe to block-use (right-click on a block) events.
    pub fn on_use_block<F>(&mut self, handler: F)
    where
        F: Fn(&UseBlockEvent, &dyn Server) + Send + Sync + 'static,
    {
        self.use_block.push(Box::new(handler));
    }

    /// Subscribe to attack-entity (left-click on an entity) events.
    pub fn on_attack_entity<F>(&mut self, handler: F)
    where
        F: Fn(&AttackEntityEvent, &dyn Server) + Send + Sync + 'static,
    {
        self.attack_entity.push(Box::new(handler));
    }

    /// Subscribe to living-entity damage events.
    pub fn on_entity_damage<F>(&mut self, handler: F)
    where
        F: Fn(&EntityDamageEvent, &dyn Server) + Send + Sync + 'static,
    {
        self.entity_damage.push(Box::new(handler));
    }

    /// Subscribe to living-entity death events.
    pub fn on_entity_death<F>(&mut self, handler: F)
    where
        F: Fn(&EntityDeathEvent, &dyn Server) + Send + Sync + 'static,
    {
        self.entity_death.push(Box::new(handler));
    }

    /// Subscribe to the end of every server tick (20×/second). Keep these cheap.
    pub fn on_tick<F>(&mut self, listener: F)
    where
        F: Fn(&dyn Server) + Send + Sync + 'static,
    {
        self.server_tick.push(Box::new(listener));
    }

    /// Run `handler` once after `delay_ticks` ticks (e.g. 20 = 1 second).
    pub fn schedule_once<F>(&self, delay_ticks: u64, handler: F)
    where
        F: Fn(&dyn Server) + Send + Sync + 'static,
    {
        self.scheduled.lock().expect("scheduler poisoned").push(ScheduledTask {
            remaining: delay_ticks,
            period: None,
            handler: Arc::new(handler),
        });
    }

    /// Run `handler` repeatedly, first time after `period_ticks`, then every
    /// `period_ticks` thereafter. Returns immediately; cancellation is not yet
    /// supported (schedule a one-shot that re-schedules itself for conditional
    /// repeating logic).
    pub fn schedule_repeating<F>(&self, period_ticks: u64, handler: F)
    where
        F: Fn(&dyn Server) + Send + Sync + 'static,
    {
        self.scheduled.lock().expect("scheduler poisoned").push(ScheduledTask {
            remaining: period_ticks,
            period: Some(period_ticks),
            handler: Arc::new(handler),
        });
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

    // ── commands ────────────────────────────────────────────────────────────

    /// Register `/name`. The handler may return a reply sent back to the source.
    pub fn on_command<F>(&mut self, name: impl Into<String>, handler: F)
    where
        F: Fn(&CommandContext, &dyn Server) -> Option<String> + Send + Sync + 'static,
    {
        self.commands.insert(name.into(), Box::new(handler));
    }

    /// Names of all registered commands (used by the host to wire Brigadier).
    pub fn command_names(&self) -> Vec<String> {
        self.commands.keys().cloned().collect()
    }

    // ── content (custom items / blocks) ─────────────────────────────────────

    /// Register a custom item. Takes effect during host startup, before the
    /// game's registries freeze.
    pub fn register_item(&mut self, def: ItemDef) {
        self.items.push(def);
    }

    /// Register a custom block (and a matching item).
    pub fn register_block(&mut self, def: BlockDef) {
        self.blocks.push(def);
    }

    /// Declared items / blocks (used by the host to register them).
    pub fn items(&self) -> &[ItemDef] {
        &self.items
    }
    pub fn blocks(&self) -> &[BlockDef] {
        &self.blocks
    }

    // ── networking (raw-byte packets) ───────────────────────────────────────

    /// Handle a packet received on the **server** from a client, on `channel`.
    pub fn on_packet<F>(&mut self, channel: impl Into<String>, handler: F)
    where
        F: Fn(&PacketEvent, &dyn Server) + Send + Sync + 'static,
    {
        self.server_packets.entry(channel.into()).or_default().push(Box::new(handler));
    }

    /// Handle a packet received on the **client** from the server, on `channel`.
    pub fn on_client_packet<F>(&mut self, channel: impl Into<String>, handler: F)
    where
        F: Fn(&PacketEvent, &dyn Server) + Send + Sync + 'static,
    {
        self.client_packets.entry(channel.into()).or_default().push(Box::new(handler));
    }

    /// Channels the host must register receivers for (server / client).
    pub fn packet_channels(&self) -> Vec<String> {
        self.server_packets.keys().cloned().collect()
    }
    pub fn client_packet_channels(&self) -> Vec<String> {
        self.client_packets.keys().cloned().collect()
    }

    // ── dispatch: called by the runtime, not by mod authors ─────────────────

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

    pub fn dispatch_use_item(&self, event: &UseItemEvent, server: &dyn Server) {
        for handler in &self.use_item {
            handler(event, server);
        }
    }

    pub fn dispatch_use_block(&self, event: &UseBlockEvent, server: &dyn Server) {
        for handler in &self.use_block {
            handler(event, server);
        }
    }

    pub fn dispatch_attack_entity(&self, event: &AttackEntityEvent, server: &dyn Server) {
        for handler in &self.attack_entity {
            handler(event, server);
        }
    }

    pub fn dispatch_entity_damage(&self, event: &EntityDamageEvent, server: &dyn Server) {
        for handler in &self.entity_damage {
            handler(event, server);
        }
    }

    pub fn dispatch_entity_death(&self, event: &EntityDeathEvent, server: &dyn Server) {
        for handler in &self.entity_death {
            handler(event, server);
        }
    }

    pub fn dispatch_server_tick(&self, server: &dyn Server) {
        for listener in &self.server_tick {
            listener(server);
        }

        // Advance the scheduler: collect ready handlers, rebuild the queue,
        // then fire — all without holding the lock during handler execution.
        let to_fire: Vec<Arc<dyn Fn(&dyn Server) + Send + Sync>> = {
            let mut tasks = self.scheduled.lock().expect("scheduler poisoned");
            let mut ready = Vec::new();
            let mut kept = Vec::new();
            for mut task in tasks.drain(..) {
                if task.remaining == 0 {
                    ready.push(Arc::clone(&task.handler));
                    if let Some(period) = task.period {
                        task.remaining = period;
                        kept.push(task);
                    }
                } else {
                    task.remaining -= 1;
                    kept.push(task);
                }
            }
            *tasks = kept;
            ready
        };
        for handler in to_fire {
            handler(server);
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

    /// Run the handler for `ctx.name`, returning its reply (if any).
    pub fn dispatch_command(&self, ctx: &CommandContext, server: &dyn Server) -> Option<String> {
        self.commands.get(&ctx.name).and_then(|h| h(ctx, server))
    }

    pub fn dispatch_packet(&self, event: &PacketEvent, server: &dyn Server) {
        if let Some(hs) = self.server_packets.get(&event.channel) {
            for h in hs {
                h(event, server);
            }
        }
    }

    pub fn dispatch_client_packet(&self, event: &PacketEvent, server: &dyn Server) {
        if let Some(hs) = self.client_packets.get(&event.channel) {
            for h in hs {
                h(event, server);
            }
        }
    }
}

/// Implemented by every Yog mod. The runtime calls [`Mod::register`] once at
/// startup so the mod can register the events and commands it cares about.
pub trait Mod {
    fn register(registry: &mut Registry);
}

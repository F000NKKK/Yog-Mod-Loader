//! Registration hub and mod entry-point trait.
//!
//! [`Registry`] wraps the [`YogApi`] C table passed to `yog_mod_register` and
//! provides an ergonomic Rust API over it.  All closures registered here are
//! boxed on the heap and their raw pointers handed to the runtime; the runtime
//! then drives them via C function-pointer calls.  Closures are intentionally
//! leaked (never freed) — they live for the entire process lifetime, which is
//! correct for a game server.

use std::os::raw::c_void;

use yog_abi::{
    YogApi, YogAttackEntityEvent, YogBlockBreakEvent, YogBlockDef, YogChatEvent,
    YogCommandEvent, YogEntityDamageEvent, YogEntityDeathEvent, YogItemDef, YogPacketEvent,
    YogPlayerEvent, YogServer, YogStr, YogUseBlockEvent, YogUseItemEvent,
};
use yog_command::CommandContext;
use yog_core::Server;
use yog_event::{
    AttackEntityEvent, BlockBreakEvent, ChatEvent, EntityDamageEvent, EntityDeathEvent,
    PlayerJoinEvent, PlayerLeaveEvent, UseBlockEvent, UseItemEvent,
};
use yog_network::PacketEvent;
use yog_registry::{BlockDef, ItemDef};

// ── CServer — implements Server via the YogServer function table ──────────────

/// A handle to the runtime's server actions, backed by [`YogServer`] fn pointers.
/// Given to every handler as `&dyn Server`.
pub struct CServer(pub *const YogServer);

unsafe impl Send for CServer {}
unsafe impl Sync for CServer {}

macro_rules! srv {
    ($self:ident) => { unsafe { &*$self.0 } };
}

impl Server for CServer {
    fn broadcast(&self, message: &str) {
        let s = srv!(self);
        unsafe { (s.broadcast)(s.ctx, YogStr::from_str(message)) }
    }

    fn get_block(&self, dimension: &str, pos: yog_core::BlockPos) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe {
            (s.get_block)(s.ctx, YogStr::from_str(dimension),
                yog_abi::YogBlockPos { x: pos.x, y: pos.y, z: pos.z })
        };
        if owned.is_none() { return None; }
        let result = unsafe {
            String::from_utf8(
                std::slice::from_raw_parts(owned.ptr, owned.len as usize).to_vec()
            ).ok()
        };
        unsafe { (s.free_str)(owned.ptr, owned.len) };
        result
    }

    fn set_block(&self, dimension: &str, pos: yog_core::BlockPos, block_id: &str) -> bool {
        let s = srv!(self);
        unsafe {
            (s.set_block)(s.ctx, YogStr::from_str(dimension),
                yog_abi::YogBlockPos { x: pos.x, y: pos.y, z: pos.z },
                YogStr::from_str(block_id))
        }
    }

    fn give_item(&self, player: &str, item_id: &str, count: u32) -> bool {
        let s = srv!(self);
        unsafe { (s.give_item)(s.ctx, YogStr::from_str(player), YogStr::from_str(item_id), count) }
    }

    fn teleport(&self, player: &str, x: f64, y: f64, z: f64) -> bool {
        let s = srv!(self);
        unsafe { (s.player_teleport)(s.ctx, YogStr::from_str(player), yog_abi::YogVec3 { x, y, z }) }
    }

    fn send_to_player(&self, player: &str, channel: &str, payload: &[u8]) -> bool {
        let s = srv!(self);
        unsafe {
            (s.send_to_player)(s.ctx, YogStr::from_str(player), YogStr::from_str(channel),
                payload.as_ptr(), payload.len() as u32)
        }
    }

    fn send_to_server(&self, channel: &str, payload: &[u8]) -> bool {
        let s = srv!(self);
        unsafe {
            (s.send_to_server)(s.ctx, YogStr::from_str(channel),
                payload.as_ptr(), payload.len() as u32)
        }
    }

    fn entity_teleport(&self, uuid: &str, x: f64, y: f64, z: f64) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_teleport)(s.ctx, YogStr::from_str(uuid), yog_abi::YogVec3 { x, y, z }) }
    }

    fn entity_position(&self, uuid: &str) -> Option<(f64, f64, f64)> {
        let s = srv!(self);
        let mut out = yog_abi::YogVec3 { x: 0.0, y: 0.0, z: 0.0 };
        if unsafe { (s.entity_position)(s.ctx, YogStr::from_str(uuid), &mut out) } {
            Some((out.x, out.y, out.z))
        } else {
            None
        }
    }

    fn entity_health(&self, uuid: &str) -> Option<f32> {
        let s = srv!(self);
        let mut hp = 0f32;
        if unsafe { (s.entity_health)(s.ctx, YogStr::from_str(uuid), &mut hp) } { Some(hp) } else { None }
    }

    fn entity_set_health(&self, uuid: &str, health: f32) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_set_health)(s.ctx, YogStr::from_str(uuid), health) }
    }

    fn entity_kill(&self, uuid: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_kill)(s.ctx, YogStr::from_str(uuid)) }
    }

    fn spawn_entity(&self, entity_type: &str, dimension: &str, x: f64, y: f64, z: f64) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe {
            (s.spawn_entity)(s.ctx, YogStr::from_str(entity_type),
                YogStr::from_str(dimension), yog_abi::YogVec3 { x, y, z })
        };
        if owned.is_none() { return None; }
        let result = unsafe {
            String::from_utf8(
                std::slice::from_raw_parts(owned.ptr, owned.len as usize).to_vec()
            ).ok()
        };
        unsafe { (s.free_str)(owned.ptr, owned.len) };
        result
    }

    fn entity_add_effect(&self, uuid: &str, effect_id: &str, duration_ticks: i32, amplifier: u8, show_particles: bool) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_add_effect)(s.ctx, YogStr::from_str(uuid), YogStr::from_str(effect_id), duration_ticks, amplifier, show_particles) }
    }

    fn entity_remove_effect(&self, uuid: &str, effect_id: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_remove_effect)(s.ctx, YogStr::from_str(uuid), YogStr::from_str(effect_id)) }
    }

    fn entity_clear_effects(&self, uuid: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_clear_effects)(s.ctx, YogStr::from_str(uuid)) }
    }

    fn drop_loot(&self, table_id: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool {
        let s = srv!(self);
        unsafe { (s.drop_loot)(s.ctx, YogStr::from_str(table_id), YogStr::from_str(dimension), yog_abi::YogVec3 { x, y, z }) }
    }

    fn has_item_tag(&self, item_id: &str, tag_id: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.has_item_tag)(s.ctx, YogStr::from_str(item_id), YogStr::from_str(tag_id)) }
    }

    fn has_block_tag(&self, block_id: &str, tag_id: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.has_block_tag)(s.ctx, YogStr::from_str(block_id), YogStr::from_str(tag_id)) }
    }

    fn world_time(&self, dimension: &str) -> Option<i64> {
        let s = srv!(self);
        let mut t = 0i64;
        if unsafe { (s.world_time)(s.ctx, YogStr::from_str(dimension), &mut t) } { Some(t) } else { None }
    }

    fn world_set_time(&self, dimension: &str, time: i64) -> bool {
        let s = srv!(self);
        unsafe { (s.set_time)(s.ctx, YogStr::from_str(dimension), time) }
    }

    fn world_is_raining(&self, dimension: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.is_raining)(s.ctx, YogStr::from_str(dimension)) }
    }

    fn world_set_weather(&self, dimension: &str, raining: bool, duration_ticks: i32) -> bool {
        let s = srv!(self);
        unsafe { (s.set_weather)(s.ctx, YogStr::from_str(dimension), raining, duration_ticks) }
    }

    fn entity_velocity(&self, uuid: &str) -> Option<(f64, f64, f64)> {
        let s = srv!(self);
        let mut v = yog_abi::YogVec3 { x: 0.0, y: 0.0, z: 0.0 };
        if unsafe { (s.entity_velocity)(s.ctx, YogStr::from_str(uuid), &mut v) } {
            Some((v.x, v.y, v.z))
        } else {
            None
        }
    }

    fn entity_set_velocity(&self, uuid: &str, vx: f64, vy: f64, vz: f64) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_set_velocity)(s.ctx, YogStr::from_str(uuid), yog_abi::YogVec3 { x: vx, y: vy, z: vz }) }
    }

    fn entity_add_velocity(&self, uuid: &str, vx: f64, vy: f64, vz: f64) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_add_velocity)(s.ctx, YogStr::from_str(uuid), yog_abi::YogVec3 { x: vx, y: vy, z: vz }) }
    }

    fn scoreboard_get(&self, objective: &str, player: &str) -> Option<i32> {
        let s = srv!(self);
        let mut score = 0i32;
        if unsafe { (s.scoreboard_get)(s.ctx, YogStr::from_str(objective), YogStr::from_str(player), &mut score) } { Some(score) } else { None }
    }

    fn scoreboard_set(&self, objective: &str, player: &str, score: i32) -> bool {
        let s = srv!(self);
        unsafe { (s.scoreboard_set)(s.ctx, YogStr::from_str(objective), YogStr::from_str(player), score) }
    }

    fn scoreboard_add(&self, objective: &str, player: &str, delta: i32) -> Option<i32> {
        let s = srv!(self);
        let mut new_score = 0i32;
        if unsafe { (s.scoreboard_add)(s.ctx, YogStr::from_str(objective), YogStr::from_str(player), delta, &mut new_score) } { Some(new_score) } else { None }
    }

    fn game_dir(&self) -> String {
        let s = srv!(self);
        let owned = unsafe { (s.game_dir)(s.ctx) };
        if owned.is_none() { return String::new(); }
        let result = unsafe {
            String::from_utf8(
                std::slice::from_raw_parts(owned.ptr, owned.len as usize).to_vec()
            ).unwrap_or_default()
        };
        unsafe { (s.free_str)(owned.ptr, owned.len) };
        result
    }

    fn play_sound(&self, dimension: &str, x: f64, y: f64, z: f64, sound_id: &str, volume: f32, pitch: f32) -> bool {
        let s = srv!(self);
        unsafe { (s.play_sound)(s.ctx, YogStr::from_str(dimension), yog_abi::YogVec3 { x, y, z }, YogStr::from_str(sound_id), volume, pitch) }
    }

    fn play_sound_to_player(&self, player: &str, sound_id: &str, volume: f32, pitch: f32) -> bool {
        let s = srv!(self);
        unsafe { (s.play_sound_player)(s.ctx, YogStr::from_str(player), YogStr::from_str(sound_id), volume, pitch) }
    }

    fn send_title(&self, player: &str, title: &str, subtitle: &str, fadein: i32, stay: i32, fadeout: i32) -> bool {
        let s = srv!(self);
        unsafe { (s.send_title)(s.ctx, YogStr::from_str(player), YogStr::from_str(title), YogStr::from_str(subtitle), fadein, stay, fadeout) }
    }

    fn send_actionbar(&self, player: &str, message: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.send_actionbar)(s.ctx, YogStr::from_str(player), YogStr::from_str(message)) }
    }

    fn kick_player(&self, player: &str, reason: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.kick_player)(s.ctx, YogStr::from_str(player), YogStr::from_str(reason)) }
    }

    fn set_gamemode(&self, player: &str, gamemode: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.set_gamemode)(s.ctx, YogStr::from_str(player), YogStr::from_str(gamemode)) }
    }

    fn bossbar_create(&self, id: &str, title: &str, color: &str, style: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.bossbar_create)(s.ctx, YogStr::from_str(id), YogStr::from_str(title), YogStr::from_str(color), YogStr::from_str(style)) }
    }

    fn bossbar_remove(&self, id: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.bossbar_remove)(s.ctx, YogStr::from_str(id)) }
    }

    fn bossbar_set_title(&self, id: &str, title: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.bossbar_set_title)(s.ctx, YogStr::from_str(id), YogStr::from_str(title)) }
    }

    fn bossbar_set_progress(&self, id: &str, progress: f32) -> bool {
        let s = srv!(self);
        unsafe { (s.bossbar_set_progress)(s.ctx, YogStr::from_str(id), progress) }
    }

    fn bossbar_set_color(&self, id: &str, color: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.bossbar_set_color)(s.ctx, YogStr::from_str(id), YogStr::from_str(color)) }
    }

    fn bossbar_add_player(&self, id: &str, player: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.bossbar_add_player)(s.ctx, YogStr::from_str(id), YogStr::from_str(player)) }
    }

    fn bossbar_remove_player(&self, id: &str, player: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.bossbar_remove_player)(s.ctx, YogStr::from_str(id), YogStr::from_str(player)) }
    }

    fn bossbar_set_visible(&self, id: &str, visible: bool) -> bool {
        let s = srv!(self);
        unsafe { (s.bossbar_set_visible)(s.ctx, YogStr::from_str(id), visible) }
    }

    fn get_block_nbt(&self, dimension: &str, pos: yog_core::BlockPos) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe {
            (s.get_block_nbt)(s.ctx, YogStr::from_str(dimension),
                yog_abi::YogBlockPos { x: pos.x, y: pos.y, z: pos.z })
        };
        if owned.is_none() { return None; }
        let result = unsafe {
            String::from_utf8(
                std::slice::from_raw_parts(owned.ptr, owned.len as usize).to_vec()
            ).ok()
        };
        unsafe { (s.free_str)(owned.ptr, owned.len) };
        result
    }

    fn set_block_nbt(&self, dimension: &str, pos: yog_core::BlockPos, snbt: &str) -> bool {
        let s = srv!(self);
        unsafe {
            (s.set_block_nbt)(s.ctx, YogStr::from_str(dimension),
                yog_abi::YogBlockPos { x: pos.x, y: pos.y, z: pos.z },
                YogStr::from_str(snbt))
        }
    }

    fn player_inventory(&self, player: &str) -> Vec<(u32, String, u32)> {
        let s = srv!(self);
        let owned = unsafe { (s.player_inventory)(s.ctx, YogStr::from_str(player)) };
        if owned.is_none() { return Vec::new(); }
        let text = unsafe {
            String::from_utf8(
                std::slice::from_raw_parts(owned.ptr, owned.len as usize).to_vec()
            ).unwrap_or_default()
        };
        unsafe { (s.free_str)(owned.ptr, owned.len) };
        text.lines().filter_map(|line| {
            let mut it = line.split('\t');
            let slot: u32 = it.next()?.parse().ok()?;
            let item_id = it.next()?.to_owned();
            let count: u32 = it.next()?.parse().ok()?;
            Some((slot, item_id, count))
        }).collect()
    }

    fn player_set_slot(&self, player: &str, slot: u32, item_id: &str, count: u32) -> bool {
        let s = srv!(self);
        unsafe {
            (s.player_set_slot)(s.ctx, YogStr::from_str(player), slot, YogStr::from_str(item_id), count)
        }
    }

    fn teleport_to_dim(&self, player: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool {
        let s = srv!(self);
        unsafe {
            (s.player_teleport_dim)(s.ctx, YogStr::from_str(player),
                YogStr::from_str(dimension), yog_abi::YogVec3 { x, y, z })
        }
    }

    fn entity_teleport_to_dim(&self, uuid: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool {
        let s = srv!(self);
        unsafe {
            (s.entity_teleport_dim)(s.ctx, YogStr::from_str(uuid),
                YogStr::from_str(dimension), yog_abi::YogVec3 { x, y, z })
        }
    }
}

// ── Trampoline helpers ────────────────────────────────────────────────────────
//
// Each trampoline is a `unsafe extern "C" fn` that:
//   1. Casts `ud` back to the original boxed closure.
//   2. Converts ABI C structs to Rust event types.
//   3. Builds a `CServer` from the `*const YogServer`.
//   4. Calls the closure.
//
// Closures are Box::into_raw'd in Registry methods — they are INTENTIONALLY
// leaked and live for the process lifetime.

unsafe extern "C" fn trampoline_block_break<F>(ud: *mut c_void, srv: *const YogServer, ev: *const YogBlockBreakEvent)
where F: Fn(&BlockBreakEvent, &dyn Server) + Send + Sync,
{
    let f = &*(ud as *const F);
    let ev = &*ev;
    let rust_ev = BlockBreakEvent {
        player_name: ev.player.as_str().to_owned(),
        block_id:    ev.block.as_str().to_owned(),
        pos: yog_core::BlockPos { x: ev.pos.x, y: ev.pos.y, z: ev.pos.z },
    };
    f(&rust_ev, &CServer(srv));
}

unsafe extern "C" fn trampoline_chat<F>(ud: *mut c_void, srv: *const YogServer, ev: *const YogChatEvent)
where F: Fn(&ChatEvent, &dyn Server) + Send + Sync,
{
    let f = &*(ud as *const F);
    let ev = &*ev;
    let rust_ev = ChatEvent { player_name: ev.player.as_str().to_owned(), message: ev.message.as_str().to_owned() };
    f(&rust_ev, &CServer(srv));
}

unsafe extern "C" fn trampoline_player_join<F>(ud: *mut c_void, srv: *const YogServer, ev: *const YogPlayerEvent)
where F: Fn(&PlayerJoinEvent, &dyn Server) + Send + Sync,
{
    let f = &*(ud as *const F);
    let ev = &*ev;
    let rust_ev = PlayerJoinEvent { player_name: ev.player.as_str().to_owned(), uuid: ev.uuid.as_str().to_owned() };
    f(&rust_ev, &CServer(srv));
}

unsafe extern "C" fn trampoline_player_leave<F>(ud: *mut c_void, srv: *const YogServer, ev: *const YogPlayerEvent)
where F: Fn(&PlayerLeaveEvent, &dyn Server) + Send + Sync,
{
    let f = &*(ud as *const F);
    let ev = &*ev;
    let rust_ev = PlayerLeaveEvent { player_name: ev.player.as_str().to_owned(), uuid: ev.uuid.as_str().to_owned() };
    f(&rust_ev, &CServer(srv));
}

unsafe extern "C" fn trampoline_use_item<F>(ud: *mut c_void, srv: *const YogServer, ev: *const YogUseItemEvent)
where F: Fn(&UseItemEvent, &dyn Server) + Send + Sync,
{
    let f = &*(ud as *const F);
    let ev = &*ev;
    let rust_ev = UseItemEvent { player_name: ev.player.as_str().to_owned(), item_id: ev.item.as_str().to_owned() };
    f(&rust_ev, &CServer(srv));
}

unsafe extern "C" fn trampoline_use_block<F>(ud: *mut c_void, srv: *const YogServer, ev: *const YogUseBlockEvent)
where F: Fn(&UseBlockEvent, &dyn Server) + Send + Sync,
{
    let f = &*(ud as *const F);
    let ev = &*ev;
    let rust_ev = UseBlockEvent {
        player_name: ev.player.as_str().to_owned(),
        block_id:    ev.block.as_str().to_owned(),
        pos: yog_core::BlockPos { x: ev.pos.x, y: ev.pos.y, z: ev.pos.z },
    };
    f(&rust_ev, &CServer(srv));
}

unsafe extern "C" fn trampoline_attack_entity<F>(ud: *mut c_void, srv: *const YogServer, ev: *const YogAttackEntityEvent)
where F: Fn(&AttackEntityEvent, &dyn Server) + Send + Sync,
{
    let f = &*(ud as *const F);
    let ev = &*ev;
    let rust_ev = AttackEntityEvent {
        player_name: ev.player.as_str().to_owned(),
        target_type: ev.target_type.as_str().to_owned(),
        target_uuid: ev.target_uuid.as_str().to_owned(),
    };
    f(&rust_ev, &CServer(srv));
}

unsafe extern "C" fn trampoline_entity_damage<F>(ud: *mut c_void, srv: *const YogServer, ev: *const YogEntityDamageEvent)
where F: Fn(&EntityDamageEvent, &dyn Server) + Send + Sync,
{
    let f = &*(ud as *const F);
    let ev = &*ev;
    let rust_ev = EntityDamageEvent {
        entity_type: ev.entity_type.as_str().to_owned(),
        uuid:        ev.uuid.as_str().to_owned(),
        amount:      ev.amount,
        source:      ev.source.as_str().to_owned(),
    };
    f(&rust_ev, &CServer(srv));
}

unsafe extern "C" fn trampoline_entity_death<F>(ud: *mut c_void, srv: *const YogServer, ev: *const YogEntityDeathEvent)
where F: Fn(&EntityDeathEvent, &dyn Server) + Send + Sync,
{
    let f = &*(ud as *const F);
    let ev = &*ev;
    let rust_ev = EntityDeathEvent {
        entity_type: ev.entity_type.as_str().to_owned(),
        uuid:        ev.uuid.as_str().to_owned(),
        source:      ev.source.as_str().to_owned(),
    };
    f(&rust_ev, &CServer(srv));
}

unsafe extern "C" fn trampoline_server_fn<F>(ud: *mut c_void, srv: *const YogServer)
where F: Fn(&dyn Server) + Send + Sync,
{
    let f = &*(ud as *const F);
    f(&CServer(srv));
}

unsafe extern "C" fn trampoline_packet<F>(ud: *mut c_void, srv: *const YogServer, ev: *const YogPacketEvent)
where F: Fn(&PacketEvent, &dyn Server) + Send + Sync,
{
    let f = &*(ud as *const F);
    let ev = &*ev;
    let rust_ev = PacketEvent {
        channel: ev.channel.as_str().to_owned(),
        player:  ev.player.as_str().to_owned(),
        payload: std::slice::from_raw_parts(ev.payload, ev.payload_len as usize).to_vec(),
    };
    f(&rust_ev, &CServer(srv));
}

unsafe extern "C" fn trampoline_command<F>(
    ud: *mut c_void,
    srv: *const YogServer,
    ev: *const YogCommandEvent,
    reply_buf: *mut u8,
    reply_cap: u32,
    reply_len: *mut u32,
) where F: Fn(&CommandContext, &dyn Server) -> Option<String> + Send + Sync,
{
    let f = &*(ud as *const F);
    let ev = &*ev;
    let ctx = CommandContext {
        name:   ev.name.as_str().to_owned(),
        args:   ev.args.as_str().to_owned(),
        source: ev.source.as_str().to_owned(),
        uuid:   ev.uuid.as_str().to_owned(),
    };
    *reply_len = 0;
    if let Some(reply) = f(&ctx, &CServer(srv)) {
        let bytes = reply.as_bytes();
        let n = bytes.len().min(reply_cap as usize);
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), reply_buf, n);
        *reply_len = n as u32;
    }
}

unsafe extern "C" fn trampoline_scheduled<F>(ud: *mut c_void, srv: *const YogServer)
where F: Fn(&dyn Server) + Send + Sync,
{
    let f = &*(ud as *const F);
    f(&CServer(srv));
}

// ── Registry ─────────────────────────────────────────────────────────────────

/// Wraps the [`YogApi`] table and provides an ergonomic registration API.
///
/// Obtained inside `yog_mod_register` via `export_mod!`.  Closures registered
/// here are boxed and leaked — they live as long as the process (which is the
/// correct lifetime for a server mod).
pub struct Registry {
    api: *const YogApi,
}

// SAFETY: `api` is a static provided by the runtime, valid for process lifetime.
unsafe impl Send for Registry {}
unsafe impl Sync for Registry {}

impl Registry {
    /// Build from the pointer passed by the runtime. Only called by `export_mod!`.
    pub unsafe fn from_raw(api: *const YogApi) -> Self {
        Self { api }
    }

    #[inline]
    fn ctx(&self) -> *mut c_void { unsafe { (*self.api).ctx } }

    // ── helpers ──────────────────────────────────────────────────────────────

    fn leak<F: 'static>(f: F) -> *mut c_void {
        Box::into_raw(Box::new(f)) as *mut c_void
    }

    // ── events ───────────────────────────────────────────────────────────────

    pub fn on_block_break<F>(&mut self, handler: F)
    where F: Fn(&BlockBreakEvent, &dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_block_break)(self.ctx(), ud, trampoline_block_break::<F>) }
    }

    pub fn on_chat<F>(&mut self, handler: F)
    where F: Fn(&ChatEvent, &dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_chat)(self.ctx(), ud, trampoline_chat::<F>) }
    }

    pub fn on_player_join<F>(&mut self, handler: F)
    where F: Fn(&PlayerJoinEvent, &dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_player_join)(self.ctx(), ud, trampoline_player_join::<F>) }
    }

    pub fn on_player_leave<F>(&mut self, handler: F)
    where F: Fn(&PlayerLeaveEvent, &dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_player_leave)(self.ctx(), ud, trampoline_player_leave::<F>) }
    }

    pub fn on_use_item<F>(&mut self, handler: F)
    where F: Fn(&UseItemEvent, &dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_use_item)(self.ctx(), ud, trampoline_use_item::<F>) }
    }

    pub fn on_use_block<F>(&mut self, handler: F)
    where F: Fn(&UseBlockEvent, &dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_use_block)(self.ctx(), ud, trampoline_use_block::<F>) }
    }

    pub fn on_attack_entity<F>(&mut self, handler: F)
    where F: Fn(&AttackEntityEvent, &dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_attack_entity)(self.ctx(), ud, trampoline_attack_entity::<F>) }
    }

    pub fn on_entity_damage<F>(&mut self, handler: F)
    where F: Fn(&EntityDamageEvent, &dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_entity_damage)(self.ctx(), ud, trampoline_entity_damage::<F>) }
    }

    pub fn on_entity_death<F>(&mut self, handler: F)
    where F: Fn(&EntityDeathEvent, &dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_entity_death)(self.ctx(), ud, trampoline_entity_death::<F>) }
    }

    pub fn on_tick<F>(&mut self, listener: F)
    where F: Fn(&dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(listener);
        unsafe { ((*self.api).on_server_tick)(self.ctx(), ud, trampoline_server_fn::<F>) }
    }

    pub fn on_server_started<F>(&mut self, listener: F)
    where F: Fn(&dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(listener);
        unsafe { ((*self.api).on_server_started)(self.ctx(), ud, trampoline_server_fn::<F>) }
    }

    pub fn on_server_stopping<F>(&mut self, listener: F)
    where F: Fn(&dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(listener);
        unsafe { ((*self.api).on_server_stopping)(self.ctx(), ud, trampoline_server_fn::<F>) }
    }

    // ── commands ─────────────────────────────────────────────────────────────

    pub fn on_command<F>(&mut self, name: impl AsRef<str>, handler: F)
    where F: Fn(&CommandContext, &dyn Server) -> Option<String> + Send + Sync + 'static {
        let name_ys = YogStr::from_str(name.as_ref());
        let ud = Self::leak(handler);
        unsafe { ((*self.api).register_command)(self.ctx(), name_ys, ud, trampoline_command::<F>) }
    }

    /// Register a command with Brigadier-typed arguments.
    ///
    /// `schema` is a space-separated list of argument types:
    /// `int`, `float`, `word`, `string` (greedy, must be last), `player`, `blockpos`.
    ///
    /// In the handler use `ctx.arg_int(0)`, `ctx.arg_blockpos(1)`, etc.
    pub fn on_typed_command<F>(&mut self, name: impl AsRef<str>, schema: impl AsRef<str>, handler: F)
    where F: Fn(&CommandContext, &dyn Server) -> Option<String> + Send + Sync + 'static {
        let name_ys   = YogStr::from_str(name.as_ref());
        let schema_ys = YogStr::from_str(schema.as_ref());
        let ud = Self::leak(handler);
        unsafe { ((*self.api).register_typed_command)(self.ctx(), name_ys, schema_ys, ud, trampoline_command::<F>) }
    }

    // ── networking ───────────────────────────────────────────────────────────

    pub fn on_packet<F>(&mut self, channel: impl AsRef<str>, handler: F)
    where F: Fn(&PacketEvent, &dyn Server) + Send + Sync + 'static {
        let ch = YogStr::from_str(channel.as_ref());
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_packet)(self.ctx(), ch, ud, trampoline_packet::<F>) }
    }

    pub fn on_client_packet<F>(&mut self, channel: impl AsRef<str>, handler: F)
    where F: Fn(&PacketEvent, &dyn Server) + Send + Sync + 'static {
        let ch = YogStr::from_str(channel.as_ref());
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_client_packet)(self.ctx(), ch, ud, trampoline_packet::<F>) }
    }

    // ── content ──────────────────────────────────────────────────────────────

    pub fn register_item(&mut self, def: ItemDef) {
        // Build a C-compatible YogItemDef whose YogStr fields point into `def`'s
        // String storage.  We then call register_item which must copy the data
        // before returning (the runtime stores owned Strings).
        let food_nutrition  = def.food.as_ref().map_or(0, |f| f.nutrition);
        let food_saturation = def.food.as_ref().map_or(0.0, |f| f.saturation);
        let food_always_eat = def.food.as_ref().map_or(false, |f| f.can_always_eat);
        let c = YogItemDef {
            id:              YogStr::from_str(&def.id),
            max_stack:       def.max_stack as u32,
            name:            def.name.as_deref().map(YogStr::from_str).unwrap_or(YogStr::EMPTY),
            tooltip:         def.tooltip.as_deref().map(YogStr::from_str).unwrap_or(YogStr::EMPTY),
            max_damage:      def.max_damage,
            fire_resistant:  def.fire_resistant,
            fuel_ticks:      def.fuel_ticks,
            food_nutrition,
            food_saturation,
            food_always_eat,
        };
        unsafe { ((*self.api).register_item)(self.ctx(), &c) }
    }

    pub fn register_block(&mut self, def: BlockDef) {
        let shape = def.shape.unwrap_or([0.0; 6]);
        let c = YogBlockDef {
            id:            YogStr::from_str(&def.id),
            hardness:      def.hardness,
            resistance:    def.resistance,
            name:          def.name.as_deref().map(YogStr::from_str).unwrap_or(YogStr::EMPTY),
            light_level:   def.light_level,
            sound:         def.sound.as_deref().map(YogStr::from_str).unwrap_or(YogStr::EMPTY),
            requires_tool: def.requires_tool,
            no_collision:  def.no_collision,
            slipperiness:  def.slipperiness,
            shape,
        };
        unsafe { ((*self.api).register_block)(self.ctx(), &c) }
    }

    // ── scheduler ────────────────────────────────────────────────────────────

    pub fn schedule_once<F>(&self, delay_ticks: u64, handler: F)
    where F: Fn(&dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).schedule_once)(self.ctx(), delay_ticks, ud, trampoline_scheduled::<F>) }
    }

    pub fn schedule_repeating<F>(&self, period_ticks: u64, handler: F)
    where F: Fn(&dyn Server) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).schedule_repeating)(self.ctx(), period_ticks, ud, trampoline_scheduled::<F>) }
    }
}

// ── Mod trait ─────────────────────────────────────────────────────────────────

/// Implemented by every Yog mod. Called once at startup to register handlers.
pub trait Mod {
    fn register(registry: &mut Registry);
}

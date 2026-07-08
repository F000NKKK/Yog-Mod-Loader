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
    YogAdvancementEvent, YogApi, YogAttackEntityEvent, YogBlockBreakEvent, YogBlockDef,
    YogChatEvent, YogCommandEvent, YogContainerCloseEvent, YogContainerOpenEvent, YogCraftEvent,
    YogEntityDamageEvent, YogEntityDeathEvent, YogEntityInteractEvent, YogEntitySpawnEvent,
    YogExplosionEvent, YogGfxApi, YogItemDef, YogItemPickupEvent, YogKeyPressEvent,
    YogPacketEvent, YogPlaceBlockEvent, YogPlayerDeathEvent, YogPlayerEvent, YogPlayerMoveEvent,
    YogPlayerRespawnEvent, YogProjectileHitEvent, YogServer, YogStr, YogStartupGrantDef,
    YogUseBlockEvent, YogUseItemEvent,
};
use yog_book::Book;
use yog_gfx::GfxContext;
use yog_command::CommandContext;
use yog_core::Server;
use yog_event::{
    AdvancementEvent, AttackEntityEvent, BlockBreakEvent, ChatEvent, ClientTickEvent,
    ContainerCloseEvent, ContainerOpenEvent, CraftEvent, EntityDamageEvent, EntityDeathEvent,
    EntityInteractEvent, EntitySpawnEvent, EventPhase, ExplosionEvent,
    ItemPickupEvent, KeyPressEvent, PlaceBlockEvent, PlayerDeathEvent, PlayerJoinEvent,
    PlayerLeaveEvent, PlayerMoveEvent, PlayerRespawnEvent, ProjectileHitEvent, ScreenEvent,
    UseBlockEvent, UseItemEvent,
};
use yog_network::{Packet, PacketEvent};
use yog_registry::{BlockDef, FurnaceRecipe, ItemDef, ShapedRecipe, ShapelessRecipe, StartupGrant};

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

    fn entity_rotation(&self, uuid: &str) -> Option<(f32, f32)> {
        let s = srv!(self);
        let mut out = yog_abi::YogVec3 { x: 0.0, y: 0.0, z: 0.0 };
        let ok = unsafe { (s.entity_rotation)(s.ctx, YogStr::from_str(uuid), &mut out) };
        if ok { Some((out.x as f32, out.y as f32)) } else { None }
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

    fn online_players(&self) -> Vec<String> {
        let s = srv!(self);
        let owned = unsafe { (s.online_players)(s.ctx) };
        if owned.is_none() { return Vec::new(); }
        let text = unsafe {
            String::from_utf8(
                std::slice::from_raw_parts(owned.ptr, owned.len as usize).to_vec()
            ).unwrap_or_default()
        };
        unsafe { (s.free_str)(owned.ptr, owned.len) };
        if text.is_empty() { Vec::new() } else { text.lines().map(str::to_owned).collect() }
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

    fn world_entity_count(&self, dimension: &str, entity_type: &str) -> i32 {
        let s = srv!(self);
        unsafe { (s.world_entity_count)(s.ctx, YogStr::from_str(dimension), YogStr::from_str(entity_type)) }
    }

    fn entity_get_nbt(&self, uuid: &str) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe { (s.entity_get_nbt)(s.ctx, YogStr::from_str(uuid)) };
        if owned.is_none() { return None; }
        let result = unsafe {
            String::from_utf8(std::slice::from_raw_parts(owned.ptr, owned.len as usize).to_vec()).ok()
        };
        unsafe { (s.free_str)(owned.ptr, owned.len) };
        result
    }

    fn entity_set_nbt(&self, uuid: &str, snbt: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_set_nbt)(s.ctx, YogStr::from_str(uuid), YogStr::from_str(snbt)) }
    }

    fn spawn_particles(&self, dimension: &str, x: f64, y: f64, z: f64, particle_type: &str, count: i32, dx: f64, dy: f64, dz: f64, speed: f64) -> bool {
        let s = srv!(self);
        unsafe {
            (s.spawn_particles)(s.ctx, YogStr::from_str(dimension),
                yog_abi::YogVec3 { x, y, z }, YogStr::from_str(particle_type),
                count, dx, dy, dz, speed)
        }
    }

    fn entity_attribute_get(&self, uuid: &str, attribute_id: &str) -> Option<f64> {
        let s = srv!(self);
        let v = unsafe { (s.entity_attribute_get)(s.ctx, YogStr::from_str(uuid), YogStr::from_str(attribute_id)) };
        if v.is_nan() { None } else { Some(v) }
    }

    fn entity_attribute_set(&self, uuid: &str, attribute_id: &str, value: f64) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_attribute_set)(s.ctx, YogStr::from_str(uuid), YogStr::from_str(attribute_id), value) }
    }

    fn get_held_item_nbt(&self, player: &str) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe { (s.get_held_item_nbt)(s.ctx, YogStr::from_str(player)) };
        if owned.is_none() { return None; }
        let result = unsafe {
            String::from_utf8(std::slice::from_raw_parts(owned.ptr, owned.len as usize).to_vec()).ok()
        };
        unsafe { (s.free_str)(owned.ptr, owned.len) };
        result
    }

    fn set_held_item_nbt(&self, player: &str, snbt: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.set_held_item_nbt)(s.ctx, YogStr::from_str(player), YogStr::from_str(snbt)) }
    }

    fn get_offhand_item_nbt(&self, player: &str) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe { (s.get_offhand_item_nbt)(s.ctx, YogStr::from_str(player)) };
        if owned.is_none() { return None; }
        let result = unsafe {
            String::from_utf8(std::slice::from_raw_parts(owned.ptr, owned.len as usize).to_vec()).ok()
        };
        unsafe { (s.free_str)(owned.ptr, owned.len) };
        result
    }

    fn set_offhand_item_nbt(&self, player: &str, snbt: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.set_offhand_item_nbt)(s.ctx, YogStr::from_str(player), YogStr::from_str(snbt)) }
    }

    fn get_slot_item(&self, player: &str, slot: u32) -> Option<(String, u32, String)> {
        let s = srv!(self);
        let owned = unsafe { (s.get_slot_item)(s.ctx, YogStr::from_str(player), slot) };
        if owned.is_none() { return None; }
        let text = unsafe {
            String::from_utf8(std::slice::from_raw_parts(owned.ptr, owned.len as usize).to_vec())
                .unwrap_or_default()
        };
        unsafe { (s.free_str)(owned.ptr, owned.len) };
        let mut it = text.splitn(3, '\t');
        let item_id = it.next()?.to_owned();
        let count: u32 = it.next()?.parse().ok()?;
        let nbt = it.next().unwrap_or("{}").to_owned();
        Some((item_id, count, nbt))
    }

    fn set_slot_item(&self, player: &str, slot: u32, item_id: &str, count: u32, snbt: &str) -> bool {
        let s = srv!(self);
        unsafe {
            (s.set_slot_item)(s.ctx, YogStr::from_str(player), slot,
                YogStr::from_str(item_id), count, YogStr::from_str(snbt))
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

// ── Trampoline helpers ────────────────────────────────────────────────────────
//
// All phased event trampolines share the same pattern:
//   `phase: u8`  0 = EventPhase::Pre, 1 = EventPhase::Post
//   Return value is meaningful only in Pre phase.

macro_rules! trampoline_phased {
    ($name:ident, $abi_ev:ty, $rust_ev:ty, |$ev:ident| $build:expr) => {
        unsafe extern "C" fn $name<F>(
            ud: *mut c_void, srv: *const YogServer, ev: *const $abi_ev, phase: u8,
        ) -> bool
        where F: Fn(&$rust_ev, EventPhase, &dyn Server) -> bool + Send + Sync,
        {
            let f = &*(ud as *const F);
            let $ev = &*ev;
            let rust_ev = $build;
            let p = if phase == 0 { EventPhase::Pre } else { EventPhase::Post };
            f(&rust_ev, p, &CServer(srv))
        }
    };
}

trampoline_phased!(trampoline_block_break, YogBlockBreakEvent, BlockBreakEvent, |ev| BlockBreakEvent {
    player_name: ev.player.as_str().to_owned(),
    block_id:    ev.block.as_str().to_owned(),
    pos: yog_core::BlockPos { x: ev.pos.x, y: ev.pos.y, z: ev.pos.z },
});

trampoline_phased!(trampoline_chat, YogChatEvent, ChatEvent, |ev| ChatEvent {
    player_name: ev.player.as_str().to_owned(),
    message:     ev.message.as_str().to_owned(),
});

trampoline_phased!(trampoline_player_join, YogPlayerEvent, PlayerJoinEvent, |ev| PlayerJoinEvent {
    player_name: ev.player.as_str().to_owned(),
    uuid:        ev.uuid.as_str().to_owned(),
});

trampoline_phased!(trampoline_player_leave, YogPlayerEvent, PlayerLeaveEvent, |ev| PlayerLeaveEvent {
    player_name: ev.player.as_str().to_owned(),
    uuid:        ev.uuid.as_str().to_owned(),
});

trampoline_phased!(trampoline_use_item, YogUseItemEvent, UseItemEvent, |ev| UseItemEvent {
    player_name: ev.player.as_str().to_owned(),
    item_id:     ev.item.as_str().to_owned(),
    sneaking: ev.sneaking,
});

trampoline_phased!(trampoline_use_block, YogUseBlockEvent, UseBlockEvent, |ev| UseBlockEvent {
    player_name: ev.player.as_str().to_owned(),
    block_id:    ev.block.as_str().to_owned(),
    pos: yog_core::BlockPos { x: ev.pos.x, y: ev.pos.y, z: ev.pos.z },
});

trampoline_phased!(trampoline_attack_entity, YogAttackEntityEvent, AttackEntityEvent, |ev| AttackEntityEvent {
    player_name: ev.player.as_str().to_owned(),
    target_type: ev.target_type.as_str().to_owned(),
    target_uuid: ev.target_uuid.as_str().to_owned(),
});

trampoline_phased!(trampoline_entity_damage, YogEntityDamageEvent, EntityDamageEvent, |ev| EntityDamageEvent {
    entity_type: ev.entity_type.as_str().to_owned(),
    uuid:        ev.uuid.as_str().to_owned(),
    amount:      ev.amount,
    source:      ev.source.as_str().to_owned(),
});

trampoline_phased!(trampoline_entity_death, YogEntityDeathEvent, EntityDeathEvent, |ev| EntityDeathEvent {
    entity_type: ev.entity_type.as_str().to_owned(),
    uuid:        ev.uuid.as_str().to_owned(),
    source:      ev.source.as_str().to_owned(),
});

trampoline_phased!(trampoline_entity_spawn, YogEntitySpawnEvent, EntitySpawnEvent, |ev| EntitySpawnEvent {
    entity_type: ev.entity_type.as_str().to_owned(),
    uuid:        ev.uuid.as_str().to_owned(),
    dimension:   ev.dimension.as_str().to_owned(),
});

trampoline_phased!(trampoline_place_block, YogPlaceBlockEvent, PlaceBlockEvent, |ev| PlaceBlockEvent {
    player_name: ev.player.as_str().to_owned(),
    block_id:    ev.block.as_str().to_owned(),
    pos: yog_core::BlockPos { x: ev.pos.x, y: ev.pos.y, z: ev.pos.z },
});

trampoline_phased!(trampoline_player_death, YogPlayerDeathEvent, PlayerDeathEvent, |ev| PlayerDeathEvent {
    player_name: ev.player.as_str().to_owned(),
    uuid:        ev.uuid.as_str().to_owned(),
    source:      ev.source.as_str().to_owned(),
});

trampoline_phased!(trampoline_player_respawn, YogPlayerRespawnEvent, PlayerRespawnEvent, |ev| PlayerRespawnEvent {
    player_name: ev.player.as_str().to_owned(),
    uuid:        ev.uuid.as_str().to_owned(),
    at_anchor:   ev.at_anchor,
});

trampoline_phased!(trampoline_advancement, YogAdvancementEvent, AdvancementEvent, |ev| AdvancementEvent {
    player_name:    ev.player.as_str().to_owned(),
    uuid:           ev.uuid.as_str().to_owned(),
    advancement_id: ev.advancement.as_str().to_owned(),
});

trampoline_phased!(trampoline_entity_interact, YogEntityInteractEvent, EntityInteractEvent, |ev| EntityInteractEvent {
    player_name: ev.player.as_str().to_owned(),
    player_uuid: ev.player_uuid.as_str().to_owned(),
    entity_type: ev.entity_type.as_str().to_owned(),
    entity_uuid: ev.entity_uuid.as_str().to_owned(),
    hand:        ev.hand.as_str().to_owned(),
});

trampoline_phased!(trampoline_craft, YogCraftEvent, CraftEvent, |ev| CraftEvent {
    player_name:  ev.player.as_str().to_owned(),
    player_uuid:  ev.player_uuid.as_str().to_owned(),
    result_item:  ev.result_item.as_str().to_owned(),
    result_count: ev.result_count,
});

trampoline_phased!(trampoline_explosion, YogExplosionEvent, ExplosionEvent, |ev| ExplosionEvent {
    dimension:  ev.dimension.as_str().to_owned(),
    x:          ev.x,
    y:          ev.y,
    z:          ev.z,
    power:      ev.power,
    cause_uuid: ev.cause_uuid.as_str().to_owned(),
});

trampoline_phased!(trampoline_item_pickup, YogItemPickupEvent, ItemPickupEvent, |ev| ItemPickupEvent {
    player_name: ev.player.as_str().to_owned(),
    player_uuid: ev.player_uuid.as_str().to_owned(),
    item_id:     ev.item_id.as_str().to_owned(),
    item_count:  ev.item_count,
    entity_uuid: ev.entity_uuid.as_str().to_owned(),
});

trampoline_phased!(trampoline_player_move, YogPlayerMoveEvent, PlayerMoveEvent, |ev| PlayerMoveEvent {
    player_name: ev.player.as_str().to_owned(),
    player_uuid: ev.player_uuid.as_str().to_owned(),
    x:     ev.x,
    y:     ev.y,
    z:     ev.z,
    yaw:   ev.yaw,
    pitch: ev.pitch,
});

trampoline_phased!(trampoline_container_open, YogContainerOpenEvent, ContainerOpenEvent, |ev| ContainerOpenEvent {
    player_name:    ev.player.as_str().to_owned(),
    player_uuid:    ev.player_uuid.as_str().to_owned(),
    container_type: ev.container_type.as_str().to_owned(),
});

trampoline_phased!(trampoline_container_close, YogContainerCloseEvent, ContainerCloseEvent, |ev| ContainerCloseEvent {
    player_name: ev.player.as_str().to_owned(),
    player_uuid: ev.player_uuid.as_str().to_owned(),
});

trampoline_phased!(trampoline_projectile_hit, YogProjectileHitEvent, ProjectileHitEvent, |ev| ProjectileHitEvent {
    projectile_type: ev.projectile_type.as_str().to_owned(),
    projectile_uuid: ev.projectile_uuid.as_str().to_owned(),
    shooter_uuid:    ev.shooter_uuid.as_str().to_owned(),
    hit_type:        ev.hit_type.as_str().to_owned(),
    hit_entity_uuid: ev.hit_entity_uuid.as_str().to_owned(),
    x:               ev.x,
    y:               ev.y,
    z:               ev.z,
    dimension:       ev.dimension.as_str().to_owned(),
});

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

// ── ABI minor 10 — client-side trampolines ────────────────────────────────────

unsafe extern "C" fn trampoline_client_tick<F>(ud: *mut c_void)
where F: Fn(&ClientTickEvent) + Send + Sync,
{
    let f = &*(ud as *const F);
    f(&ClientTickEvent {});
}

unsafe extern "C" fn trampoline_hud_render<F>(ud: *mut c_void, gfx: *const YogGfxApi)
where F: Fn(&GfxContext) + Send + Sync,
{
    let f = &*(ud as *const F);
    f(&GfxContext::from_raw(gfx));
}

unsafe extern "C" fn trampoline_world_render<F>(ud: *mut c_void, gfx: *const YogGfxApi)
where F: Fn(&GfxContext) + Send + Sync,
{
    let f = &*(ud as *const F);
    f(&GfxContext::from_raw(gfx));
}

unsafe extern "C" fn trampoline_key_press<F>(
    ud: *mut c_void,
    ev: *const YogKeyPressEvent,
) -> bool
where F: Fn(&KeyPressEvent) -> bool + Send + Sync,
{
    let f = &*(ud as *const F);
    let e = &*ev;
    f(&KeyPressEvent {
        key_code:  e.key_code,
        scan_code: e.scan_code,
        action:    e.action,
        modifiers: e.modifiers,
    })
}

unsafe extern "C" fn trampoline_screen<F>(ud: *mut c_void, screen_class: YogStr) -> bool
where F: Fn(&ScreenEvent) + Send + Sync,
{
    let f = &*(ud as *const F);
    f(&ScreenEvent { screen_class: screen_class.as_str().to_owned() });
    true
}

// ── Registry ─────────────────────────────────────────────────────────────────

/// Wraps the [`YogApi`] table and provides an ergonomic registration API.
///
/// Obtained inside `yog_mod_register` via `export_mod!`.  Closures registered
/// here are boxed and leaked — they live as long as the process (which is the
/// correct lifetime for a server mod).
/// Pointer to the runtime's `YogApi` table (a process-lifetime static in the
/// runtime), captured when the mod registers. Lets free functions like
/// [`installed_mods`] work outside of `register()` — e.g. in UI handlers.
static GLOBAL_API: std::sync::atomic::AtomicPtr<YogApi> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

/// Metadata of an installed mod, as reported by the loader.
#[derive(Debug, Clone)]
pub struct ModInfo {
    /// `"yog"` for .yog mods, `"platform"` for loader (Java) mods.
    pub source:      String,
    pub id:          String,
    pub name:        String,
    pub version:     String,
    /// Comma-separated author list (may be empty).
    pub authors:     String,
    pub description: String,
}

/// All installed mods known to the loader: .yog mods plus, where the host
/// exposes them, platform (Java) mods. Callable at any time after this mod's
/// `register()` ran — including client-side UI handlers. Note that during
/// `register()` mods that load after this one are not in the list yet; query
/// lazily (e.g. on first render) for a complete view.
pub fn installed_mods() -> Vec<ModInfo> {
    let api = GLOBAL_API.load(std::sync::atomic::Ordering::Acquire);
    if api.is_null() { return Vec::new(); }
    let owned = unsafe { ((*api).mods_list)((*api).ctx) };
    if owned.is_none() { return Vec::new(); }
    let text = unsafe {
        String::from_utf8(
            std::slice::from_raw_parts(owned.ptr, owned.len as usize).to_vec()
        ).unwrap_or_default()
    };
    unsafe { ((*api).free_str)(owned.ptr, owned.len) };
    text.lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            Some(ModInfo {
                source:      f.next()?.to_string(),
                id:          f.next()?.to_string(),
                name:        f.next().unwrap_or_default().to_string(),
                version:     f.next().unwrap_or_default().to_string(),
                authors:     f.next().unwrap_or_default().to_string(),
                description: f.next().unwrap_or_default().to_string(),
            })
        })
        .collect()
}

/// Open the Yog UI registered as `ui_id` (client side; no-op on dedicated
/// servers). `modal` blocks game input, `pause` pauses singleplayer. Callable
/// from any handler after registration — e.g. an `on_client_packet` handler.
pub fn open_ui(ui_id: &str, modal: bool, pause: bool) {
    let api = GLOBAL_API.load(std::sync::atomic::Ordering::Acquire);
    if api.is_null() { return; }
    unsafe { ((*api).ui_open)((*api).ctx, YogStr::from_str(ui_id), modal, pause) }
}

/// Handle to the runtime's server-action table, usable from any handler after
/// registration — including client-side ones that don't receive `&dyn Server`
/// (UI event handlers, client tick). Server-world actions are no-ops client
/// side, but networking (`send_to_server`) and similar client-safe calls work.
pub fn server() -> Option<CServer> {
    let api = GLOBAL_API.load(std::sync::atomic::Ordering::Acquire);
    if api.is_null() { return None; }
    let srv = unsafe { (*api).server };
    if srv.is_null() { return None; }
    Some(CServer(srv))
}

pub struct Registry {
    api: *const YogApi,
}

// SAFETY: `api` is a static provided by the runtime, valid for process lifetime.
unsafe impl Send for Registry {}
unsafe impl Sync for Registry {}

impl Registry {
    /// Build from the pointer passed by the runtime. Only called by `export_mod!`.
    pub unsafe fn from_raw(api: *const YogApi) -> Self {
        GLOBAL_API.store(api as *mut YogApi, std::sync::atomic::Ordering::Release);
        Self { api }
    }

    #[inline]
    fn ctx(&self) -> *mut c_void { unsafe { (*self.api).ctx } }

    // ── helpers ──────────────────────────────────────────────────────────────

    fn leak<F: 'static>(f: F) -> *mut c_void {
        Box::into_raw(Box::new(f)) as *mut c_void
    }

    // ── events ───────────────────────────────────────────────────────────────
    //
    // All handlers receive `(event, EventPhase, &dyn Server) -> bool`.
    // In `Pre` phase, returning `false` cancels the action.
    // In `Post` phase, the return value is ignored.
    // A single registration fires for BOTH phases.

    pub fn on_block_break<F>(&mut self, handler: F)
    where F: Fn(&BlockBreakEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_block_break)(self.ctx(), ud, trampoline_block_break::<F>) }
    }

    pub fn on_chat<F>(&mut self, handler: F)
    where F: Fn(&ChatEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_chat)(self.ctx(), ud, trampoline_chat::<F>) }
    }

    pub fn on_player_join<F>(&mut self, handler: F)
    where F: Fn(&PlayerJoinEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_player_join)(self.ctx(), ud, trampoline_player_join::<F>) }
    }

    pub fn on_player_leave<F>(&mut self, handler: F)
    where F: Fn(&PlayerLeaveEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_player_leave)(self.ctx(), ud, trampoline_player_leave::<F>) }
    }

    pub fn on_use_item<F>(&mut self, handler: F)
    where F: Fn(&UseItemEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_use_item)(self.ctx(), ud, trampoline_use_item::<F>) }
    }

    pub fn on_use_block<F>(&mut self, handler: F)
    where F: Fn(&UseBlockEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_use_block)(self.ctx(), ud, trampoline_use_block::<F>) }
    }

    pub fn on_attack_entity<F>(&mut self, handler: F)
    where F: Fn(&AttackEntityEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_attack_entity)(self.ctx(), ud, trampoline_attack_entity::<F>) }
    }

    pub fn on_entity_damage<F>(&mut self, handler: F)
    where F: Fn(&EntityDamageEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_entity_damage)(self.ctx(), ud, trampoline_entity_damage::<F>) }
    }

    pub fn on_entity_death<F>(&mut self, handler: F)
    where F: Fn(&EntityDeathEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_entity_death)(self.ctx(), ud, trampoline_entity_death::<F>) }
    }

    pub fn on_entity_spawn<F>(&mut self, handler: F)
    where F: Fn(&EntitySpawnEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_entity_spawn)(self.ctx(), ud, trampoline_entity_spawn::<F>) }
    }

    pub fn on_player_place_block<F>(&mut self, handler: F)
    where F: Fn(&PlaceBlockEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_player_place_block)(self.ctx(), ud, trampoline_place_block::<F>) }
    }

    pub fn on_player_death<F>(&mut self, handler: F)
    where F: Fn(&PlayerDeathEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_player_death)(self.ctx(), ud, trampoline_player_death::<F>) }
    }

    pub fn on_player_respawn<F>(&mut self, handler: F)
    where F: Fn(&PlayerRespawnEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_player_respawn)(self.ctx(), ud, trampoline_player_respawn::<F>) }
    }

    pub fn on_advancement<F>(&mut self, handler: F)
    where F: Fn(&AdvancementEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_advancement)(self.ctx(), ud, trampoline_advancement::<F>) }
    }

    pub fn on_entity_interact<F>(&mut self, handler: F)
    where F: Fn(&EntityInteractEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_entity_interact)(self.ctx(), ud, trampoline_entity_interact::<F>) }
    }

    pub fn on_item_craft<F>(&mut self, handler: F)
    where F: Fn(&CraftEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_item_craft)(self.ctx(), ud, trampoline_craft::<F>) }
    }

    pub fn on_explosion<F>(&mut self, handler: F)
    where F: Fn(&ExplosionEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_explosion)(self.ctx(), ud, trampoline_explosion::<F>) }
    }

    pub fn on_item_pickup<F>(&mut self, handler: F)
    where F: Fn(&ItemPickupEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_item_pickup)(self.ctx(), ud, trampoline_item_pickup::<F>) }
    }

    pub fn on_player_move<F>(&mut self, handler: F)
    where F: Fn(&PlayerMoveEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_player_move)(self.ctx(), ud, trampoline_player_move::<F>) }
    }

    pub fn on_container_open<F>(&mut self, handler: F)
    where F: Fn(&ContainerOpenEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_container_open)(self.ctx(), ud, trampoline_container_open::<F>) }
    }

    pub fn on_container_close<F>(&mut self, handler: F)
    where F: Fn(&ContainerCloseEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_container_close)(self.ctx(), ud, trampoline_container_close::<F>) }
    }

    pub fn on_projectile_hit<F>(&mut self, handler: F)
    where F: Fn(&ProjectileHitEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_projectile_hit)(self.ctx(), ud, trampoline_projectile_hit::<F>) }
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

    /// Register a typed-packet handler.
    ///
    /// The payload is decoded from raw bytes using `P`'s [`Packet`] impl.
    /// Malformed payloads are silently dropped.
    pub fn on_typed_packet<P, F>(&mut self, channel: impl AsRef<str>, handler: F)
    where
        P: Packet + Send + Sync + 'static,
        F: Fn(&P, &dyn Server) + Send + Sync + 'static,
    {
        self.on_packet(channel, move |ev, srv| {
            if let Some(pkt) = P::decode(&ev.payload) {
                handler(&pkt, srv);
            }
        });
    }

    // ── recipes ──────────────────────────────────────────────────────────────

    fn recipe(&mut self, ns: &str, name: &str, json: &str) {
        unsafe {
            ((*self.api).register_recipe_json)(
                self.ctx(),
                YogStr::from_str(ns), YogStr::from_str(name), YogStr::from_str(json),
            )
        }
    }

    /// Register a shaped crafting recipe.
    pub fn add_shaped_recipe(&mut self, recipe: ShapedRecipe) {
        let json = recipe.to_json();
        let (ns, name) = recipe.ns_name();
        self.recipe(ns, name, &json);
    }

    /// Register a shapeless crafting recipe.
    pub fn add_shapeless_recipe(&mut self, recipe: ShapelessRecipe) {
        let json = recipe.to_json();
        let (ns, name) = recipe.ns_name();
        self.recipe(ns, name, &json);
    }

    /// Register a furnace smelting recipe.
    pub fn add_furnace_recipe(&mut self, recipe: FurnaceRecipe) {
        let json = recipe.to_json();
        let (ns, name) = recipe.ns_name();
        self.recipe(ns, name, &json);
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
        let groups_joined = def.connect_groups.join(",");
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
            connects:      def.connects,
            connect_groups: YogStr::from_str(&groups_joined),
        };
        unsafe { ((*self.api).register_block)(self.ctx(), &c) }
    }

    // ── startup grants ───────────────────────────────────────────────────────

    /// Register a startup grant: items/books to give once when a player first joins.
    pub fn register_startup_grant(&mut self, grant: StartupGrant) {
        let items_str = grant.items.join("|");
        let c = YogStartupGrantDef {
            id:      YogStr::from_str(&grant.id),
            items:   YogStr::from_str(&items_str),
            book:    grant.book.as_deref().map(YogStr::from_str).unwrap_or(YogStr::EMPTY),
            command: grant.command.as_deref().map(YogStr::from_str).unwrap_or(YogStr::EMPTY),
        };
        unsafe { ((*self.api).register_startup_grant)(self.ctx(), &c) }
    }

    /// Register a book with its JSON-serialized content.
    pub fn register_book(&mut self, book: &Book) {
        let json = book.to_json();
        let id = YogStr::from_str(&book.id);
        let j = YogStr::from_str(&json);
        unsafe { ((*self.api).register_book)(self.ctx(), id, j) }
    }


    /// Register a menu entry — the host renders a button on vanilla screens
    /// (TitleScreen on Fabric, ModListScreen on Forge/NeoForge) that opens `ui_id`.
    /// `label` is the human-readable button text (e.g. "Yog Mods").
    /// `ui_id` is the UI to open when clicked (e.g. "yog:modlist").
    /// See [`installed_mods`]. During `register()` the list only contains
    /// mods loaded before this one.
    pub fn installed_mods(&self) -> Vec<ModInfo> {
        installed_mods()
    }

    pub fn register_menu_entry(&mut self, label: &str, ui_id: &str) {
        let l = YogStr::from_str(label);
        let u = YogStr::from_str(ui_id);
        unsafe { ((*self.api).register_menu_entry)(self.ctx(), l, u) }
    }

    /// Register a UI tree with an event callback.
    /// `ui_id` is the unique identifier (e.g. "mymod:menu").
    /// `handler` receives `(ui_id, event_id)` when a widget is clicked.
    pub fn register_ui<F>(&mut self, ui_id: &str, handler: F)
    where F: Fn(&str, &str) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        let id = YogStr::from_str(ui_id);
        let empty = YogStr::from_str("{}");
        unsafe {
            ((*self.api).register_ui)(self.ctx(), id, empty, ud, trampoline_ui_event::<F>)
        }
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

    // ── client-side hooks (ABI minor 10) ─────────────────────────────────────

    /// Register a handler called every client tick (render thread, no server).
    pub fn on_client_tick<F>(&mut self, handler: F)
    where F: Fn(&ClientTickEvent) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_client_tick)(self.ctx(), ud, trampoline_client_tick::<F>) }
    }

    /// Register a render callback for a specific UI screen (`ui_id`).
    ///
    /// Called from `YogUIScreen.render()` — i.e. AFTER `renderBackground()` darkens
    /// the screen — so your UI draws on top of the dimmed world view.
    /// Use this instead of `on_hud_render` for book/inventory screens.
    ///
    /// Clicks are forwarded as `"click:X:Y"` via the `register_ui` handler so you
    /// can do hit-testing on your own stored layout.
    pub fn on_ui_render<F>(&mut self, ui_id: &str, handler: F)
    where F: Fn(&GfxContext) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        let id = YogStr::from_str(ui_id);
        unsafe { ((*self.api).on_ui_render)(self.ctx(), id, ud, trampoline_hud_render::<F>) }
    }

    /// Register a handler called every frame when the HUD is rendered.
    ///
    /// `gfx` provides full GPU pipeline access plus 2D convenience draw calls.
    /// `view_proj` and `camera_pos` are zero in HUD context; use `gfx.draw2d()` for HUD elements.
    pub fn on_hud_render<F>(&mut self, handler: F)
    where F: Fn(&GfxContext) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_hud_render)(self.ctx(), ud, trampoline_hud_render::<F>) }
    }

    /// Register a handler called every frame at the end of world rendering.
    ///
    /// `gfx.view_proj()` is the combined projection × view matrix (camera-relative).
    /// `gfx.camera_pos()` is the camera world position.
    /// To render at world position P, translate by `P - camera_pos` before drawing.
    pub fn on_world_render<F>(&mut self, handler: F)
    where F: Fn(&GfxContext) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_world_render)(self.ctx(), ud, trampoline_world_render::<F>) }
    }

    /// Register a handler for keyboard input (client-side).
    /// Return `false` to prevent Minecraft from processing the key.
    pub fn on_key_press<F>(&mut self, handler: F)
    where F: Fn(&KeyPressEvent) -> bool + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_key_press)(self.ctx(), ud, trampoline_key_press::<F>) }
    }

    /// Register a handler called when a GUI screen is opened.
    pub fn on_screen_open<F>(&mut self, handler: F)
    where F: Fn(&ScreenEvent) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_screen_open)(self.ctx(), ud, trampoline_screen::<F>) }
    }

    /// Register a handler called when a GUI screen is closed.
    pub fn on_screen_close<F>(&mut self, handler: F)
    where F: Fn(&ScreenEvent) + Send + Sync + 'static {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_screen_close)(self.ctx(), ud, trampoline_screen::<F>) }
    }
}

unsafe extern "C" fn trampoline_ui_event<F>(ud: *mut c_void, ui_id: yog_abi::YogStr, event_id: yog_abi::YogStr)
where F: Fn(&str, &str) + Send + Sync + 'static
{
    let f = &*(ud as *const F);
    let ui = unsafe { ui_id.as_str() };
    let ev = unsafe { event_id.as_str() };
    f(ui, ev);
}

// ── Mod trait ─────────────────────────────────────────────────────────────────

/// Implemented by every Yog mod. Called once at startup to register handlers.
pub trait Mod {
    fn register(registry: &mut Registry);
}
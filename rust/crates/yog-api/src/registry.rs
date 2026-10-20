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
    YogApi, YogCommandEvent, YogGfxApi, YogItemInsertEvent, YogMachineCraftEvent,
    YogPacketEvent, YogServer,
    YogStr, };
use yog_formats::book::Book;
use yog_command::CommandContext;
use yog_core::Server;
use yog_dimensions::{ChunkWriter, YogDimensionDef};
use yog_event::EventPhase;
use yog_event::block::Insert as ItemInsertEvent;
use yog_gfx::GfxContext;
use yog_network::{Packet, PacketEvent};
use yog_registry::{ArmorMaterial, BlockDef, ItemDef, Recipe, SoundGroup, StartupGrant, ToolMaterial};

// ── CServer — implements Server via the YogServer function table ──────────────

/// A handle to the runtime's server actions, backed by [`YogServer`] fn pointers.
/// Given to every handler as `&dyn Server`.
pub struct CServer(pub *const YogServer);

unsafe impl Send for CServer {}
unsafe impl Sync for CServer {}

macro_rules! srv {
    ($self:ident) => {
        unsafe { &*$self.0 }
    };
}

impl Server for CServer {
    fn broadcast(&self, message: &str) {
        let s = srv!(self);
        unsafe { (s.broadcast)(s.ctx, YogStr::from_str(message)) }
    }

    fn get_block(&self, dimension: &str, pos: yog_core::BlockPos) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe {
            (s.get_block)(
                s.ctx,
                YogStr::from_str(dimension),
                yog_abi::YogBlockPos {
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                },
            )
        };

        take_owned(s.free_str, owned)
    }

    fn set_block(&self, dimension: &str, pos: yog_core::BlockPos, block_id: &str) -> bool {
        let s = srv!(self);
        unsafe {
            (s.set_block)(
                s.ctx,
                YogStr::from_str(dimension),
                yog_abi::YogBlockPos {
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                },
                YogStr::from_str(block_id),
            )
        }
    }

    fn give_item(&self, player: &str, item_id: &str, count: u32) -> bool {
        let s = srv!(self);
        unsafe {
            (s.give_item)(
                s.ctx,
                YogStr::from_str(player),
                YogStr::from_str(item_id),
                count,
            )
        }
    }

    fn teleport(&self, player: &str, x: f64, y: f64, z: f64) -> bool {
        let s = srv!(self);
        unsafe {
            (s.player_teleport)(
                s.ctx,
                YogStr::from_str(player),
                yog_abi::YogVec3 { x, y, z },
            )
        }
    }

    fn send_to_player(&self, player: &str, channel: &str, payload: &[u8]) -> bool {
        let s = srv!(self);
        unsafe {
            (s.send_to_player)(
                s.ctx,
                YogStr::from_str(player),
                YogStr::from_str(channel),
                payload.as_ptr(),
                payload.len() as u32,
            )
        }
    }

    fn send_to_server(&self, channel: &str, payload: &[u8]) -> bool {
        let s = srv!(self);
        unsafe {
            (s.send_to_server)(
                s.ctx,
                YogStr::from_str(channel),
                payload.as_ptr(),
                payload.len() as u32,
            )
        }
    }

    fn entity_teleport(&self, uuid: &str, x: f64, y: f64, z: f64) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_teleport)(s.ctx, YogStr::from_str(uuid), yog_abi::YogVec3 { x, y, z }) }
    }

    fn entity_position(&self, uuid: &str) -> Option<(f64, f64, f64)> {
        let s = srv!(self);
        let mut out = yog_abi::YogVec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        if unsafe { (s.entity_position)(s.ctx, YogStr::from_str(uuid), &mut out) } {
            Some((out.x, out.y, out.z))
        } else {
            None
        }
    }

    fn entity_health(&self, uuid: &str) -> Option<f32> {
        let s = srv!(self);
        let mut hp = 0f32;
        if unsafe { (s.entity_health)(s.ctx, YogStr::from_str(uuid), &mut hp) } {
            Some(hp)
        } else {
            None
        }
    }

    fn entity_set_health(&self, uuid: &str, health: f32) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_set_health)(s.ctx, YogStr::from_str(uuid), health) }
    }

    fn entity_kill(&self, uuid: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_kill)(s.ctx, YogStr::from_str(uuid)) }
    }

    fn spawn_entity(
        &self,
        entity_type: &str,
        dimension: &str,
        x: f64,
        y: f64,
        z: f64,
    ) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe {
            (s.spawn_entity)(
                s.ctx,
                YogStr::from_str(entity_type),
                YogStr::from_str(dimension),
                yog_abi::YogVec3 { x, y, z },
            )
        };

        take_owned(s.free_str, owned)
    }

    fn entity_add_effect(
        &self,
        uuid: &str,
        effect_id: &str,
        duration_ticks: i32,
        amplifier: u8,
        show_particles: bool,
    ) -> bool {
        let s = srv!(self);
        unsafe {
            (s.entity_add_effect)(
                s.ctx,
                YogStr::from_str(uuid),
                YogStr::from_str(effect_id),
                duration_ticks,
                amplifier,
                show_particles,
            )
        }
    }

    fn entity_remove_effect(&self, uuid: &str, effect_id: &str) -> bool {
        let s = srv!(self);
        unsafe {
            (s.entity_remove_effect)(s.ctx, YogStr::from_str(uuid), YogStr::from_str(effect_id))
        }
    }

    fn entity_clear_effects(&self, uuid: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_clear_effects)(s.ctx, YogStr::from_str(uuid)) }
    }

    fn drop_loot(&self, table_id: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool {
        let s = srv!(self);
        unsafe {
            (s.drop_loot)(
                s.ctx,
                YogStr::from_str(table_id),
                YogStr::from_str(dimension),
                yog_abi::YogVec3 { x, y, z },
            )
        }
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
        if unsafe { (s.world_time)(s.ctx, YogStr::from_str(dimension), &mut t) } {
            Some(t)
        } else {
            None
        }
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
        let mut v = yog_abi::YogVec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        if unsafe { (s.entity_velocity)(s.ctx, YogStr::from_str(uuid), &mut v) } {
            Some((v.x, v.y, v.z))
        } else {
            None
        }
    }

    fn entity_set_velocity(&self, uuid: &str, vx: f64, vy: f64, vz: f64) -> bool {
        let s = srv!(self);
        unsafe {
            (s.entity_set_velocity)(
                s.ctx,
                YogStr::from_str(uuid),
                yog_abi::YogVec3 {
                    x: vx,
                    y: vy,
                    z: vz,
                },
            )
        }
    }

    fn entity_add_velocity(&self, uuid: &str, vx: f64, vy: f64, vz: f64) -> bool {
        let s = srv!(self);
        unsafe {
            (s.entity_add_velocity)(
                s.ctx,
                YogStr::from_str(uuid),
                yog_abi::YogVec3 {
                    x: vx,
                    y: vy,
                    z: vz,
                },
            )
        }
    }

    fn scoreboard_get(&self, objective: &str, player: &str) -> Option<i32> {
        let s = srv!(self);
        let mut score = 0i32;
        if unsafe {
            (s.scoreboard_get)(
                s.ctx,
                YogStr::from_str(objective),
                YogStr::from_str(player),
                &mut score,
            )
        } {
            Some(score)
        } else {
            None
        }
    }

    fn scoreboard_set(&self, objective: &str, player: &str, score: i32) -> bool {
        let s = srv!(self);
        unsafe {
            (s.scoreboard_set)(
                s.ctx,
                YogStr::from_str(objective),
                YogStr::from_str(player),
                score,
            )
        }
    }

    fn scoreboard_add(&self, objective: &str, player: &str, delta: i32) -> Option<i32> {
        let s = srv!(self);
        let mut new_score = 0i32;
        if unsafe {
            (s.scoreboard_add)(
                s.ctx,
                YogStr::from_str(objective),
                YogStr::from_str(player),
                delta,
                &mut new_score,
            )
        } {
            Some(new_score)
        } else {
            None
        }
    }

    fn game_dir(&self) -> String {
        let s = srv!(self);
        let owned = unsafe { (s.game_dir)(s.ctx) };
        take_owned(s.free_str, owned).unwrap_or_default()
    }

    fn world_dir(&self) -> String {
        let s = srv!(self);
        let owned = unsafe { (s.world_dir)(s.ctx) };
        take_owned(s.free_str, owned).unwrap_or_default()
    }

    fn play_sound(
        &self,
        dimension: &str,
        x: f64,
        y: f64,
        z: f64,
        sound_id: &str,
        volume: f32,
        pitch: f32,
    ) -> bool {
        let s = srv!(self);
        unsafe {
            (s.play_sound)(
                s.ctx,
                YogStr::from_str(dimension),
                yog_abi::YogVec3 { x, y, z },
                YogStr::from_str(sound_id),
                volume,
                pitch,
            )
        }
    }

    fn play_sound_to_player(&self, player: &str, sound_id: &str, volume: f32, pitch: f32) -> bool {
        let s = srv!(self);
        unsafe {
            (s.play_sound_player)(
                s.ctx,
                YogStr::from_str(player),
                YogStr::from_str(sound_id),
                volume,
                pitch,
            )
        }
    }

    fn send_title(
        &self,
        player: &str,
        title: &str,
        subtitle: &str,
        fadein: i32,
        stay: i32,
        fadeout: i32,
    ) -> bool {
        let s = srv!(self);
        unsafe {
            (s.send_title)(
                s.ctx,
                YogStr::from_str(player),
                YogStr::from_str(title),
                YogStr::from_str(subtitle),
                fadein,
                stay,
                fadeout,
            )
        }
    }

    fn entity_rotation(&self, uuid: &str) -> Option<(f32, f32)> {
        let s = srv!(self);
        let mut out = yog_abi::YogVec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let ok = unsafe { (s.entity_rotation)(s.ctx, YogStr::from_str(uuid), &mut out) };
        if ok {
            Some((out.x as f32, out.y as f32))
        } else {
            None
        }
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
        unsafe {
            (s.bossbar_create)(
                s.ctx,
                YogStr::from_str(id),
                YogStr::from_str(title),
                YogStr::from_str(color),
                YogStr::from_str(style),
            )
        }
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
        let Some(text) = take_owned(s.free_str, owned) else {
            return Vec::new();
        };
        if text.is_empty() {
            Vec::new()
        } else {
            text.lines().map(str::to_owned).collect()
        }
    }

    fn get_block_nbt(&self, dimension: &str, pos: yog_core::BlockPos) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe {
            (s.get_block_nbt)(
                s.ctx,
                YogStr::from_str(dimension),
                yog_abi::YogBlockPos {
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                },
            )
        };

        take_owned(s.free_str, owned)
    }

    fn set_block_nbt(&self, dimension: &str, pos: yog_core::BlockPos, snbt: &str) -> bool {
        let s = srv!(self);
        unsafe {
            (s.set_block_nbt)(
                s.ctx,
                YogStr::from_str(dimension),
                yog_abi::YogBlockPos {
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                },
                YogStr::from_str(snbt),
            )
        }
    }

    fn get_inventory_slot(
        &self,
        dimension: &str,
        pos: yog_core::BlockPos,
        slot: u32,
    ) -> Option<(String, u32, String)> {
        let s = srv!(self);
        let owned = unsafe {
            (s.get_inventory_slot)(
                s.ctx,
                YogStr::from_str(dimension),
                yog_abi::YogBlockPos {
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                },
                slot,
            )
        };
        let text = take_owned(s.free_str, owned)?;
        let mut it = text.splitn(3, '\t');
        let item_id = it.next()?.to_owned();
        let count = it.next()?.parse().ok()?;
        let snbt = it.next().unwrap_or("{}").to_owned();
        Some((item_id, count, snbt))
    }

    fn set_inventory_slot(
        &self,
        dimension: &str,
        pos: yog_core::BlockPos,
        slot: u32,
        item_id: &str,
        count: u32,
        snbt: &str,
    ) -> bool {
        // The ABI's item id carries the SNBT after a tab.
        let item = if snbt.is_empty() { item_id.to_string() } else { format!("{item_id}\t{snbt}") };
        let s = srv!(self);
        unsafe {
            (s.set_inventory_slot)(
                s.ctx,
                YogStr::from_str(dimension),
                yog_abi::YogBlockPos {
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                },
                slot,
                YogStr::from_str(&item),
                count,
            )
        }
    }

    fn player_inventory(&self, player: &str) -> Vec<(u32, String, u32)> {
        let s = srv!(self);
        let owned = unsafe { (s.player_inventory)(s.ctx, YogStr::from_str(player)) };
        let Some(text) = take_owned(s.free_str, owned) else {
            return Vec::new();
        };
        text.lines()
            .filter_map(|line| {
                let mut it = line.split('\t');
                let slot: u32 = it.next()?.parse().ok()?;
                let item_id = it.next()?.to_owned();
                let count: u32 = it.next()?.parse().ok()?;
                Some((slot, item_id, count))
            })
            .collect()
    }

    fn player_set_slot(&self, player: &str, slot: u32, item_id: &str, count: u32) -> bool {
        let s = srv!(self);
        unsafe {
            (s.player_set_slot)(
                s.ctx,
                YogStr::from_str(player),
                slot,
                YogStr::from_str(item_id),
                count,
            )
        }
    }

    fn teleport_to_dim(&self, player: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool {
        let s = srv!(self);
        unsafe {
            (s.player_teleport_dim)(
                s.ctx,
                YogStr::from_str(player),
                YogStr::from_str(dimension),
                yog_abi::YogVec3 { x, y, z },
            )
        }
    }

    fn entity_teleport_to_dim(&self, uuid: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool {
        let s = srv!(self);
        unsafe {
            (s.entity_teleport_dim)(
                s.ctx,
                YogStr::from_str(uuid),
                YogStr::from_str(dimension),
                yog_abi::YogVec3 { x, y, z },
            )
        }
    }

    fn world_entity_count(&self, dimension: &str, entity_type: &str) -> i32 {
        let s = srv!(self);
        unsafe {
            (s.world_entity_count)(
                s.ctx,
                YogStr::from_str(dimension),
                YogStr::from_str(entity_type),
            )
        }
    }

    fn entity_get_nbt(&self, uuid: &str) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe { (s.entity_get_nbt)(s.ctx, YogStr::from_str(uuid)) };

        take_owned(s.free_str, owned)
    }

    fn entity_set_nbt(&self, uuid: &str, snbt: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_set_nbt)(s.ctx, YogStr::from_str(uuid), YogStr::from_str(snbt)) }
    }

    fn spawn_particles(
        &self,
        dimension: &str,
        x: f64,
        y: f64,
        z: f64,
        particle_type: &str,
        count: i32,
        dx: f64,
        dy: f64,
        dz: f64,
        speed: f64,
    ) -> bool {
        let s = srv!(self);
        unsafe {
            (s.spawn_particles)(
                s.ctx,
                YogStr::from_str(dimension),
                yog_abi::YogVec3 { x, y, z },
                YogStr::from_str(particle_type),
                count,
                dx,
                dy,
                dz,
                speed,
            )
        }
    }

    fn entity_attribute_value(&self, uuid: &str, attribute_id: &str) -> Option<f64> {
        let s = srv!(self);
        let v = unsafe { (s.entity_attribute_value)(s.ctx, YogStr::from_str(uuid), YogStr::from_str(attribute_id)) };
        (!v.is_nan()).then_some(v)
    }

    fn entity_attribute_get(&self, uuid: &str, attribute_id: &str) -> Option<f64> {
        let s = srv!(self);
        let v = unsafe {
            (s.entity_attribute_get)(
                s.ctx,
                YogStr::from_str(uuid),
                YogStr::from_str(attribute_id),
            )
        };
        if v.is_nan() { None } else { Some(v) }
    }

    fn entity_attribute_set(&self, uuid: &str, attribute_id: &str, value: f64) -> bool {
        let s = srv!(self);
        unsafe {
            (s.entity_attribute_set)(
                s.ctx,
                YogStr::from_str(uuid),
                YogStr::from_str(attribute_id),
                value,
            )
        }
    }

    fn get_held_item_nbt(&self, player: &str) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe { (s.get_held_item_nbt)(s.ctx, YogStr::from_str(player)) };

        take_owned(s.free_str, owned)
    }

    fn set_held_item_nbt(&self, player: &str, snbt: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.set_held_item_nbt)(s.ctx, YogStr::from_str(player), YogStr::from_str(snbt)) }
    }

    fn get_offhand_item_nbt(&self, player: &str) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe { (s.get_offhand_item_nbt)(s.ctx, YogStr::from_str(player)) };

        take_owned(s.free_str, owned)
    }

    fn set_offhand_item_nbt(&self, player: &str, snbt: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.set_offhand_item_nbt)(s.ctx, YogStr::from_str(player), YogStr::from_str(snbt)) }
    }

    fn get_slot_item(&self, player: &str, slot: u32) -> Option<(String, u32, String)> {
        let s = srv!(self);
        let owned = unsafe { (s.get_slot_item)(s.ctx, YogStr::from_str(player), slot) };
        let text = take_owned(s.free_str, owned)?;
        let mut it = text.splitn(3, '\t');
        let item_id = it.next()?.to_owned();
        let count: u32 = it.next()?.parse().ok()?;
        let nbt = it.next().unwrap_or("{}").to_owned();
        Some((item_id, count, nbt))
    }

    fn set_slot_item(
        &self,
        player: &str,
        slot: u32,
        item_id: &str,
        count: u32,
        snbt: &str,
    ) -> bool {
        let s = srv!(self);
        unsafe {
            (s.set_slot_item)(
                s.ctx,
                YogStr::from_str(player),
                slot,
                YogStr::from_str(item_id),
                count,
                YogStr::from_str(snbt),
            )
        }
    }

    // ── world queries & effects (ABI minor 1) ────────────────────────────────

    fn entity_info(&self, uuid: &str) -> Option<yog_core::EntityInfo> {
        let s = srv!(self);
        let owned = unsafe { (s.entity_info)(s.ctx, YogStr::from_str(uuid)) };
        take_owned(s.free_str, owned).and_then(|l| yog_core::EntityInfo::decode(&l))
    }

    fn entities_in_box(
        &self,
        dimension: &str,
        min: (f64, f64, f64),
        max: (f64, f64, f64),
    ) -> Vec<String> {
        let s = srv!(self);
        let owned =
            unsafe { (s.entities_in_box)(s.ctx, YogStr::from_str(dimension), v3(min), v3(max)) };
        take_owned(s.free_str, owned)
            .map(|t| {
                t.lines()
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn raycast_block(
        &self,
        dimension: &str,
        from: (f64, f64, f64),
        to: (f64, f64, f64),
    ) -> Option<yog_core::BlockHit> {
        let s = srv!(self);
        let owned =
            unsafe { (s.raycast_block)(s.ctx, YogStr::from_str(dimension), v3(from), v3(to)) };
        take_owned(s.free_str, owned).and_then(|l| yog_core::BlockHit::decode(&l))
    }

    fn raycast_entity(
        &self,
        dimension: &str,
        from: (f64, f64, f64),
        to: (f64, f64, f64),
        exclude: &str,
    ) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe {
            (s.raycast_entity)(
                s.ctx,
                YogStr::from_str(dimension),
                v3(from),
                v3(to),
                YogStr::from_str(exclude),
            )
        };
        take_owned(s.free_str, owned).filter(|u| !u.is_empty())
    }

    fn block_info(&self, dimension: &str, pos: yog_core::BlockPos) -> Option<yog_core::BlockInfo> {
        let s = srv!(self);
        let owned = unsafe { (s.block_info)(s.ctx, YogStr::from_str(dimension), bp(pos)) };
        take_owned(s.free_str, owned).and_then(|l| yog_core::BlockInfo::decode(&l))
    }

    fn break_block(
        &self,
        dimension: &str,
        pos: yog_core::BlockPos,
        drop: bool,
        breaker: &str,
    ) -> bool {
        let s = srv!(self);
        unsafe {
            (s.break_block)(
                s.ctx,
                YogStr::from_str(dimension),
                bp(pos),
                drop,
                YogStr::from_str(breaker),
            )
        }
    }

    fn place_block_from_slot(
        &self,
        player: &str,
        slot: u32,
        dimension: &str,
        pos: yog_core::BlockPos,
    ) -> bool {
        let s = srv!(self);
        unsafe {
            (s.place_block_from_slot)(
                s.ctx,
                YogStr::from_str(player),
                slot,
                YogStr::from_str(dimension),
                bp(pos),
            )
        }
    }

    fn bonemeal(&self, dimension: &str, pos: yog_core::BlockPos) -> bool {
        let s = srv!(self);
        unsafe { (s.bonemeal)(s.ctx, YogStr::from_str(dimension), bp(pos)) }
    }

    fn explode(
        &self,
        dimension: &str,
        x: f64,
        y: f64,
        z: f64,
        power: f32,
        fire: bool,
        source: &str,
    ) -> bool {
        let s = srv!(self);
        unsafe {
            (s.explode)(
                s.ctx,
                YogStr::from_str(dimension),
                v3((x, y, z)),
                power,
                fire,
                YogStr::from_str(source),
            )
        }
    }

    fn tellraw(&self, player: &str, json: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.tellraw)(s.ctx, YogStr::from_str(player), YogStr::from_str(json)) }
    }

    fn entity_damage(&self, uuid: &str, amount: f32, damage_type: &str, attacker: &str) -> bool {
        let s = srv!(self);
        unsafe {
            (s.entity_damage)(
                s.ctx,
                YogStr::from_str(uuid),
                amount,
                YogStr::from_str(damage_type),
                YogStr::from_str(attacker),
            )
        }
    }

    fn entity_set_fire_ticks(&self, uuid: &str, ticks: i32) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_set_fire_ticks)(s.ctx, YogStr::from_str(uuid), ticks) }
    }

    fn player_add_xp(&self, player: &str, amount: i32) -> bool {
        let s = srv!(self);
        unsafe { (s.player_add_xp)(s.ctx, YogStr::from_str(player), amount) }
    }

    fn player_set_may_fly(&self, player: &str, may_fly: bool) -> bool {
        let s = srv!(self);
        unsafe { (s.player_set_may_fly)(s.ctx, YogStr::from_str(player), may_fly) }
    }

    fn player_info(&self, player: &str) -> Option<yog_core::PlayerInfo> {
        let s = srv!(self);
        let owned = unsafe { (s.player_info)(s.ctx, YogStr::from_str(player)) };
        take_owned(s.free_str, owned).and_then(|l| yog_core::PlayerInfo::decode(&l))
    }

    fn spawn_item(
        &self,
        dimension: &str,
        pos: (f64, f64, f64),
        vel: (f64, f64, f64),
        item_id: &str,
        count: u32,
        snbt: &str,
    ) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe {
            (s.spawn_item)(
                s.ctx,
                YogStr::from_str(dimension),
                v3(pos),
                v3(vel),
                YogStr::from_str(item_id),
                count,
                YogStr::from_str(snbt),
            )
        };
        take_owned(s.free_str, owned).filter(|u| !u.is_empty())
    }

    fn item_entity_stack(&self, uuid: &str) -> Option<(String, u32, String)> {
        let s = srv!(self);
        let owned = unsafe { (s.item_entity_stack)(s.ctx, YogStr::from_str(uuid)) };
        let text = take_owned(s.free_str, owned)?;
        let mut it = text.splitn(3, '\t');
        let item_id = it.next()?.to_owned();
        let count = it.next()?.parse().ok()?;
        let nbt = it.next().unwrap_or("{}").to_owned();
        Some((item_id, count, nbt))
    }

    fn item_entity_set_stack(&self, uuid: &str, item_id: &str, count: u32, snbt: &str) -> bool {
        let s = srv!(self);
        unsafe {
            (s.item_entity_set_stack)(
                s.ctx,
                YogStr::from_str(uuid),
                YogStr::from_str(item_id),
                count,
                YogStr::from_str(snbt),
            )
        }
    }

    fn actionbar_json(&self, player: &str, json: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.actionbar_json)(s.ctx, YogStr::from_str(player), YogStr::from_str(json)) }
    }

    fn use_item_on_block(
        &self,
        dimension: &str,
        pos: yog_core::BlockPos,
        face: yog_core::Face,
        item_id: &str,
        player: &str,
    ) -> bool {
        let s = srv!(self);
        unsafe {
            (s.use_item_on_block)(
                s.ctx,
                YogStr::from_str(dimension),
                bp(pos),
                face.index(),
                YogStr::from_str(item_id),
                YogStr::from_str(player),
            )
        }
    }

    fn pour_fluid(&self, dimension: &str, pos: yog_core::BlockPos, bucket_item_id: &str) -> bool {
        let s = srv!(self);
        unsafe {
            (s.pour_fluid)(
                s.ctx,
                YogStr::from_str(dimension),
                bp(pos),
                YogStr::from_str(bucket_item_id),
            )
        }
    }

    fn has_entity_tag(&self, entity_type: &str, tag_id: &str) -> bool {
        let s = srv!(self);
        unsafe {
            (s.has_entity_tag)(
                s.ctx,
                YogStr::from_str(entity_type),
                YogStr::from_str(tag_id),
            )
        }
    }

    fn item_block(&self, item_id: &str) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe { (s.item_block)(s.ctx, YogStr::from_str(item_id)) };
        take_owned(s.free_str, owned).filter(|b| !b.is_empty())
    }

    fn place_feature(&self, dimension: &str, pos: yog_core::BlockPos, feature_id: &str) -> bool {
        let s = srv!(self);
        unsafe {
            (s.place_feature)(
                s.ctx,
                YogStr::from_str(dimension),
                bp(pos),
                YogStr::from_str(feature_id),
            )
        }
    }

    fn set_item_cooldown(&self, player: &str, item_id: &str, ticks: u32) -> bool {
        let s = srv!(self);
        unsafe { (s.set_item_cooldown)(s.ctx, YogStr::from_str(player), YogStr::from_str(item_id), ticks.min(i32::MAX as u32) as i32) }
    }

    fn entity_vehicle(&self, uuid: &str) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe { (s.entity_vehicle)(s.ctx, YogStr::from_str(uuid)) };
        take_owned(s.free_str, owned).filter(|v| !v.is_empty())
    }

    fn entity_passengers(&self, uuid: &str) -> Vec<String> {
        let s = srv!(self);
        let owned = unsafe { (s.entity_passengers)(s.ctx, YogStr::from_str(uuid)) };
        take_owned(s.free_str, owned)
            .map(|t| t.lines().filter(|l| !l.is_empty()).map(str::to_string).collect())
            .unwrap_or_default()
    }

    fn entity_mount(&self, rider: &str, vehicle: &str, force: bool) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_mount)(s.ctx, YogStr::from_str(rider), YogStr::from_str(vehicle), force) }
    }

    fn entity_dismount(&self, uuid: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_dismount)(s.ctx, YogStr::from_str(uuid)) }
    }

    fn entity_data(&self, uuid: &str, key: &str) -> Option<Vec<u8>> {
        let s = srv!(self);
        let mut out = Vec::new();
        let found = unsafe {
            (s.entity_data_get)(s.ctx, YogStr::from_str(uuid), YogStr::from_str(key), &mut out as *mut Vec<u8> as *mut c_void, crate::body::collect)
        };
        found.then_some(out)
    }

    fn set_entity_data(&self, uuid: &str, key: &str, value: Option<&[u8]>, persist: bool) -> bool {
        let s = srv!(self);
        let flags = value.map_or(0, |_| 1) | if persist { 2 } else { 0 };
        let v = value.unwrap_or(&[]);
        unsafe { (s.entity_data_set)(s.ctx, YogStr::from_str(uuid), YogStr::from_str(key), v.as_ptr(), v.len() as u32, flags) }
    }

    fn entity_set_dimensions(&self, uuid: &str, size: Option<(f32, f32, f32)>) -> bool {
        let s = srv!(self);
        let (w, h, e) = size.unwrap_or((-1.0, 0.0, 0.0));
        unsafe { (s.entity_set_dimensions)(s.ctx, YogStr::from_str(uuid), w, h, e) }
    }

    fn entity_set_fall_distance(&self, uuid: &str, distance: f32) -> bool {
        let s = srv!(self);
        unsafe { (s.entity_set_fall_distance)(s.ctx, YogStr::from_str(uuid), distance) }
    }

    fn redstone_signal(&self, dimension: &str, pos: yog_core::BlockPos) -> Option<u8> {
        let s = srv!(self);
        let p = unsafe { (s.redstone_signal)(s.ctx, YogStr::from_str(dimension), bp(pos)) };
        (0..=15).contains(&p).then_some(p as u8)
    }

    fn redstone_power(&self, dimension: &str, pos: yog_core::BlockPos) -> Option<u8> {
        let s = srv!(self);
        let p = unsafe { (s.redstone_power)(s.ctx, YogStr::from_str(dimension), bp(pos)) };
        (0..=15).contains(&p).then_some(p as u8)
    }

    fn set_redstone_output(&self, dimension: &str, pos: yog_core::BlockPos, level: u8) -> bool {
        let s = srv!(self);
        unsafe { (s.set_redstone_output)(s.ctx, YogStr::from_str(dimension), bp(pos), level.min(15) as i32) }
    }

    fn get_block_state(&self, dimension: &str, pos: yog_core::BlockPos) -> Option<String> {
        let s = srv!(self);
        let owned = unsafe { (s.get_block_state)(s.ctx, YogStr::from_str(dimension), bp(pos)) };
        take_owned(s.free_str, owned).filter(|b| !b.is_empty())
    }

    fn set_block_state(&self, dimension: &str, pos: yog_core::BlockPos, state: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.set_block_state)(s.ctx, YogStr::from_str(dimension), bp(pos), YogStr::from_str(state)) }
    }

    fn world_is_thundering(&self, dimension: &str) -> bool {
        let s = srv!(self);
        unsafe { (s.world_is_thundering)(s.ctx, YogStr::from_str(dimension)) }
    }

    fn world_set_thunder(&self, dimension: &str, thundering: bool, duration_ticks: i32) -> bool {
        let s = srv!(self);
        unsafe { (s.world_set_thunder)(s.ctx, YogStr::from_str(dimension), thundering, duration_ticks) }
    }

    fn fill_region(&self, dimension: &str, from: yog_core::BlockPos, to: yog_core::BlockPos, state: &str) -> Option<u32> {
        let s = srv!(self);
        let n = unsafe { (s.fill_region)(s.ctx, YogStr::from_str(dimension), bp(from), bp(to), YogStr::from_str(state)) };
        u32::try_from(n).ok()
    }

    fn scan_region(&self, dimension: &str, from: yog_core::BlockPos, to: yog_core::BlockPos) -> Vec<(yog_core::BlockPos, String)> {
        let s = srv!(self);
        let owned = unsafe { (s.scan_region)(s.ctx, YogStr::from_str(dimension), bp(from), bp(to)) };
        let Some(text) = take_owned(s.free_str, owned) else {
            return Vec::new();
        };
        text.lines()
            .filter_map(|line| {
                let mut it = line.splitn(4, ' ');
                let (x, y, z) = (it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?);
                Some((yog_core::BlockPos { x, y, z }, it.next()?.to_string()))
            })
            .collect()
    }
}

/// Copy a runtime-owned string out and free it; `None` for NONE.
pub(crate) fn take_owned(
    free_str: unsafe extern "C" fn(ptr: *mut u8, len: u32),
    owned: yog_abi::YogOwnedStr,
) -> Option<String> {
    if owned.is_none() {
        return None;
    }
    // SAFETY: a non-NONE owned string is `len` valid bytes until freed.
    let text = unsafe {
        String::from_utf8_lossy(std::slice::from_raw_parts(owned.ptr, owned.len as usize))
            .into_owned()
    };
    unsafe { free_str(owned.ptr, owned.len) };
    Some(text)
}

fn v3((x, y, z): (f64, f64, f64)) -> yog_abi::YogVec3 {
    yog_abi::YogVec3 { x, y, z }
}

fn bp(pos: yog_core::BlockPos) -> yog_abi::YogBlockPos {
    yog_abi::YogBlockPos {
        x: pos.x,
        y: pos.y,
        z: pos.z,
    }
}

// ── Trampoline helpers ────────────────────────────────────────────────────────
//
// A panic must never unwind out of a trampoline: the runtime calls them
// through `extern "C"`, so it would abort the whole game. `guard` stops it
// at the boundary — the panic message is already in the log — and the
// event goes on as if the mod hadn't handled it.

pub(crate) fn guard<R>(fallback: R, call: impl FnOnce() -> R) -> R {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)).unwrap_or_else(|_| {
        crate::error!("a mod's callback panicked (see above); carrying on without it");
        fallback
    })
}
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
    ($name:ident, $abi_ev:ty, $rust_ev:ty, |$ev:ident| $build:expr_2021) => {
        unsafe extern "C" fn $name<F>(
            ud: *mut c_void,
            srv: *const YogServer,
            ev: *const $abi_ev,
            phase: u8,
        ) -> bool
        where
            F: Fn(&$rust_ev, EventPhase, &dyn Server) -> bool + Send + Sync,
        {
            unsafe {
                let f = &*(ud as *const F);
                let $ev = &*ev;
                let rust_ev = $build;
                let p = if phase == 0 {
                    EventPhase::Pre
                } else {
                    EventPhase::Post
                };
                guard(true, || f(&rust_ev, p, &CServer(srv)))
            }
        }
    };
}

/// A machine — an inventory whose `.yui` has a `<RecipeArea kind="…">` —
/// making a recipe of its kind, like a crafting table: the result shows as
/// soon as its slots hold a recipe's ingredients, and taking it takes them.
///
/// - `Pre` — whether it may: `false` and the result doesn't show (a
///   workbench short of what the recipe's `values` cost). `player` is empty.
/// - `Post` — a player took the result; the host takes the ingredients, the mod
///   its share (the workbench's resources).
#[derive(Debug, Clone)]
pub struct MachineCraftEvent {
    /// The machine's inventory, e.g. `"mymod:press"`.
    pub machine: String,
    /// The recipe's kind (`"mymod:pressing"`) and id.
    pub kind: String,
    pub recipe: String,
    pub result_item: String,
    pub result_count: u32,
    /// The recipe's values: `{ "energy": 4 }`.
    pub values: yog_data::Value,
    pub pos: yog_core::BlockPos,
    pub dimension: String,
    pub player: String,
}

trampoline_phased!(trampoline_machine_craft, YogMachineCraftEvent, MachineCraftEvent, |ev| {
    MachineCraftEvent {
        machine: ev.machine.as_str().to_owned(),
        kind: ev.kind.as_str().to_owned(),
        recipe: ev.recipe.as_str().to_owned(),
        result_item: ev.result_item.as_str().to_owned(),
        result_count: ev.result_count,
        values: yog_data::json::from_json_text(ev.values.as_str()).unwrap_or(yog_data::Value::Null),
        pos: yog_core::BlockPos { x: ev.pos.x, y: ev.pos.y, z: ev.pos.z },
        dimension: ev.dimension.as_str().to_owned(),
        player: ev.player.as_str().to_owned(),
    }
});

trampoline_phased!(
    trampoline_item_insert,
    YogItemInsertEvent,
    ItemInsertEvent,
    |ev| ItemInsertEvent {
        block_id: ev.block.as_str().to_owned(),
        pos: yog_core::BlockPos {
            x: ev.pos.x,
            y: ev.pos.y,
            z: ev.pos.z
        },
        dimension: ev.dimension.as_str().to_owned(),
        item_id: ev.item_id.as_str().to_owned(),
        count: ev.count,
        nbt: ev.nbt.as_str().to_owned(),
    }
);

unsafe extern "C" fn trampoline_packet<F>(
    ud: *mut c_void,
    srv: *const YogServer,
    ev: *const YogPacketEvent,
) where
    F: Fn(&PacketEvent, &dyn Server) + Send + Sync,
{
    unsafe {
        let f = &*(ud as *const F);
        let ev = &*ev;
        let rust_ev = PacketEvent {
            channel: ev.channel.as_str().to_owned(),
            player: ev.player.as_str().to_owned(),
            // (A null pointer with no bytes is fine for C, not for a slice.)
            payload: if ev.payload.is_null() || ev.payload_len == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(ev.payload, ev.payload_len as usize).to_vec()
            },
        };
        guard((), || f(&rust_ev, &CServer(srv)));
    }
}

unsafe extern "C" fn trampoline_command<F>(
    ud: *mut c_void,
    srv: *const YogServer,
    ev: *const YogCommandEvent,
    reply_buf: *mut u8,
    reply_cap: u32,
    reply_len: *mut u32,
) where
    F: Fn(&CommandContext, &dyn Server) -> Option<String> + Send + Sync,
{
    unsafe {
        let f = &*(ud as *const F);
        let ev = &*ev;
        let ctx = CommandContext {
            name: ev.name.as_str().to_owned(),
            args: ev.args.as_str().to_owned(),
            source: ev.source.as_str().to_owned(),
            uuid: ev.uuid.as_str().to_owned(),
            dimension: ev.dimension.as_str().to_owned(),
        };
        *reply_len = 0;
        if let Some(reply) = guard(None, || f(&ctx, &CServer(srv))) {
            let bytes = reply.as_bytes();
            // As much as fits, cut between characters (it's read as UTF-8).
            let mut n = bytes.len().min(reply_cap as usize);
            while !reply.is_char_boundary(n) {
                n -= 1;
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), reply_buf, n);
            *reply_len = n as u32;
        }
    }
}

unsafe extern "C" fn trampoline_scheduled<F>(ud: *mut c_void, srv: *const YogServer)
where
    F: Fn(&dyn Server) + Send + Sync,
{
    unsafe {
        let f = &*(ud as *const F);
        guard((), || f(&CServer(srv)));
    }
}

// ── ABI minor 10 — client-side trampolines ────────────────────────────────────

unsafe extern "C" fn trampoline_hud_render<F>(ud: *mut c_void, gfx: *const YogGfxApi)
where
    F: Fn(&GfxContext) + Send + Sync,
{
    unsafe {
        let f = &*(ud as *const F);
        guard((), || f(&GfxContext::from_raw(gfx)));
    }
}

/// What `Registry::on_item_tooltip` keeps: the size and the draw closures.
struct TooltipPicture<S, D> {
    size: S,
    draw: D,
}

unsafe extern "C" fn trampoline_tooltip_size<S, D>(ud: *mut c_void, item: YogStr, snbt: YogStr, w: *mut f32, h: *mut f32) -> bool
where
    S: Fn(&str, &str) -> Option<(f32, f32)> + Send + Sync,
{
    unsafe {
        let t = &*(ud as *const TooltipPicture<S, D>);
        match guard(None, || (t.size)(item.as_str(), snbt.as_str())) {
            Some((pw, ph)) => {
                (*w, *h) = (pw, ph);
                true
            }
            None => false,
        }
    }
}

unsafe extern "C" fn trampoline_item_text<F>(ud: *mut c_void, item: YogStr, snbt: YogStr, sink: *mut c_void, write: yog_abi::YogTextSink) -> bool
where
    F: Fn(&str, &str) -> Option<(Option<String>, Vec<String>)> + Send + Sync,
{
    unsafe {
        let f = &*(ud as *const F);
        let Some((name, lines)) = guard(None, || f(item.as_str(), snbt.as_str())) else { return false };
        let mut text = name.unwrap_or_default();
        for l in lines {
            text.push('\n');
            text.push_str(&l.replace('\n', " "));
        }
        write(sink, YogStr::from_str(&text));
        true
    }
}

unsafe extern "C" fn trampoline_entity_tick<F>(ud: *mut c_void, srv: *const yog_abi::YogServer, uuid: YogStr)
where
    F: Fn(&dyn Server, &str) + Send + Sync,
{
    unsafe {
        let f = &*(ud as *const F);
        let s = CServer(srv);
        guard((), || f(&s, uuid.as_str()));
    }
}

unsafe extern "C" fn trampoline_loot<F>(
    ud: *mut c_void,
    srv: *const yog_abi::YogServer,
    loot: *const u8,
    len: usize,
    sink: *mut c_void,
    write: yog_abi::YogBytesSink,
) where
    F: Fn(&dyn Server, &mut crate::loot::Loot) + Send + Sync,
{
    use yog_data::Schema;
    unsafe {
        let f = &*(ud as *const F);
        let Ok(mut loot) = yog_data::read_bytes::<crate::loot::Loot>(std::slice::from_raw_parts(loot, len)) else { return };
        let before = loot.items.clone();
        let s = CServer(srv);
        guard((), || f(&s, &mut loot));
        if loot.items != before {
            let bytes = yog_data::binary::encode(&loot.items.write());
            write(sink, bytes.as_ptr(), bytes.len());
        }
    }
}

unsafe extern "C" fn trampoline_body<F>(ud: *mut c_void, frame: *const u8, len: usize, sink: *mut c_void, write: yog_abi::YogBytesSink)
where
    F: Fn(&yog_body::Frame, &mut yog_body::Ops) + Send + Sync,
{
    unsafe {
        let f = &*(ud as *const F);
        let Some(frame) = yog_body::Frame::decode(std::slice::from_raw_parts(frame, len)) else { return };
        let mut ops = yog_body::Ops::default();
        guard((), || f(&frame, &mut ops));
        if !ops.is_empty() {
            write(sink, ops.0.as_ptr(), ops.0.len());
        }
    }
}

unsafe extern "C" fn trampoline_camera<F>(ud: *mut c_void, frame: *const yog_abi::YogCameraFrame, out: *mut yog_abi::YogCameraOut)
where
    F: Fn(&yog_body::CameraFrame, &mut yog_body::CameraOut) + Send + Sync,
{
    unsafe {
        let f = &*(ud as *const F);
        let c = &*frame;
        let frame = yog_body::CameraFrame {
            uuid: c.uuid.as_str().to_owned(),
            partial: c.partial,
            pos: c.pos,
            yaw: c.yaw,
            pitch: c.pitch,
            detached: c.detached,
            flags: c.flags,
        };
        let o = &mut *out;
        let mut cam = yog_body::CameraOut {
            local_offset: yog_body::Vec3::from_array(o.local_offset),
            world_offset: yog_body::Vec3::from_array(o.world_offset),
            yaw: o.yaw,
            pitch: o.pitch,
            roll: o.roll,
            fov: o.fov,
        };
        guard((), || f(&frame, &mut cam));
        *o = yog_abi::YogCameraOut {
            local_offset: cam.local_offset.to_array(),
            world_offset: cam.world_offset.to_array(),
            yaw: cam.yaw,
            pitch: cam.pitch,
            roll: cam.roll,
            fov: cam.fov,
        };
    }
}

unsafe extern "C" fn trampoline_travel<F>(ud: *mut c_void, frame: *const yog_abi::YogTravelFrame, velocity: *mut [f32; 3]) -> bool
where
    F: Fn(&yog_body::TravelFrame) -> Option<yog_body::Vec3> + Send + Sync,
{
    unsafe {
        let f = &*(ud as *const F);
        let t = &*frame;
        let frame = yog_body::TravelFrame {
            uuid: t.uuid.as_str().to_owned(),
            velocity: yog_body::Vec3::from_array(t.velocity),
            look: yog_body::Vec3::from_array(t.look),
            yaw: t.yaw,
            pitch: t.pitch,
            forward: t.forward,
            strafe: t.strafe,
            jumping: t.jumping,
            sneaking: t.sneaking,
            gravity: t.gravity,
            flags: t.flags,
        };
        match guard(None, || f(&frame)) {
            Some(v) => {
                *velocity = v.to_array();
                true
            }
            None => false,
        }
    }
}

#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn trampoline_tooltip_draw<S, D>(ud: *mut c_void, gfx: *const YogGfxApi, item: YogStr, snbt: YogStr, x: f32, y: f32, w: f32, h: f32)
where
    D: Fn(&GfxContext, &str, &str, [f32; 4]) + Send + Sync,
{
    unsafe {
        let t = &*(ud as *const TooltipPicture<S, D>);
        guard((), || (t.draw)(&GfxContext::from_raw(gfx), item.as_str(), snbt.as_str(), [x, y, w, h]));
    }
}

#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn trampoline_book_page<F>(ud: *mut c_void, gfx: *const YogGfxApi, data: *const u8, len: usize, x: f32, y: f32, w: f32, h: f32)
where
    F: Fn(&GfxContext, &yog_data::Value, [f32; 4]) + Send + Sync,
{
    unsafe {
        let f = &*(ud as *const F);
        let bytes = if data.is_null() { &[][..] } else { std::slice::from_raw_parts(data, len) };
        let value = yog_data::binary::decode(bytes).unwrap_or(yog_data::Value::Null);
        guard((), || f(&GfxContext::from_raw(gfx), &value, [x, y, w, h]));
    }
}

unsafe extern "C" fn trampoline_world_render<F>(ud: *mut c_void, gfx: *const YogGfxApi)
where
    F: Fn(&GfxContext) + Send + Sync,
{
    unsafe {
        let f = &*(ud as *const F);
        guard((), || f(&GfxContext::from_raw(gfx)));
    }
}

unsafe extern "C" fn trampoline_key_binding<F>(ud: *mut c_void, id: YogStr)
where
    F: Fn(&str) + Send + Sync,
{
    unsafe {
        let f = &*(ud as *const F);
        let id = id.as_str();
        guard((), || f(id));
    }
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

/// The runtime's `YogApi` table, null before this mod registered.
pub(crate) fn global_api() -> *mut YogApi {
    GLOBAL_API.load(std::sync::atomic::Ordering::Acquire)
}

/// Metadata of an installed mod, as reported by the loader.
#[derive(Debug, Clone)]
pub struct ModInfo {
    /// `"yog"` for .yog mods, `"platform"` for loader (Java) mods.
    pub source: String,
    pub id: String,
    pub name: String,
    pub version: String,
    /// Comma-separated author list (may be empty).
    pub authors: String,
    pub description: String,
}

/// All installed mods known to the loader: .yog mods plus, where the host
/// exposes them, platform (Java) mods. Callable at any time after this mod's
/// `register()` ran — including client-side UI handlers. Note that during
/// `register()` mods that load after this one are not in the list yet; query
/// lazily (e.g. on first render) for a complete view.
pub fn installed_mods() -> Vec<ModInfo> {
    let api = GLOBAL_API.load(std::sync::atomic::Ordering::Acquire);
    if api.is_null() {
        return Vec::new();
    }
    let owned = unsafe { ((*api).mods_list)((*api).ctx) };
    let Some(text) = take_owned(unsafe { (*api).free_str }, owned) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            Some(ModInfo {
                source: f.next()?.to_string(),
                id: f.next()?.to_string(),
                name: f.next().unwrap_or_default().to_string(),
                version: f.next().unwrap_or_default().to_string(),
                authors: f.next().unwrap_or_default().to_string(),
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
    if api.is_null() {
        return;
    }
    unsafe { ((*api).ui_open)((*api).ctx, YogStr::from_str(ui_id), modal, pause) }
}

/// Close the Yog UI screen showing `ui_id` — e.g. from that screen's own
/// "Done" button. Does nothing when another screen is open. Deferred onto the
/// render thread, so the handler that calls it finishes first. Client side;
/// a no-op on dedicated servers.
pub fn close_ui(ui_id: &str) {
    let api = GLOBAL_API.load(std::sync::atomic::Ordering::Acquire);
    if api.is_null() {
        return;
    }
    unsafe { ((*api).ui_close)((*api).ctx, YogStr::from_str(ui_id)) }
}

/// What a [`Registry::register_ui`] handler returns: `()` (never marks
/// events as used) or `bool` (`true` = the handler used this `key:`/`char:`
/// event — the host then doesn't, e.g. Esc doesn't close the screen).
pub trait UiHandled {
    fn handled(self) -> bool;
}

impl UiHandled for () {
    fn handled(self) -> bool {
        false
    }
}

impl UiHandled for bool {
    fn handled(self) -> bool {
        self
    }
}

/// Handle to the runtime's server-action table, usable from any handler after
/// registration — including client-side ones that don't receive `&dyn Server`
/// (UI event handlers, client tick). Server-world actions are no-ops client
/// side, but networking (`send_to_server`) and similar client-safe calls work.
/// The server's table, if there's a server.
pub(crate) fn server_table() -> Option<&'static yog_abi::YogServer> {
    let api = GLOBAL_API.load(std::sync::atomic::Ordering::Acquire);
    if api.is_null() {
        return None;
    }
    let srv = unsafe { (*api).server };
    (!srv.is_null()).then(|| unsafe { &*srv })
}

pub fn server() -> Option<CServer> {
    let api = GLOBAL_API.load(std::sync::atomic::Ordering::Acquire);
    if api.is_null() {
        return None;
    }
    let srv = unsafe { (*api).server };
    if srv.is_null() {
        return None;
    }
    Some(CServer(srv))
}

pub struct Registry {
    api: *const YogApi,
}

// SAFETY: `api` is a static provided by the runtime, valid for process lifetime.
unsafe impl Send for Registry {}
unsafe impl Sync for Registry {}

/// A record of this mod's (`yog_api::info!`…) handed to the runtime's log.
fn log_to_host(level: yog_logging::Level, message: &str) {
    let api = GLOBAL_API.load(std::sync::atomic::Ordering::Acquire);
    if api.is_null() {
        return;
    }
    unsafe { ((*api).log)(level as u32, YogStr::from_str(message)) }
}

impl Registry {
    /// Build from the pointer passed by the runtime. Only called by `export_mod!`.
    ///
    /// # Safety
    ///
    /// `api` must point to the runtime's `YogApi` table, valid for the rest of
    /// the process.
    pub unsafe fn from_raw(api: *const YogApi) -> Self {
        GLOBAL_API.store(api as *mut YogApi, std::sync::atomic::Ordering::Release);
        // The mod's records go to the game's log, through the runtime.
        yog_logging::set_sink(log_to_host);
        Self { api }
    }

    /// The runtime's table.
    pub(crate) fn api(&self) -> &YogApi {
        unsafe { &*self.api }
    }

    #[inline]
    fn ctx(&self) -> *mut c_void {
        unsafe { (*self.api).ctx }
    }

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

    /// A machine making a recipe — see [`MachineCraftEvent`].
    pub fn on_machine_craft<F>(&mut self, handler: F)
    where
        F: Fn(&MachineCraftEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_machine_craft)(self.ctx(), ud, trampoline_machine_craft::<F>) }
    }

    /// A hopper or dropper offers an item to a block that accepts items
    /// (`block.accepts_items`): `Pre` returns whether it takes it, `Post`
    /// takes it.
    pub fn on_item_insert<F>(&mut self, handler: F)
    where
        F: Fn(&ItemInsertEvent, EventPhase, &dyn Server) -> bool + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_item_insert)(self.ctx(), ud, trampoline_item_insert::<F>) }
    }

    // ── commands ─────────────────────────────────────────────────────────────

    pub fn on_command<F>(&mut self, name: impl AsRef<str>, handler: F)
    where
        F: Fn(&CommandContext, &dyn Server) -> Option<String> + Send + Sync + 'static,
    {
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
    pub fn on_typed_command<F>(
        &mut self,
        name: impl AsRef<str>,
        schema: impl AsRef<str>,
        handler: F,
    ) where
        F: Fn(&CommandContext, &dyn Server) -> Option<String> + Send + Sync + 'static,
    {
        let name_ys = YogStr::from_str(name.as_ref());
        let schema_ys = YogStr::from_str(schema.as_ref());
        let ud = Self::leak(handler);
        unsafe {
            ((*self.api).register_typed_command)(
                self.ctx(),
                name_ys,
                schema_ys,
                ud,
                trampoline_command::<F>,
            );
        }
    }

    // ── networking ───────────────────────────────────────────────────────────

    pub fn on_packet<F>(&mut self, channel: impl AsRef<str>, handler: F)
    where
        F: Fn(&PacketEvent, &dyn Server) + Send + Sync + 'static,
    {
        let ch = YogStr::from_str(channel.as_ref());
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_packet)(self.ctx(), ch, ud, trampoline_packet::<F>) }
    }

    pub fn on_client_packet<F>(&mut self, channel: impl AsRef<str>, handler: F)
    where
        F: Fn(&PacketEvent, &dyn Server) + Send + Sync + 'static,
    {
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

    // ── content ──────────────────────────────────────────────────────────────

    /// Hands the runtime content of `kind` named `id`: its definition as a
    /// Yog Data blob (`register_content`, one ABI call for every kind).
    fn content(&mut self, kind: &str, id: &str, def: &yog_data::Value) {
        let blob = yog_data::binary::encode(def);
        unsafe { ((*self.api).register_content)(self.ctx(), YogStr::from_str(kind), YogStr::from_str(id), blob.as_ptr(), blob.len()) }
    }

    /// JSON text as Yog Data (recipes and books are written as JSON by their crates).
    fn json(text: &str) -> yog_data::Value {
        yog_data::json::from_json_text(text).unwrap_or_default()
    }

    /// Register a recipe (Yog's format — see [`Recipe`]; the host writes
    /// its Minecraft version's JSON).
    pub fn add_recipe(&mut self, recipe: Recipe) {
        let (ns, name) = recipe.ns_name();
        let id = format!("{ns}:{name}");
        self.content(yog_ids::kind::RECIPE, &id, &recipe.to_data());
    }

    /// Register a recipe given as Yog's recipe JSON — how `yog build`
    /// registers the mod's own kinds (`{"type": "mymod:infusion", "slots":
    /// …}`), which stay the mod's data: for its machines and its books.
    pub fn add_recipe_json(&mut self, id: &str, json: &str) {
        let id = yog_ids::qualify(id);
        self.content(yog_ids::kind::RECIPE, &id, &Self::json(json));
    }

    pub fn register_item(&mut self, def: ItemDef) {
        use yog_data::Schema;
        self.content(yog_ids::kind::ITEM, &def.id.clone(), &def.write());
    }

    pub fn register_block(&mut self, def: BlockDef) {
        use yog_data::Schema;
        self.content(yog_ids::kind::BLOCK, &def.id.clone(), &def.write());
    }

    /// Registers an entity type (what a `.yentity` makes). With a `model`,
    /// the loader draws it — its body model with the clip of its state
    /// (`clips`: `idle`, `walk`, or entity data `yog:state`).
    pub fn register_entity(&mut self, def: yog_registry::EntityDef) {
        use yog_data::Schema;
        if crate::entities::drawn(&def) {
            self.on_body(crate::entities::draw);
        }
        self.content(yog_ids::kind::ENTITY, &def.id.clone(), &def.write());
    }

    /// A compiled `.yentity` (the generated code calls this).
    pub fn register_entity_data(&mut self, id: &str, data: &[u8]) {
        match yog_data::read_bytes::<yog_registry::EntityDef>(data) {
            Ok(def) => self.register_entity(def),
            Err(e) => crate::error!("entity {id}: {e:?}"),
        }
    }

    /// Calls `handler` each tick of every entity of `entity_type` (a mod's
    /// type with `ticks`), with its uuid — server side.
    pub fn on_entity_tick<F>(&mut self, entity_type: &str, handler: F)
    where
        F: Fn(&dyn Server, &str) + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_entity_tick)(self.ctx(), YogStr::from_str(entity_type), ud, trampoline_entity_tick::<F>) }
    }

    /// Add placed feature `feature` (the mod's datapack's, like an ore's
    /// `mymod:mythril_ore`) to the biomes of tag `biomes` at generation
    /// `step` (`underground_ores`). An ore `.ydef` does this for you.
    pub fn add_biome_feature(&mut self, feature: &str, biomes: &str, step: &str) {
        use yog_data::Schema;
        let f = yog_registry::BiomeFeature { feature: feature.into(), biomes: biomes.into(), step: step.into() };
        self.content(yog_ids::kind::BIOME_FEATURE, feature, &f.write());
    }

    /// Register a tool material — what tool items naming it
    /// (`ItemDef::tool`) are made of. The game's are built in.
    pub fn register_tool_material(&mut self, material: ToolMaterial) {
        use yog_data::Schema;
        self.content(yog_ids::kind::TOOL_MATERIAL, &material.id.clone(), &material.write());
    }

    /// Register an armor material — what armor items naming it
    /// (`ItemDef::armor`) are made of. The game's are built in.
    pub fn register_armor_material(&mut self, material: ArmorMaterial) {
        use yog_data::Schema;
        self.content(yog_ids::kind::ARMOR_MATERIAL, &material.id.clone(), &material.write());
    }

    /// Register a sound group — what blocks naming it (`BlockDef::sound`)
    /// sound like. The game's groups are built in.
    pub fn register_sound_group(&mut self, group: SoundGroup) {
        use yog_data::Schema;
        self.content(yog_ids::kind::SOUND_GROUP, &group.id.clone(), &group.write());
    }

    /// Register a real Container/Menu inventory screen — see
    /// `yog_inventory::InventoryDef`. Attach it to a block via
    /// `BlockDef::inventory(id)`.
    pub fn register_inventory(&mut self, def: yog_inventory::InventoryDef) {
        use yog_data::Schema;
        self.content(yog_ids::kind::INVENTORY, &def.id.clone(), &def.write());
    }

    // ── startup grants ───────────────────────────────────────────────────────

    /// Register a startup grant: items/books to give once when a player first joins.
    pub fn register_startup_grant(&mut self, grant: StartupGrant) {
        use yog_data::Schema;
        self.content(yog_ids::kind::STARTUP_GRANT, &grant.id.clone(), &grant.write());
    }

    /// Register a book.
    pub fn register_book(&mut self, book: &Book) {
        use yog_data::Schema;
        self.content(yog_ids::kind::BOOK, &book.id, &book.write());
    }

    /// Register a book given as Yog Data, binary — what `yog build` makes
    /// of a `content/**/*.ybook`; the bytes go to the runtime as they are.
    pub fn register_book_data(&mut self, id: &str, data: &[u8]) {
        unsafe { ((*self.api).register_content)(self.ctx(), YogStr::from_str(yog_ids::kind::BOOK), YogStr::from_str(id), data.as_ptr(), data.len()) }
    }

    /// Register a font given as a compiled `.yfont` Yog Data blob.
    pub fn register_font_data(&mut self, id: &str, data: &[u8]) {
        unsafe { ((*self.api).register_content)(self.ctx(), YogStr::from_str("font"), YogStr::from_str(id), data.as_ptr(), data.len()) }
    }

    /// Register (or patch) a dimension's *type* (sky, lighting, physics, ...).
    ///
    /// `def.id` need not be a new custom id — pass a vanilla id
    /// (`"minecraft:overworld"`, `"minecraft:the_nether"`,
    /// `"minecraft:the_end"`) to patch that dimension's type/generator
    /// instead of creating a new one (e.g. to replace the overworld's
    /// terrain with your own generator type).
    ///
    /// Must be called from `register()` — the same mod-init window as
    /// blocks/items — so the host can declare/patch the dimension before
    /// Minecraft's registries freeze. See the `yog-dimensions` crate docs;
    /// set `def.generator_type` (via [`YogDimensionDef::generator_type`]) to
    /// pick a generator type registered with
    /// [`Registry::register_chunk_generator`].
    pub fn register_dimension(&mut self, def: &YogDimensionDef) {
        let value = Self::json(&def.to_json());
        self.content(yog_ids::kind::DIMENSION, &def.id.clone(), &value);
    }

    /// Register a dimension given as Yog Data, binary — what `yog build`
    /// makes of a `.ydef` of kind `dimension` (its `terrain` generated by
    /// the runtime itself); the bytes go to the runtime as they are.
    pub fn register_dimension_data(&mut self, id: &str, data: &[u8]) {
        unsafe { ((*self.api).register_content)(self.ctx(), YogStr::from_str(yog_ids::kind::DIMENSION), YogStr::from_str(id), data.as_ptr(), data.len()) }
    }

    /// Register a named, reusable chunk-generator type under
    /// `generator_type_id` (e.g. `"mymod:custom_terrain"`) — analogous to
    /// how Biomes O'Plenty registers selectable world-type presets. Any
    /// dimension (custom or vanilla) can opt into this generator via
    /// [`YogDimensionDef::generator_type`]; the platform host may also list
    /// registered generator type ids as options on the world-creation screen.
    ///
    /// `generator` is called once per chunk column that needs generating in
    /// any dimension using this type — write whatever terrain logic you want
    /// using the [`ChunkWriter`] handle (combine as many noise layers as you
    /// like; this crate doesn't prescribe a generation style).
    pub fn register_chunk_generator<F>(&mut self, generator_type_id: &str, generator: F)
    where
        F: Fn(&ChunkWriter) + Send + Sync + 'static,
    {
        let ud = Self::leak(generator);
        let id = YogStr::from_str(generator_type_id);
        unsafe {
            ((*self.api).register_chunk_generator)(
                self.ctx(),
                id,
                ud,
                trampoline_chunk_generator::<F>,
            );
        }
    }

    /// Register a menu entry — the host renders a button on vanilla screens
    /// (TitleScreen on Fabric, ModListScreen on Forge/NeoForge) that opens `ui_id`.
    /// `label` is the human-readable button text (e.g. "Yog Mods").
    /// `ui_id` is the UI to open when clicked (e.g. "example:menu").
    /// See [`installed_mods`]. During `register()` the list only contains
    /// mods loaded before this one.
    pub fn installed_mods(&self) -> Vec<ModInfo> {
        installed_mods()
    }

    /// Access the inter-mod communication layer.
    ///
    /// Allows exporting function pointers from this mod and importing
    /// function pointers from other mods.
    pub fn interop(&self) -> crate::Interop {
        crate::Interop::new(self.api)
    }

    /// Raw `YogApi` pointer — for passing to interop-imported functions
    /// that register content through the C ABI table directly.
    pub fn raw_api(&self) -> *const yog_abi::YogApi {
        self.api
    }

    pub fn register_menu_entry(&mut self, label: &str, ui_id: &str) {
        let l = YogStr::from_str(label);
        let u = YogStr::from_str(ui_id);
        unsafe { ((*self.api).register_menu_entry)(self.ctx(), l, u) }
    }

    /// Register a UI tree with an event callback.
    /// `ui_id` is the unique identifier (e.g. "mymod:menu").
    /// `handler` receives `(ui_id, event)` for the screen's input: `click:X:Y`,
    /// `drag:X:Y`, `release:X:Y`, `scroll:DY`, `key:KEY:MODS` (GLFW key code
    /// and modifier bits, on press and repeat) and `char:CODEPOINT` (a typed
    /// character). It returns `()` or a `bool`: `true` marks a `key:`/`char:`
    /// event as used, so the host skips its default handling of the key
    /// (Esc closing the screen, ...) — see [`UiHandled`].
    pub fn register_ui<F, R>(&mut self, ui_id: &str, handler: F)
    where
        F: Fn(&str, &str) -> R + Send + Sync + 'static,
        R: UiHandled,
    {
        let ud = Self::leak(handler);
        let id = YogStr::from_str(ui_id);
        let empty = YogStr::from_str("{}");
        unsafe { ((*self.api).register_ui)(self.ctx(), id, empty, ud, trampoline_ui_event::<F, R>) }
    }

    // ── scheduler ────────────────────────────────────────────────────────────

    pub fn schedule_once<F>(&self, delay_ticks: u64, handler: F)
    where
        F: Fn(&dyn Server) + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        unsafe {
            ((*self.api).schedule_once)(self.ctx(), delay_ticks, ud, trampoline_scheduled::<F>);
        }
    }

    pub fn schedule_repeating<F>(&self, period_ticks: u64, handler: F)
    where
        F: Fn(&dyn Server) + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        unsafe {
            ((*self.api).schedule_repeating)(
                self.ctx(),
                period_ticks,
                ud,
                trampoline_scheduled::<F>,
            );
        }
    }

    // ── client-side hooks (ABI minor 10) ─────────────────────────────────────

    /// Register a render callback for a specific UI screen (`ui_id`).
    ///
    /// Called from `YogUIScreen.render()` — i.e. AFTER `renderBackground()` darkens
    /// the screen — so your UI draws on top of the dimmed world view.
    /// Use this instead of `on_hud_render` for book/inventory screens.
    ///
    /// Clicks are forwarded as `"click:X:Y"` via the `register_ui` handler so you
    /// can do hit-testing on your own stored layout.
    pub fn on_ui_render<F>(&mut self, ui_id: &str, handler: F)
    where
        F: Fn(&GfxContext) + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        let id = YogStr::from_str(ui_id);
        unsafe { ((*self.api).on_ui_render)(self.ctx(), id, ud, trampoline_hud_render::<F>) }
    }

    /// Draw the pictures of book pages of type `drawn` whose `drawer` is
    /// `drawer` (`"mymod:diagram"`): `handler` gets the page's `data` and
    /// the picture's box `[x, y, w, h]` in screen pixels, every frame the
    /// page is open.
    ///
    /// ```text
    /// { type: "drawn", drawer: "mymod:diagram", title: "Wiring", height: 64,
    ///   caption: "{u|in} → {u|out}", text: "…", data: { nodes: 3 } }
    /// ```
    pub fn on_book_page<F>(&mut self, drawer: &str, handler: F)
    where
        F: Fn(&GfxContext, &yog_data::Value, [f32; 4]) + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        let id = YogStr::from_str(drawer);
        unsafe { ((*self.api).on_book_page)(self.ctx(), id, ud, trampoline_book_page::<F>) }
    }

    /// A picture under the tooltips of `item` (as Hex Casting's scrolls show
    /// their pattern): `size(item, snbt)` says whether a stack — its custom
    /// data as SNBT — gets one and its size in GUI pixels; `draw` draws it
    /// in the box `[x, y, w, h]` (screen pixels) while the tooltip shows.
    pub fn on_item_tooltip<S, D>(&mut self, item: &str, size: S, draw: D)
    where
        S: Fn(&str, &str) -> Option<(f32, f32)> + Send + Sync + 'static,
        D: Fn(&GfxContext, &str, &str, [f32; 4]) + Send + Sync + 'static,
    {
        let ud = Self::leak(TooltipPicture { size, draw });
        let id = YogStr::from_str(item);
        unsafe {
            ((*self.api).on_item_tooltip)(self.ctx(), id, ud, trampoline_tooltip_size::<S, D>, trampoline_tooltip_draw::<S, D>)
        }
    }

    /// A stack's own name and tooltip, from its custom data (SNBT): `text(item,
    /// snbt)` gives `Some((name, lines))` — `name` `None` keeps the item's;
    /// `lines` stand instead of the item's description — or `None` for the
    /// item's own. A written blueprint names its design:
    ///
    /// ```ignore
    /// registry.on_item_text(Items::BLUEPRINT, |_, snbt| {
    ///     let bp = Blueprint::from_nbt(snbt)?;
    ///     Some((Some(bp.name.clone()), vec![format!("{} ports", bp.ports)]))
    /// });
    /// ```
    pub fn on_item_text<F>(&mut self, item: &str, text: F)
    where
        F: Fn(&str, &str) -> Option<(Option<String>, Vec<String>)> + Send + Sync + 'static,
    {
        let ud = Self::leak(text);
        let id = YogStr::from_str(item);
        unsafe { ((*self.api).on_item_text)(self.ctx(), id, ud, trampoline_item_text::<F>) }
    }

    /// A compiled `.ymodel` (the generated code calls this): found later by
    /// [`crate::body::model`].
    pub fn register_body_model(&mut self, id: &str, data: &[u8]) {
        if let Err(e) = yog_body::register_model(id, data) {
            crate::error!("{e}");
        }
    }

    /// A compiled `.yanim` (the generated code calls this): its clips found
    /// later by [`crate::body::clip`].
    pub fn register_body_anims(&mut self, id: &str, data: &[u8]) {
        if let Err(e) = yog_body::register_anims(id, data) {
            crate::error!("{e}");
        }
    }

    /// Register a handler asked each frame about the body of every tracked
    /// entity ([`crate::body::track`]): it answers with [`crate::body::Ops`]
    /// — pose vanilla parts, turn the body, draw meshes (rigs, morphs),
    /// disguise, hide, capture. Every mod's ops add up.
    pub fn on_body<F>(&mut self, handler: F)
    where
        F: Fn(&yog_body::Frame, &mut yog_body::Ops) + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_body)(self.ctx(), ud, trampoline_body::<F>) }
    }

    /// Register a handler that sees every loot table rolled — a chest's, a
    /// block's drops, a mob's — and may change what it gives
    /// ([`crate::loot::Loot::items`]); each handler sees the one before's
    /// result. Server side. For adding and scaling items on some tables, a
    /// [`crate::loot::LootModifier`] says it as data.
    pub fn on_loot<F>(&mut self, handler: F)
    where
        F: Fn(&dyn Server, &mut crate::loot::Loot) + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_loot)(self.ctx(), ud, trampoline_loot::<F>) }
    }

    /// Change the loot of some tables as `modifier` says.
    pub fn add_loot_modifier(&mut self, modifier: crate::loot::LootModifier) {
        self.on_loot(move |srv, loot| crate::loot::apply(srv, &modifier, loot));
    }

    /// Register an attribute every player has — what effects change and
    /// [`Server::entity_attribute_value`] reads.
    pub fn register_attribute(&mut self, def: yog_registry::AttributeDef) {
        use yog_data::Schema;
        self.content(yog_ids::kind::ATTRIBUTE, &def.id.clone(), &def.write());
    }

    /// Register a mob effect.
    pub fn register_effect(&mut self, def: yog_registry::EffectDef) {
        use yog_data::Schema;
        self.content(yog_ids::kind::EFFECT, &def.id.clone(), &def.write());
    }

    /// Register a potion and its brewing.
    pub fn register_potion(&mut self, def: yog_registry::PotionDef) {
        use yog_data::Schema;
        self.content(yog_ids::kind::POTION, &def.id.clone(), &def.write());
    }

    /// A compiled `.ydef` the game registers (`attribute`, `effect`,
    /// `potion`) — the generated code calls this.
    pub fn register_definition_data(&mut self, kind: &str, id: &str, data: &[u8]) {
        unsafe { ((*self.api).register_content)(self.ctx(), YogStr::from_str(kind), YogStr::from_str(id), data.as_ptr(), data.len()) }
    }

    /// A compiled `.ydef` loot modifier (the generated code calls this).
    pub fn add_loot_modifier_data(&mut self, id: &str, data: &[u8]) {
        match yog_data::read_bytes::<crate::loot::LootModifier>(data) {
            Ok(m) => self.add_loot_modifier(m),
            Err(e) => crate::error!("loot modifier {id}: {e:?}"),
        }
    }

    /// Register loot function `name` (`mymod:pattern_scroll`): what loot
    /// modifiers naming it (`function`) run on each stack they add — set
    /// its custom data, say.
    pub fn loot_function<F>(&mut self, name: &str, function: F)
    where
        F: Fn(&dyn Server, &mut crate::loot::LootStack, &crate::loot::Loot) + Send + Sync + 'static,
    {
        if let Ok(mut f) = crate::loot::functions().write() {
            f.insert(name.to_string(), std::sync::Arc::new(function));
        }
    }

    /// Register a handler that moves, turns or zooms the camera each frame;
    /// it changes the [`crate::body::CameraOut`] the handlers before it made.
    pub fn on_camera<F>(&mut self, handler: F)
    where
        F: Fn(&yog_body::CameraFrame, &mut yog_body::CameraOut) + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_camera)(self.ctx(), ud, trampoline_camera::<F>) }
    }

    /// Register a handler that may replace the local player's travel step:
    /// `Some(velocity)` (blocks per tick, drag applied) moves the player by
    /// it instead of vanilla's walking, swimming, falling or gliding.
    pub fn on_travel<F>(&mut self, handler: F)
    where
        F: Fn(&yog_body::TravelFrame) -> Option<yog_body::Vec3> + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_travel)(self.ctx(), ud, trampoline_travel::<F>) }
    }

    /// Register a handler called every frame when the HUD is rendered.
    ///
    /// `gfx` provides full GPU pipeline access plus 2D convenience draw calls.
    /// `view_proj` and `camera_pos` are zero in HUD context; use `gfx.draw2d()` for HUD elements.
    pub fn on_hud_render<F>(&mut self, handler: F)
    where
        F: Fn(&GfxContext) + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_hud_render)(self.ctx(), ud, trampoline_hud_render::<F>) }
    }

    /// Register a handler called every frame at the end of world rendering.
    ///
    /// `gfx.view_proj()` is the combined projection × view matrix (camera-relative).
    /// `gfx.camera_pos()` is the camera world position.
    /// To render at world position P, translate by `P - camera_pos` before drawing.
    pub fn on_world_render<F>(&mut self, handler: F)
    where
        F: Fn(&GfxContext) + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_world_render)(self.ctx(), ud, trampoline_world_render::<F>) }
    }

    /// A key binding the player sees in the game's controls (under this
    /// mod's name) and can rebind there; the game keeps the key it's bound
    /// to. `id` names it (`mymod:xray`), `name` is its label's translation
    /// key (a `.yres` text's `…_KEY`), `default_key` a GLFW key code
    /// ([`crate::client::keys`]; `None`: unbound). `handler` runs on the
    /// client thread at each press.
    ///
    /// ```ignore
    /// registry.key_binding("mymod:xray", Strings::KEY_XRAY_KEY, Some(keys::X), || toggle_xray());
    /// ```
    pub fn key_binding<F>(&mut self, id: &str, name: &str, default_key: Option<i32>, handler: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        unsafe {
            ((*self.api).register_key_binding)(
                self.ctx(),
                YogStr::from_str(id),
                YogStr::from_str(name),
                default_key.unwrap_or(crate::client::keys::UNBOUND),
            )
        };
        let id = id.to_owned();
        self.on_key_binding(move |pressed: &str| {
            if pressed == id {
                handler();
            }
        });
    }

    fn on_key_binding<F>(&mut self, handler: F)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        let ud = Self::leak(handler);
        unsafe { ((*self.api).on_key_binding)(self.ctx(), ud, trampoline_key_binding::<F>) }
    }

}

unsafe extern "C" fn trampoline_ui_event<F, R>(
    ud: *mut c_void,
    ui_id: yog_abi::YogStr,
    event_id: yog_abi::YogStr,
) where
    F: Fn(&str, &str) -> R + Send + Sync + 'static,
    R: UiHandled,
{
    let f = unsafe { &*(ud as *const F) };
    let ui = unsafe { ui_id.as_str() };
    let ev = unsafe { event_id.as_str() };
    if guard(false, || f(ui, ev).handled()) {
        let api = GLOBAL_API.load(std::sync::atomic::Ordering::Acquire);
        if !api.is_null() {
            unsafe { ((*api).ui_consume_event)((*api).ctx) }
        }
    }
}

unsafe extern "C" fn trampoline_chunk_generator<F>(
    ud: *mut c_void,
    writer: *const yog_abi::YogChunkWriterApi,
) where
    F: Fn(&ChunkWriter) + Send + Sync + 'static,
{
    let f = unsafe { &*(ud as *const F) };
    let writer = unsafe { ChunkWriter::from_raw(writer) };
    guard((), || f(&writer));
}

// ── Mod trait ─────────────────────────────────────────────────────────────────

/// Implemented by every Yog mod. Called once at startup to register handlers.
pub trait Mod {
    fn register(registry: &mut Registry);
}

#[cfg(test)]
mod trampoline_tests {
    use super::*;
    use yog_abi::{YogCommandEvent, YogPacketEvent};

    fn ud<F>(f: &F) -> *mut c_void {
        f as *const F as *mut c_void
    }

    fn command<F: Fn(&CommandContext, &dyn Server) -> Option<String> + Send + Sync>(f: &F, ev: &YogCommandEvent, buf: &mut [u8]) -> u32 {
        let mut len = 99u32;
        unsafe { trampoline_command::<F>(ud(f), std::ptr::null(), ev, buf.as_mut_ptr(), buf.len() as u32, &mut len) };
        len
    }

    fn packet<F: Fn(&PacketEvent, &dyn Server) + Send + Sync>(f: &F, ev: &YogPacketEvent) {
        unsafe { trampoline_packet::<F>(ud(f), std::ptr::null(), ev) }
    }

    #[test]
    fn command_replies_fit_the_buffer_whole_characters_only() {
        let handler = |c: &CommandContext, _: &dyn Server| Some(format!("{}: привет", c.args));
        let ev = YogCommandEvent {
            name: YogStr::from_str("hex"),
            args: YogStr::from_str("x"),
            source: YogStr::from_str("Dev"),
            uuid: YogStr::EMPTY,
            dimension: YogStr::EMPTY,
        };
        let mut buf = [0u8; 6];
        let len = command(&handler, &ev, &mut buf);
        // "x: п" is 5 bytes; "пр" wouldn't fit whole in 6.
        assert_eq!(std::str::from_utf8(&buf[..len as usize]), Ok("x: п"));
    }

    #[test]
    fn empty_packets_need_no_payload_pointer() {
        let got = std::sync::Mutex::new(None);
        let handler = |p: &PacketEvent, _: &dyn Server| *got.lock().unwrap() = Some(p.payload.len());
        let ev = YogPacketEvent {
            channel: YogStr::from_str("mymod:ping"),
            player: YogStr::from_str("Dev"),
            payload: std::ptr::null(),
            payload_len: 0,
        };
        packet(&handler, &ev);
        assert_eq!(*got.lock().unwrap(), Some(0));
    }
}

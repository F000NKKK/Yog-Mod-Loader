//! [`NullServer`] — a [`Server`] with no game behind it, for benchmarks and
//! tests of a mod's own logic.

use crate::{BlockHit, BlockInfo, BlockPos, EntityInfo, Face, PlayerInfo, Server};

/// A [`Server`] that does nothing: every query comes back empty, every
/// action reports failure. Enough to run mod logic that only needs a handle
/// (`yog bench` benchmarks, unit tests). `game_dir` is where the mod's
/// storage goes — point it at a scratch directory.
#[derive(Debug, Clone, Default)]
pub struct NullServer {
    pub game_dir: String,
}

impl NullServer {
    /// A null server whose game directory is `dir`.
    pub fn new(dir: impl Into<String>) -> Self {
        Self {
            game_dir: dir.into(),
        }
    }
}

#[allow(unused_variables)]
impl Server for NullServer {
    fn broadcast(&self, message: &str) {
        Default::default()
    }
    fn get_block(&self, dimension: &str, pos: BlockPos) -> Option<String> {
        Default::default()
    }
    fn set_block(&self, dimension: &str, pos: BlockPos, block_id: &str) -> bool {
        Default::default()
    }
    fn give_item(&self, player: &str, item_id: &str, count: u32) -> bool {
        Default::default()
    }
    fn teleport(&self, player: &str, x: f64, y: f64, z: f64) -> bool {
        Default::default()
    }
    fn send_to_player(&self, player: &str, channel: &str, payload: &[u8]) -> bool {
        Default::default()
    }
    fn send_to_server(&self, channel: &str, payload: &[u8]) -> bool {
        Default::default()
    }
    fn entity_teleport(&self, uuid: &str, x: f64, y: f64, z: f64) -> bool {
        Default::default()
    }
    fn entity_position(&self, uuid: &str) -> Option<(f64, f64, f64)> {
        Default::default()
    }
    fn entity_health(&self, uuid: &str) -> Option<f32> {
        Default::default()
    }
    fn entity_set_health(&self, uuid: &str, health: f32) -> bool {
        Default::default()
    }
    fn entity_kill(&self, uuid: &str) -> bool {
        Default::default()
    }
    fn spawn_entity(
        &self,
        entity_type: &str,
        dimension: &str,
        x: f64,
        y: f64,
        z: f64,
    ) -> Option<String> {
        Default::default()
    }
    fn entity_add_effect(
        &self,
        uuid: &str,
        effect_id: &str,
        duration_ticks: i32,
        amplifier: u8,
        show_particles: bool,
    ) -> bool {
        Default::default()
    }
    fn entity_remove_effect(&self, uuid: &str, effect_id: &str) -> bool {
        Default::default()
    }
    fn entity_clear_effects(&self, uuid: &str) -> bool {
        Default::default()
    }
    fn drop_loot(&self, table_id: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool {
        Default::default()
    }
    fn has_item_tag(&self, item_id: &str, tag_id: &str) -> bool {
        Default::default()
    }
    fn has_block_tag(&self, block_id: &str, tag_id: &str) -> bool {
        Default::default()
    }
    fn world_time(&self, dimension: &str) -> Option<i64> {
        Default::default()
    }
    fn world_set_time(&self, dimension: &str, time: i64) -> bool {
        Default::default()
    }
    fn world_is_raining(&self, dimension: &str) -> bool {
        Default::default()
    }
    fn world_set_weather(&self, dimension: &str, raining: bool, duration_ticks: i32) -> bool {
        Default::default()
    }
    fn entity_velocity(&self, uuid: &str) -> Option<(f64, f64, f64)> {
        Default::default()
    }
    fn entity_set_velocity(&self, uuid: &str, vx: f64, vy: f64, vz: f64) -> bool {
        Default::default()
    }
    fn entity_add_velocity(&self, uuid: &str, vx: f64, vy: f64, vz: f64) -> bool {
        Default::default()
    }
    fn scoreboard_get(&self, objective: &str, player: &str) -> Option<i32> {
        Default::default()
    }
    fn scoreboard_set(&self, objective: &str, player: &str, score: i32) -> bool {
        Default::default()
    }
    fn scoreboard_add(&self, objective: &str, player: &str, delta: i32) -> Option<i32> {
        Default::default()
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
        Default::default()
    }
    fn play_sound_to_player(&self, player: &str, sound_id: &str, volume: f32, pitch: f32) -> bool {
        Default::default()
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
        Default::default()
    }
    fn send_actionbar(&self, player: &str, message: &str) -> bool {
        Default::default()
    }
    fn entity_rotation(&self, uuid: &str) -> Option<(f32, f32)> {
        Default::default()
    }
    fn kick_player(&self, player: &str, reason: &str) -> bool {
        Default::default()
    }
    fn set_gamemode(&self, player: &str, gamemode: &str) -> bool {
        Default::default()
    }
    fn bossbar_create(&self, id: &str, title: &str, color: &str, style: &str) -> bool {
        Default::default()
    }
    fn bossbar_remove(&self, id: &str) -> bool {
        Default::default()
    }
    fn bossbar_set_title(&self, id: &str, title: &str) -> bool {
        Default::default()
    }
    fn bossbar_set_progress(&self, id: &str, progress: f32) -> bool {
        Default::default()
    }
    fn bossbar_set_color(&self, id: &str, color: &str) -> bool {
        Default::default()
    }
    fn bossbar_add_player(&self, id: &str, player: &str) -> bool {
        Default::default()
    }
    fn bossbar_remove_player(&self, id: &str, player: &str) -> bool {
        Default::default()
    }
    fn bossbar_set_visible(&self, id: &str, visible: bool) -> bool {
        Default::default()
    }
    fn game_dir(&self) -> String {
        self.game_dir.clone()
    }
    fn online_players(&self) -> Vec<String> {
        Default::default()
    }
    fn get_block_nbt(&self, dimension: &str, pos: BlockPos) -> Option<String> {
        Default::default()
    }
    fn set_block_nbt(&self, dimension: &str, pos: BlockPos, snbt: &str) -> bool {
        Default::default()
    }
    fn get_inventory_slot(
        &self,
        dimension: &str,
        pos: BlockPos,
        slot: u32,
    ) -> Option<(String, u32, String)> {
        Default::default()
    }
    fn set_inventory_slot(
        &self,
        dimension: &str,
        pos: BlockPos,
        slot: u32,
        item_id: &str,
        count: u32,
        snbt: &str,
    ) -> bool {
        Default::default()
    }
    fn player_inventory(&self, player: &str) -> Vec<(u32, String, u32)> {
        Default::default()
    }
    fn player_set_slot(&self, player: &str, slot: u32, item_id: &str, count: u32) -> bool {
        Default::default()
    }
    fn teleport_to_dim(&self, player: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool {
        Default::default()
    }
    fn entity_teleport_to_dim(&self, uuid: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool {
        Default::default()
    }
    fn world_entity_count(&self, dimension: &str, entity_type: &str) -> i32 {
        Default::default()
    }
    fn entity_get_nbt(&self, uuid: &str) -> Option<String> {
        Default::default()
    }
    fn entity_set_nbt(&self, uuid: &str, snbt: &str) -> bool {
        Default::default()
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
        Default::default()
    }
    fn entity_attribute_get(&self, uuid: &str, attribute_id: &str) -> Option<f64> {
        Default::default()
    }
    fn entity_attribute_set(&self, uuid: &str, attribute_id: &str, value: f64) -> bool {
        Default::default()
    }
    fn get_held_item_nbt(&self, player: &str) -> Option<String> {
        Default::default()
    }
    fn set_held_item_nbt(&self, player: &str, snbt: &str) -> bool {
        Default::default()
    }
    fn get_offhand_item_nbt(&self, player: &str) -> Option<String> {
        Default::default()
    }
    fn set_offhand_item_nbt(&self, player: &str, snbt: &str) -> bool {
        Default::default()
    }
    fn get_slot_item(&self, player: &str, slot: u32) -> Option<(String, u32, String)> {
        Default::default()
    }
    fn set_slot_item(
        &self,
        player: &str,
        slot: u32,
        item_id: &str,
        count: u32,
        snbt: &str,
    ) -> bool {
        Default::default()
    }
    fn entity_info(&self, uuid: &str) -> Option<EntityInfo> {
        Default::default()
    }
    fn entities_in_box(
        &self,
        dimension: &str,
        min: (f64, f64, f64),
        max: (f64, f64, f64),
    ) -> Vec<String> {
        Default::default()
    }
    fn raycast_block(
        &self,
        dimension: &str,
        from: (f64, f64, f64),
        to: (f64, f64, f64),
    ) -> Option<BlockHit> {
        Default::default()
    }
    fn raycast_entity(
        &self,
        dimension: &str,
        from: (f64, f64, f64),
        to: (f64, f64, f64),
        exclude: &str,
    ) -> Option<String> {
        Default::default()
    }
    fn block_info(&self, dimension: &str, pos: BlockPos) -> Option<BlockInfo> {
        Default::default()
    }
    fn break_block(&self, dimension: &str, pos: BlockPos, drop: bool, breaker: &str) -> bool {
        Default::default()
    }
    fn place_block_from_slot(
        &self,
        player: &str,
        slot: u32,
        dimension: &str,
        pos: BlockPos,
    ) -> bool {
        Default::default()
    }
    fn bonemeal(&self, dimension: &str, pos: BlockPos) -> bool {
        Default::default()
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
        Default::default()
    }
    fn tellraw(&self, player: &str, json: &str) -> bool {
        Default::default()
    }
    fn entity_damage(&self, uuid: &str, amount: f32, damage_type: &str, attacker: &str) -> bool {
        Default::default()
    }
    fn entity_set_fire_ticks(&self, uuid: &str, ticks: i32) -> bool {
        Default::default()
    }
    fn player_add_xp(&self, player: &str, amount: i32) -> bool {
        Default::default()
    }
    fn player_set_may_fly(&self, player: &str, may_fly: bool) -> bool {
        Default::default()
    }
    fn player_info(&self, player: &str) -> Option<PlayerInfo> {
        Default::default()
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
        Default::default()
    }
    fn item_entity_stack(&self, uuid: &str) -> Option<(String, u32, String)> {
        Default::default()
    }
    fn item_entity_set_stack(&self, uuid: &str, item_id: &str, count: u32, snbt: &str) -> bool {
        Default::default()
    }
    fn actionbar_json(&self, player: &str, json: &str) -> bool {
        Default::default()
    }
    fn use_item_on_block(
        &self,
        dimension: &str,
        pos: BlockPos,
        face: Face,
        item_id: &str,
        player: &str,
    ) -> bool {
        Default::default()
    }
    fn pour_fluid(&self, dimension: &str, pos: BlockPos, bucket_item_id: &str) -> bool {
        Default::default()
    }
    fn has_entity_tag(&self, entity_type: &str, tag_id: &str) -> bool {
        Default::default()
    }
    fn item_block(&self, item_id: &str) -> Option<String> {
        Default::default()
    }
    fn place_feature(&self, dimension: &str, pos: BlockPos, feature_id: &str) -> bool {
        Default::default()
    }
    fn set_item_cooldown(&self, player: &str, item_id: &str, ticks: u32) -> bool {
        Default::default()
    }
    fn entity_vehicle(&self, uuid: &str) -> Option<String> {
        Default::default()
    }
    fn entity_passengers(&self, uuid: &str) -> Vec<String> {
        Default::default()
    }
    fn entity_mount(&self, rider: &str, vehicle: &str, force: bool) -> bool {
        Default::default()
    }
    fn entity_dismount(&self, uuid: &str) -> bool {
        Default::default()
    }
    fn entity_set_glider(&self, uuid: &str, glide: bool) -> bool {
        Default::default()
    }
    fn redstone_signal(&self, dimension: &str, pos: BlockPos) -> Option<u8> {
        Default::default()
    }
    fn redstone_power(&self, dimension: &str, pos: BlockPos) -> Option<u8> {
        Default::default()
    }
    fn set_redstone_output(&self, dimension: &str, pos: BlockPos, level: u8) -> bool {
        Default::default()
    }
    fn get_block_state(&self, dimension: &str, pos: BlockPos) -> Option<String> {
        Default::default()
    }
    fn set_block_state(&self, dimension: &str, pos: BlockPos, state: &str) -> bool {
        Default::default()
    }
    fn world_is_thundering(&self, dimension: &str) -> bool {
        Default::default()
    }
    fn world_set_thunder(&self, dimension: &str, thundering: bool, duration_ticks: i32) -> bool {
        Default::default()
    }
    fn fill_region(&self, dimension: &str, from: BlockPos, to: BlockPos, state: &str) -> Option<u32> {
        Default::default()
    }
    fn scan_region(&self, dimension: &str, from: BlockPos, to: BlockPos) -> Vec<(BlockPos, String)> {
        Default::default()
    }
}

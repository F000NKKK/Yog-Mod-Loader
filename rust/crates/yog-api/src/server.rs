//! The server's actions for mods ([`Server`]): each a call of the bus action
//! the Java host provides (`yog_event::<domain>::*`, `docs/design/bus.md`).

use yog_core::{BlockHit, BlockInfo, BlockPos, EntityInfo, Face, PlayerInfo, Server};
use yog_event::entity::{self, DataValue};
use yog_event::{Point, block, bossbar, chat, game, item, net, particle, player, scoreboard, sound, tag, world};

use crate::call;

/// The server's actions, through the bus. Given to every handler as `&dyn Server`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CServer;

/// The highest redstone level.
const MAX_REDSTONE: u8 = 15;

fn point((x, y, z): (f64, f64, f64)) -> Point {
    Point { x, y, z }
}

/// A text reply that's there and not empty.
fn text(reply: Option<Option<String>>) -> Option<String> {
    reply.flatten().filter(|t| !t.is_empty())
}

/// A reply's non-empty lines.
fn lines(reply: Option<Option<String>>) -> Vec<String> {
    text(reply).map(|t| t.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect()).unwrap_or_default()
}

/// `N` tab-separated numbers.
fn numbers<const N: usize>(reply: Option<Option<String>>) -> Option<[f64; N]> {
    let t = text(reply)?;
    let parts: Vec<f64> = t.split('\t').map(str::parse).collect::<Result<_, _>>().ok()?;
    parts.try_into().ok()
}

/// A stack's line: `item\tcount\tsnbt`.
fn stack(reply: Option<Option<String>>) -> Option<(String, u32, String)> {
    let t = text(reply)?;
    let mut it = t.splitn(3, '\t');
    let item = it.next()?.to_owned();
    let count = it.next()?.parse().ok()?;
    Some((item, count, it.next().unwrap_or("{}").to_owned()))
}

/// A redstone level the host answered (`-1` and out of range: none).
fn redstone(reply: Option<i32>) -> Option<u8> {
    reply.and_then(|p| u8::try_from(p).ok()).filter(|p| *p <= MAX_REDSTONE)
}

fn done(reply: Option<bool>) -> bool {
    reply.unwrap_or(false)
}

impl Server for CServer {
    fn broadcast(&self, message: &str) {
        call(&chat::Broadcast { message: message.to_owned() });
    }

    fn get_block(&self, dimension: &str, pos: BlockPos) -> Option<String> {
        call(&world::GetBlock { dim: dimension.to_owned(), pos }).flatten()
    }

    fn set_block(&self, dimension: &str, pos: BlockPos, block_id: &str) -> bool {
        done(call(&world::SetBlock { dim: dimension.to_owned(), pos, block: block_id.to_owned() }))
    }

    fn give_item(&self, player: &str, item_id: &str, count: u32) -> bool {
        done(call(&player::GiveItem { player: player.to_owned(), item: item_id.to_owned(), count }))
    }

    fn teleport(&self, player: &str, x: f64, y: f64, z: f64) -> bool {
        done(call(&player::Teleport { player: player.to_owned(), pos: point((x, y, z)) }))
    }

    fn send_to_player(&self, player: &str, channel: &str, payload: &[u8]) -> bool {
        done(call(&net::SendToPlayer { player: player.to_owned(), channel: channel.to_owned(), payload: payload.to_vec() }))
    }

    fn send_to_server(&self, channel: &str, payload: &[u8]) -> bool {
        done(call(&net::SendToServer { channel: channel.to_owned(), payload: payload.to_vec() }))
    }

    fn entity_teleport(&self, uuid: &str, x: f64, y: f64, z: f64) -> bool {
        done(call(&entity::Teleport { uuid: uuid.to_owned(), pos: point((x, y, z)) }))
    }

    fn entity_position(&self, uuid: &str) -> Option<(f64, f64, f64)> {
        numbers::<3>(call(&entity::Position { uuid: uuid.to_owned() })).map(|[x, y, z]| (x, y, z))
    }

    fn entity_health(&self, uuid: &str) -> Option<f32> {
        call(&entity::Health { uuid: uuid.to_owned() }).flatten().map(|h| h as f32)
    }

    fn entity_set_health(&self, uuid: &str, health: f32) -> bool {
        done(call(&entity::SetHealth { uuid: uuid.to_owned(), health: f64::from(health) }))
    }

    fn entity_kill(&self, uuid: &str) -> bool {
        done(call(&entity::Kill { uuid: uuid.to_owned() }))
    }

    fn spawn_entity(&self, entity_type: &str, dimension: &str, x: f64, y: f64, z: f64) -> Option<String> {
        text(call(&entity::Spawn { entity_type: entity_type.to_owned(), dim: dimension.to_owned(), pos: point((x, y, z)) }))
    }

    fn entity_add_effect(&self, uuid: &str, effect_id: &str, duration_ticks: i32, amplifier: u8, show_particles: bool) -> bool {
        done(call(&entity::AddEffect { uuid: uuid.to_owned(), effect: effect_id.to_owned(), duration: duration_ticks, amplifier, particles: show_particles }))
    }

    fn entity_remove_effect(&self, uuid: &str, effect_id: &str) -> bool {
        done(call(&entity::RemoveEffect { uuid: uuid.to_owned(), effect: effect_id.to_owned() }))
    }

    fn entity_clear_effects(&self, uuid: &str) -> bool {
        done(call(&entity::ClearEffects { uuid: uuid.to_owned() }))
    }

    fn drop_loot(&self, table_id: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool {
        done(call(&world::DropLoot { table: table_id.to_owned(), dim: dimension.to_owned(), pos: point((x, y, z)) }))
    }

    fn has_item_tag(&self, item_id: &str, tag_id: &str) -> bool {
        done(call(&tag::HasItem { item: item_id.to_owned(), tag: tag_id.to_owned() }))
    }

    fn has_block_tag(&self, block_id: &str, tag_id: &str) -> bool {
        done(call(&tag::HasBlock { block: block_id.to_owned(), tag: tag_id.to_owned() }))
    }

    fn world_time(&self, dimension: &str) -> Option<i64> {
        call(&world::Time { dim: dimension.to_owned() }).flatten()
    }

    fn world_set_time(&self, dimension: &str, time: i64) -> bool {
        done(call(&world::SetTime { dim: dimension.to_owned(), time }))
    }

    fn world_is_raining(&self, dimension: &str) -> bool {
        done(call(&world::IsRaining { dim: dimension.to_owned() }))
    }

    fn world_set_weather(&self, dimension: &str, raining: bool, duration_ticks: i32) -> bool {
        done(call(&world::SetWeather { dim: dimension.to_owned(), raining, duration: duration_ticks }))
    }

    fn entity_velocity(&self, uuid: &str) -> Option<(f64, f64, f64)> {
        numbers::<3>(call(&entity::Velocity { uuid: uuid.to_owned() })).map(|[x, y, z]| (x, y, z))
    }

    fn entity_set_velocity(&self, uuid: &str, vx: f64, vy: f64, vz: f64) -> bool {
        done(call(&entity::SetVelocity { uuid: uuid.to_owned(), velocity: point((vx, vy, vz)) }))
    }

    fn entity_add_velocity(&self, uuid: &str, vx: f64, vy: f64, vz: f64) -> bool {
        done(call(&entity::AddVelocity { uuid: uuid.to_owned(), velocity: point((vx, vy, vz)) }))
    }

    fn scoreboard_get(&self, objective: &str, player: &str) -> Option<i32> {
        call(&scoreboard::Get { objective: objective.to_owned(), player: player.to_owned() }).flatten()
    }

    fn scoreboard_set(&self, objective: &str, player: &str, score: i32) -> bool {
        done(call(&scoreboard::Set { objective: objective.to_owned(), player: player.to_owned(), score }))
    }

    fn scoreboard_add(&self, objective: &str, player: &str, delta: i32) -> Option<i32> {
        call(&scoreboard::Add { objective: objective.to_owned(), player: player.to_owned(), delta }).flatten()
    }

    fn game_dir(&self) -> String {
        text(call(&game::Dir {})).unwrap_or_default()
    }

    fn world_dir(&self) -> String {
        text(call(&game::WorldDir {})).unwrap_or_default()
    }

    fn play_sound(&self, dimension: &str, x: f64, y: f64, z: f64, sound_id: &str, volume: f32, pitch: f32) -> bool {
        done(call(&sound::Play { dim: dimension.to_owned(), pos: point((x, y, z)), sound: sound_id.to_owned(), volume, pitch }))
    }

    fn play_sound_to_player(&self, player: &str, sound_id: &str, volume: f32, pitch: f32) -> bool {
        done(call(&sound::PlayToPlayer { player: player.to_owned(), sound: sound_id.to_owned(), volume, pitch }))
    }

    fn send_title(&self, player: &str, title: &str, subtitle: &str, fadein: i32, stay: i32, fadeout: i32) -> bool {
        done(call(&chat::Title { player: player.to_owned(), title: title.to_owned(), subtitle: subtitle.to_owned(), fade_in: fadein, stay, fade_out: fadeout }))
    }

    fn entity_rotation(&self, uuid: &str) -> Option<(f32, f32)> {
        numbers::<2>(call(&entity::Rotation { uuid: uuid.to_owned() })).map(|[yaw, pitch]| (yaw as f32, pitch as f32))
    }

    fn send_actionbar(&self, player: &str, message: &str) -> bool {
        done(call(&chat::Actionbar { player: player.to_owned(), message: message.to_owned() }))
    }

    fn kick_player(&self, player: &str, reason: &str) -> bool {
        done(call(&player::Kick { player: player.to_owned(), reason: reason.to_owned() }))
    }

    fn set_gamemode(&self, player: &str, gamemode: &str) -> bool {
        done(call(&player::SetGamemode { player: player.to_owned(), mode: gamemode.to_owned() }))
    }

    fn bossbar_create(&self, id: &str, title: &str, color: &str, style: &str) -> bool {
        done(call(&bossbar::Create { id: id.to_owned(), title: title.to_owned(), color: color.to_owned(), style: style.to_owned() }))
    }

    fn bossbar_remove(&self, id: &str) -> bool {
        done(call(&bossbar::Remove { id: id.to_owned() }))
    }

    fn bossbar_set_title(&self, id: &str, title: &str) -> bool {
        done(call(&bossbar::SetTitle { id: id.to_owned(), title: title.to_owned() }))
    }

    fn bossbar_set_progress(&self, id: &str, progress: f32) -> bool {
        done(call(&bossbar::SetProgress { id: id.to_owned(), progress }))
    }

    fn bossbar_set_color(&self, id: &str, color: &str) -> bool {
        done(call(&bossbar::SetColor { id: id.to_owned(), color: color.to_owned() }))
    }

    fn bossbar_add_player(&self, id: &str, player: &str) -> bool {
        done(call(&bossbar::AddPlayer { id: id.to_owned(), player: player.to_owned() }))
    }

    fn bossbar_remove_player(&self, id: &str, player: &str) -> bool {
        done(call(&bossbar::RemovePlayer { id: id.to_owned(), player: player.to_owned() }))
    }

    fn bossbar_set_visible(&self, id: &str, visible: bool) -> bool {
        done(call(&bossbar::SetVisible { id: id.to_owned(), visible }))
    }

    fn online_players(&self) -> Vec<String> {
        lines(call(&player::Online {}))
    }

    fn get_block_nbt(&self, dimension: &str, pos: BlockPos) -> Option<String> {
        call(&block::Nbt { dim: dimension.to_owned(), pos }).flatten()
    }

    fn set_block_nbt(&self, dimension: &str, pos: BlockPos, snbt: &str) -> bool {
        done(call(&block::SetNbt { dim: dimension.to_owned(), pos, snbt: snbt.to_owned() }))
    }

    fn get_inventory_slot(&self, dimension: &str, pos: BlockPos, slot: u32) -> Option<(String, u32, String)> {
        stack(call(&block::InventorySlot { dim: dimension.to_owned(), pos, slot }))
    }

    fn set_inventory_slot(&self, dimension: &str, pos: BlockPos, slot: u32, item_id: &str, count: u32, _snbt: &str) -> bool {
        done(call(&block::SetInventorySlot { dim: dimension.to_owned(), pos, slot, item: item_id.to_owned(), count }))
    }

    fn player_inventory(&self, player: &str) -> Vec<(u32, String, u32)> {
        lines(call(&player::Inventory { player: player.to_owned() }))
            .iter()
            .filter_map(|line| {
                let mut it = line.split('\t');
                let slot = it.next()?.parse().ok()?;
                let item = it.next()?.to_owned();
                Some((slot, item, it.next()?.parse().ok()?))
            })
            .collect()
    }

    fn player_set_slot(&self, player: &str, slot: u32, item_id: &str, count: u32) -> bool {
        done(call(&player::SetSlot { player: player.to_owned(), slot, item: item_id.to_owned(), count }))
    }

    fn teleport_to_dim(&self, player: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool {
        done(call(&player::TeleportToDim { player: player.to_owned(), dim: dimension.to_owned(), pos: point((x, y, z)) }))
    }

    fn entity_teleport_to_dim(&self, uuid: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool {
        done(call(&entity::TeleportToDim { uuid: uuid.to_owned(), dim: dimension.to_owned(), pos: point((x, y, z)) }))
    }

    fn world_entity_count(&self, dimension: &str, entity_type: &str) -> i32 {
        call(&world::EntityCount { dim: dimension.to_owned(), entity_type: entity_type.to_owned() }).unwrap_or(-1)
    }

    fn entity_get_nbt(&self, uuid: &str) -> Option<String> {
        call(&entity::Nbt { uuid: uuid.to_owned() }).flatten()
    }

    fn entity_set_nbt(&self, uuid: &str, snbt: &str) -> bool {
        done(call(&entity::SetNbt { uuid: uuid.to_owned(), snbt: snbt.to_owned() }))
    }

    fn spawn_particles(&self, dimension: &str, x: f64, y: f64, z: f64, particle_type: &str, count: i32, dx: f64, dy: f64, dz: f64, speed: f64) -> bool {
        let spawn = particle::Spawn { dim: dimension.to_owned(), pos: point((x, y, z)), particle: particle_type.to_owned(), count, spread: point((dx, dy, dz)), speed };
        done(call(&spawn))
    }

    fn entity_attribute_value(&self, uuid: &str, attribute_id: &str) -> Option<f64> {
        call(&entity::AttributeValue { uuid: uuid.to_owned(), attribute: attribute_id.to_owned() }).flatten()
    }

    fn entity_attribute_get(&self, uuid: &str, attribute_id: &str) -> Option<f64> {
        call(&entity::AttributeBase { uuid: uuid.to_owned(), attribute: attribute_id.to_owned() }).flatten()
    }

    fn entity_attribute_set(&self, uuid: &str, attribute_id: &str, value: f64) -> bool {
        done(call(&entity::SetAttributeBase { uuid: uuid.to_owned(), attribute: attribute_id.to_owned(), value }))
    }

    fn get_held_item_nbt(&self, player: &str) -> Option<String> {
        call(&player::HeldItemNbt { player: player.to_owned() }).flatten()
    }

    fn set_held_item_nbt(&self, player: &str, snbt: &str) -> bool {
        done(call(&player::SetHeldItemNbt { player: player.to_owned(), snbt: snbt.to_owned() }))
    }

    fn get_offhand_item_nbt(&self, player: &str) -> Option<String> {
        call(&player::OffhandItemNbt { player: player.to_owned() }).flatten()
    }

    fn set_offhand_item_nbt(&self, player: &str, snbt: &str) -> bool {
        done(call(&player::SetOffhandItemNbt { player: player.to_owned(), snbt: snbt.to_owned() }))
    }

    fn get_slot_item(&self, player: &str, slot: u32) -> Option<(String, u32, String)> {
        stack(call(&player::SlotItem { player: player.to_owned(), slot }))
    }

    fn set_slot_item(&self, player: &str, slot: u32, item_id: &str, count: u32, snbt: &str) -> bool {
        done(call(&player::SetSlotItem { player: player.to_owned(), slot, item: item_id.to_owned(), count, snbt: snbt.to_owned() }))
    }

    fn entity_info(&self, uuid: &str) -> Option<EntityInfo> {
        text(call(&entity::Info { uuid: uuid.to_owned() })).and_then(|l| EntityInfo::decode(&l))
    }

    fn entities_in_box(&self, dimension: &str, min: (f64, f64, f64), max: (f64, f64, f64)) -> Vec<String> {
        lines(call(&world::EntitiesInBox { dim: dimension.to_owned(), min: point(min), max: point(max) }))
    }

    fn raycast_block(&self, dimension: &str, from: (f64, f64, f64), to: (f64, f64, f64)) -> Option<BlockHit> {
        text(call(&world::RaycastBlock { dim: dimension.to_owned(), from: point(from), to: point(to) })).and_then(|l| BlockHit::decode(&l))
    }

    fn raycast_entity(&self, dimension: &str, from: (f64, f64, f64), to: (f64, f64, f64), exclude: &str) -> Option<String> {
        text(call(&world::RaycastEntity { dim: dimension.to_owned(), from: point(from), to: point(to), exclude: exclude.to_owned() }))
    }

    fn block_info(&self, dimension: &str, pos: BlockPos) -> Option<BlockInfo> {
        text(call(&block::Info { dim: dimension.to_owned(), pos })).and_then(|l| BlockInfo::decode(&l))
    }

    fn break_block(&self, dimension: &str, pos: BlockPos, drop: bool, breaker: &str) -> bool {
        done(call(&block::Destroy { dim: dimension.to_owned(), pos, drop, breaker: breaker.to_owned() }))
    }

    fn place_block_from_slot(&self, player: &str, slot: u32, dimension: &str, pos: BlockPos) -> bool {
        done(call(&player::PlaceBlockFromSlot { player: player.to_owned(), slot, dim: dimension.to_owned(), pos }))
    }

    fn bonemeal(&self, dimension: &str, pos: BlockPos) -> bool {
        done(call(&block::Bonemeal { dim: dimension.to_owned(), pos }))
    }

    fn explode(&self, dimension: &str, x: f64, y: f64, z: f64, power: f32, fire: bool, source: &str) -> bool {
        done(call(&world::Explode { dim: dimension.to_owned(), pos: point((x, y, z)), power, fire, source: source.to_owned() }))
    }

    fn tellraw(&self, player: &str, json: &str) -> bool {
        done(call(&chat::Tellraw { player: player.to_owned(), json: json.to_owned() }))
    }

    fn entity_damage(&self, uuid: &str, amount: f32, damage_type: &str, attacker: &str) -> bool {
        done(call(&entity::Damage { uuid: uuid.to_owned(), amount, damage_type: damage_type.to_owned(), attacker: attacker.to_owned() }))
    }

    fn entity_set_fire_ticks(&self, uuid: &str, ticks: i32) -> bool {
        done(call(&entity::SetFireTicks { uuid: uuid.to_owned(), ticks }))
    }

    fn player_add_xp(&self, player: &str, amount: i32) -> bool {
        done(call(&player::AddXp { player: player.to_owned(), amount }))
    }

    fn player_set_may_fly(&self, player: &str, may_fly: bool) -> bool {
        done(call(&player::SetMayFly { player: player.to_owned(), may_fly }))
    }

    fn player_info(&self, player: &str) -> Option<PlayerInfo> {
        text(call(&player::Info { player: player.to_owned() })).and_then(|l| PlayerInfo::decode(&l))
    }

    fn spawn_item(&self, dimension: &str, pos: (f64, f64, f64), vel: (f64, f64, f64), item_id: &str, count: u32, snbt: &str) -> Option<String> {
        let spawn = item::Spawn { dim: dimension.to_owned(), pos: point(pos), velocity: point(vel), item: item_id.to_owned(), count, snbt: snbt.to_owned() };
        text(call(&spawn))
    }

    fn item_entity_stack(&self, uuid: &str) -> Option<(String, u32, String)> {
        stack(call(&item::EntityStack { uuid: uuid.to_owned() }))
    }

    fn item_entity_set_stack(&self, uuid: &str, item_id: &str, count: u32, snbt: &str) -> bool {
        done(call(&item::SetEntityStack { uuid: uuid.to_owned(), item: item_id.to_owned(), count, snbt: snbt.to_owned() }))
    }

    fn actionbar_json(&self, player: &str, json: &str) -> bool {
        done(call(&chat::ActionbarJson { player: player.to_owned(), json: json.to_owned() }))
    }

    fn use_item_on_block(&self, dimension: &str, pos: BlockPos, face: Face, item_id: &str, player: &str) -> bool {
        done(call(&block::UseItem { dim: dimension.to_owned(), pos, face: face.index(), item: item_id.to_owned(), player: player.to_owned() }))
    }

    fn pour_fluid(&self, dimension: &str, pos: BlockPos, bucket_item_id: &str) -> bool {
        done(call(&block::PourFluid { dim: dimension.to_owned(), pos, bucket: bucket_item_id.to_owned() }))
    }

    fn has_entity_tag(&self, entity_type: &str, tag_id: &str) -> bool {
        done(call(&tag::HasEntity { entity_type: entity_type.to_owned(), tag: tag_id.to_owned() }))
    }

    fn item_block(&self, item_id: &str) -> Option<String> {
        text(call(&item::Block { item: item_id.to_owned() }))
    }

    fn place_feature(&self, dimension: &str, pos: BlockPos, feature_id: &str) -> bool {
        done(call(&world::PlaceFeature { dim: dimension.to_owned(), pos, feature: feature_id.to_owned() }))
    }

    fn set_item_cooldown(&self, player: &str, item_id: &str, ticks: u32) -> bool {
        let ticks = i32::try_from(ticks).unwrap_or(i32::MAX);
        done(call(&player::SetItemCooldown { player: player.to_owned(), item: item_id.to_owned(), ticks }))
    }

    fn entity_vehicle(&self, uuid: &str) -> Option<String> {
        text(call(&entity::Vehicle { uuid: uuid.to_owned() }))
    }

    fn entity_passengers(&self, uuid: &str) -> Vec<String> {
        lines(call(&entity::Passengers { uuid: uuid.to_owned() }))
    }

    fn entity_mount(&self, rider: &str, vehicle: &str, force: bool) -> bool {
        done(call(&entity::Mount { rider: rider.to_owned(), vehicle: vehicle.to_owned(), force }))
    }

    fn entity_dismount(&self, uuid: &str) -> bool {
        done(call(&entity::Dismount { uuid: uuid.to_owned() }))
    }

    fn entity_data(&self, uuid: &str, key: &str) -> Option<Vec<u8>> {
        call(&entity::Data { uuid: uuid.to_owned(), key: key.to_owned() }).flatten().map(|v| v.bytes)
    }

    fn set_entity_data(&self, uuid: &str, key: &str, value: Option<&[u8]>, persist: bool) -> bool {
        let value = value.map(|bytes| DataValue { bytes: bytes.to_vec() });
        done(call(&entity::SetData { uuid: uuid.to_owned(), key: key.to_owned(), value, persist }))
    }

    fn entity_set_dimensions(&self, uuid: &str, size: Option<(f32, f32, f32)>) -> bool {
        done(call(&entity::SetDimensions { uuid: uuid.to_owned(), size: size.map(|(w, h, e)| [w, h, e]) }))
    }

    fn entity_set_fall_distance(&self, uuid: &str, distance: f32) -> bool {
        done(call(&entity::SetFallDistance { uuid: uuid.to_owned(), distance }))
    }

    fn redstone_signal(&self, dimension: &str, pos: BlockPos) -> Option<u8> {
        redstone(call(&block::RedstoneSignal { dim: dimension.to_owned(), pos }))
    }

    fn redstone_power(&self, dimension: &str, pos: BlockPos) -> Option<u8> {
        redstone(call(&block::RedstonePower { dim: dimension.to_owned(), pos }))
    }

    fn set_redstone_output(&self, dimension: &str, pos: BlockPos, level: u8) -> bool {
        done(call(&block::SetRedstoneOutput { dim: dimension.to_owned(), pos, level: i32::from(level.min(MAX_REDSTONE)) }))
    }

    fn get_block_state(&self, dimension: &str, pos: BlockPos) -> Option<String> {
        text(call(&block::State { dim: dimension.to_owned(), pos }))
    }

    fn set_block_state(&self, dimension: &str, pos: BlockPos, state: &str) -> bool {
        done(call(&block::SetState { dim: dimension.to_owned(), pos, state: state.to_owned() }))
    }

    fn world_is_thundering(&self, dimension: &str) -> bool {
        done(call(&world::IsThundering { dim: dimension.to_owned() }))
    }

    fn world_set_thunder(&self, dimension: &str, thundering: bool, duration_ticks: i32) -> bool {
        done(call(&world::SetThunder { dim: dimension.to_owned(), thundering, duration: duration_ticks }))
    }

    fn fill_region(&self, dimension: &str, from: BlockPos, to: BlockPos, state: &str) -> Option<u32> {
        call(&world::FillRegion { dim: dimension.to_owned(), from, to, state: state.to_owned() }).and_then(|n| u32::try_from(n).ok())
    }

    fn scan_region(&self, dimension: &str, from: BlockPos, to: BlockPos) -> Vec<(BlockPos, String)> {
        lines(call(&world::ScanRegion { dim: dimension.to_owned(), from, to }))
            .iter()
            .filter_map(|line| {
                let mut it = line.splitn(4, ' ');
                let (x, y, z) = (it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?);
                Some((BlockPos { x, y, z }, it.next()?.to_owned()))
            })
            .collect()
    }
}

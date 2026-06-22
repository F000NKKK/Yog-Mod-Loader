//! Event handlers registered by this mod.

use std::sync::atomic::{AtomicU64, Ordering};

use yog_api::player::Player;
use yog_api::world::World;
use yog_api::{info, BlockPos, Registry};

pub fn register(registry: &mut Registry) {
    registry.on_block_break(|e, srv| {
        info!(
            "[example-mod] {} broke {} at ({}, {}, {})",
            e.player_name, e.block_id, e.pos.x, e.pos.y, e.pos.z
        );
        World::new(srv, "minecraft:overworld").set_block(e.pos, "minecraft:glass");
    });

    registry.on_chat(|e, _srv| {
        info!("[example-mod] <{}> {}", e.player_name, e.message);
    });

    registry.on_player_join(|e, srv| {
        info!("[example-mod] {} joined ({})", e.player_name, e.uuid);
        srv.broadcast(&format!("Welcome, {}! (greeted by a Rust mod)", e.player_name));
    });

    registry.on_player_leave(|e, _srv| {
        info!("[example-mod] {} left", e.player_name);
    });

    registry.on_use_item(|e, srv| {
        if e.item_id == "minecraft:stick" {
            Player::new(srv, &e.player_name).give("minecraft:diamond", 1);
            info!("[example-mod] gave {} a diamond", e.player_name);
        }
    });

    registry.on_use_block(|e, _srv| {
        info!(
            "[example-mod] {} used {} at ({}, {}, {})",
            e.player_name, e.block_id, e.pos.x, e.pos.y, e.pos.z
        );
    });

    registry.on_attack_entity(|e, srv| {
        info!("[example-mod] {} attacked {} ({})", e.player_name, e.target_type, e.target_uuid);
        srv.broadcast(&format!("{} is fighting a {}!", e.player_name, e.target_type));
    });

    registry.on_entity_damage(|e, _srv| {
        info!(
            "[example-mod] {} took {:.1} damage from {}",
            e.entity_type, e.amount, e.source
        );
    });

    registry.on_entity_death(|e, srv| {
        info!("[example-mod] {} died (source: {})", e.entity_type, e.source);
        srv.broadcast(&format!("A {} has perished.", e.entity_type));
    });

    registry.on_tick(|srv| {
        static TICKS: AtomicU64 = AtomicU64::new(0);
        let n = TICKS.fetch_add(1, Ordering::Relaxed) + 1;
        if n % 1200 == 0 {
            srv.broadcast(&format!("Yog: {} minute(s) elapsed.", n / 1200));
        }
    });

    registry.on_server_started(|srv| {
        info!("[example-mod] server started — Yog is awake.");
        if let Some(block) =
            World::new(srv, "minecraft:overworld").get_block(BlockPos::new(0, 64, 0))
        {
            info!("[example-mod] block at (0, 64, 0) is {}", block);
        }
    });

    registry.on_server_stopping(|_srv| {
        info!("[example-mod] server stopping — the gate closes.");
    });

    // ── cancellable events ────────────────────────────────────────────────────

    // Block "minecraft:bedrock" is protected — cancel any attempt to break it.
    registry.on_block_break_pre(|e, _srv| {
        let allow = e.block_id != "minecraft:bedrock";
        if !allow {
            info!("[example-mod] blocked {} from breaking bedrock", e.player_name);
        }
        allow
    });

    // Filter profanity demo: cancel messages starting with "!block ".
    registry.on_chat_pre(|e, _srv| {
        let allow = !e.message.starts_with("!block ");
        if !allow {
            info!("[example-mod] suppressed message from {}", e.player_name);
        }
        allow
    });
}

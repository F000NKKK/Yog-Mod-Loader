//! Event handlers registered by this mod.

use std::sync::atomic::{AtomicU64, Ordering};

use yog_api::player::Player;
use yog_api::world::World;
use yog_api::{
    info, AdvancementEvent, BlockPos, CraftEvent, EntityInteractEvent, EntitySpawnEvent,
    EventPhase, ExplosionEvent, PlaceBlockEvent, PlayerDeathEvent, PlayerRespawnEvent, Registry,
};

pub fn register(registry: &mut Registry) {
    registry.on_block_break(|e, phase, srv| {
        match phase {
            EventPhase::Pre => {
                let allow = e.block_id != "minecraft:bedrock";
                if !allow {
                    info!("[example-mod] blocked {} from breaking bedrock", e.player_name);
                }
                allow
            }
            EventPhase::Post => {
                info!(
                    "[example-mod] {} broke {} at ({}, {}, {})",
                    e.player_name, e.block_id, e.pos.x, e.pos.y, e.pos.z
                );
                World::new(srv, "minecraft:overworld").set_block(e.pos, "minecraft:glass");
                true
            }
        }
    });

    registry.on_chat(|e, phase, _srv| {
        match phase {
            EventPhase::Pre => {
                let allow = !e.message.starts_with("!block ");
                if !allow {
                    info!("[example-mod] suppressed message from {}", e.player_name);
                }
                allow
            }
            EventPhase::Post => {
                info!("[example-mod] <{}> {}", e.player_name, e.message);
                true
            }
        }
    });

    registry.on_player_join(|e, _phase, srv| {
        info!("[example-mod] {} joined ({})", e.player_name, e.uuid);
        srv.broadcast(&format!("Welcome, {}! (greeted by a Rust mod)", e.player_name));
        true
    });

    registry.on_player_leave(|e, _phase, _srv| {
        info!("[example-mod] {} left", e.player_name);
        true
    });

    registry.on_use_item(|e, _phase, srv| {
        if e.item_id == "minecraft:stick" {
            Player::new(srv, &e.player_name).give("minecraft:diamond", 1);
            info!("[example-mod] gave {} a diamond", e.player_name);
        }
        true
    });

    registry.on_use_block(|e, _phase, _srv| {
        info!(
            "[example-mod] {} used {} at ({}, {}, {})",
            e.player_name, e.block_id, e.pos.x, e.pos.y, e.pos.z
        );
        true
    });

    registry.on_attack_entity(|e, _phase, srv| {
        info!("[example-mod] {} attacked {} ({})", e.player_name, e.target_type, e.target_uuid);
        srv.broadcast(&format!("{} is fighting a {}!", e.player_name, e.target_type));
        true
    });

    registry.on_entity_damage(|e, phase, _srv| {
        match phase {
            EventPhase::Pre => {
                let is_player_fall =
                    e.entity_type == "minecraft:player" && e.source.contains("fall");
                if is_player_fall {
                    info!("[example-mod] cancelled fall damage for player {}", e.uuid);
                }
                !is_player_fall
            }
            EventPhase::Post => {
                info!(
                    "[example-mod] {} took {:.1} damage from {}",
                    e.entity_type, e.amount, e.source
                );
                true
            }
        }
    });

    registry.on_entity_death(|e, _phase, srv| {
        info!("[example-mod] {} died (source: {})", e.entity_type, e.source);
        srv.broadcast(&format!("A {} has perished.", e.entity_type));
        true
    });

    registry.on_entity_spawn(|e: &EntitySpawnEvent, phase, _srv| {
        match phase {
            EventPhase::Pre => {
                let allow = e.entity_type != "minecraft:creeper";
                if !allow {
                    info!("[example-mod] cancelled creeper spawn in {}", e.dimension);
                }
                allow
            }
            EventPhase::Post => {
                info!(
                    "[example-mod] entity spawned: {} ({}) in {}",
                    e.entity_type, e.uuid, e.dimension
                );
                true
            }
        }
    });

    registry.on_player_place_block(|e: &PlaceBlockEvent, phase, _srv| {
        match phase {
            EventPhase::Pre => {
                info!(
                    "[example-mod] {} placing {} at ({}, {}, {})",
                    e.player_name, e.block_id, e.pos.x, e.pos.y, e.pos.z
                );
                true
            }
            EventPhase::Post => true,
        }
    });

    registry.on_player_death(|e: &PlayerDeathEvent, phase, srv| {
        match phase {
            EventPhase::Pre => {
                info!("[example-mod] {} is dying (source: {})", e.player_name, e.source);
                true
            }
            EventPhase::Post => {
                srv.broadcast(&format!("{} died ({}). F.", e.player_name, e.source));
                true
            }
        }
    });

    registry.on_player_respawn(|e: &PlayerRespawnEvent, _phase, srv| {
        srv.broadcast(&format!("Welcome back, {}!", e.player_name));
        info!("[example-mod] {} respawned (anchor: {})", e.player_name, e.at_anchor);
        true
    });

    registry.on_advancement(|e: &AdvancementEvent, _phase, srv| {
        info!(
            "[example-mod] {} earned advancement {}",
            e.player_name, e.advancement_id
        );
        srv.broadcast(&format!(
            "{} earned: {}",
            e.player_name, e.advancement_id
        ));
        true
    });

    registry.on_entity_interact(|e: &EntityInteractEvent, phase, srv| {
        match phase {
            EventPhase::Pre => {
                info!(
                    "[example-mod] {} interacting with {} ({}) using {}",
                    e.player_name, e.entity_type, e.entity_uuid, e.hand
                );
                true
            }
            EventPhase::Post => {
                srv.broadcast(&format!("{} interacted with a {}.", e.player_name, e.entity_type));
                true
            }
        }
    });

    registry.on_item_craft(|e: &CraftEvent, _phase, _srv| {
        info!(
            "[example-mod] {} crafted {}x {}",
            e.player_name, e.result_count, e.result_item
        );
        true
    });

    registry.on_explosion(|e: &ExplosionEvent, phase, srv| {
        match phase {
            EventPhase::Pre => {
                info!(
                    "[example-mod] explosion at ({:.1}, {:.1}, {:.1}) power={} in {}",
                    e.x, e.y, e.z, e.power, e.dimension
                );
                true
            }
            EventPhase::Post => {
                srv.broadcast(&format!(
                    "Explosion! ({:.0}, {:.0}, {:.0}) power={}",
                    e.x, e.y, e.z, e.power
                ));
                true
            }
        }
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
}

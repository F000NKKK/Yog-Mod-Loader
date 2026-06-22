//! Example Yog mod — built on its own into a `.yog` artifact via `yog build`.
//!
//! A mod author writes exactly this: depend on `yog-api`, implement [`Mod`], and
//! export it with [`export_mod!`]. The runtime dlopen's the resulting library at
//! startup and calls into it.

use std::sync::atomic::{AtomicU64, Ordering};

use yog_api::player::Player;
use yog_api::world::World;
use yog_api::{info, BlockDef, BlockPos, ItemDef, Mod, Registry};

pub struct ExampleMod;

impl Mod for ExampleMod {
    fn register(registry: &mut Registry) {
        // Custom content (registered before the game's registries freeze).
        registry.register_item(
            ItemDef::new("yog:ruby")
                .name("Ruby")
                .tooltip("A shiny gem, forged in Rust.")
                .max_stack(16),
        );
        registry.register_block(
            BlockDef::new("yog:ruby_block")
                .name("Block of Ruby")
                .strength(3.0, 6.0)
                // Custom hitbox: a smaller centred box (selection/collision differ
                // from the full-cube model — for a real shaped block ship a model
                // to match).
                .shape(3.0, 0.0, 3.0, 13.0, 13.0, 13.0),
        );

        registry.on_block_break(|e, srv| {
            info!(
                "[example-mod] {} broke {} at ({}, {}, {})",
                e.player_name, e.block_id, e.pos.x, e.pos.y, e.pos.z
            );
            // World access: replace the broken block with glass.
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

        // Right-click holding a stick -> get a diamond (event -> player action).
        registry.on_use_item(|e, srv| {
            if e.item_id == "minecraft:stick" {
                Player::new(srv, &e.player_name).give("minecraft:diamond", 1);
                info!("[example-mod] gave {} a diamond", e.player_name);
            }
        });

        // Right-click a block -> log which block was used.
        registry.on_use_block(|e, _srv| {
            info!(
                "[example-mod] {} used {} at ({}, {}, {})",
                e.player_name, e.block_id, e.pos.x, e.pos.y, e.pos.z
            );
        });

        // Attack an entity -> announce the target.
        registry.on_attack_entity(|e, srv| {
            info!("[example-mod] {} attacked {} ({})", e.player_name, e.target_type, e.target_uuid);
            srv.broadcast(&format!("{} is fighting a {}!", e.player_name, e.target_type));
        });

        // Entity damage -> log it.
        registry.on_entity_damage(|e, _srv| {
            info!(
                "[example-mod] {} took {:.1} damage from {}",
                e.entity_type, e.amount, e.source
            );
        });

        // Entity death -> announce it.
        registry.on_entity_death(|e, srv| {
            info!("[example-mod] {} died (source: {})", e.entity_type, e.source);
            srv.broadcast(&format!("A {} has perished.", e.entity_type));
        });

        // Periodic logic with shared state — announce every minute (1200 ticks).
        registry.on_tick(|srv| {
            static TICKS: AtomicU64 = AtomicU64::new(0);
            let n = TICKS.fetch_add(1, Ordering::Relaxed) + 1;
            if n % 1200 == 0 {
                srv.broadcast(&format!("Yog: {} minute(s) elapsed.", n / 1200));
            }
        });

        registry.on_server_started(|srv| {
            info!("[example-mod] server started — Yog is awake.");
            if let Some(block) = World::new(srv, "minecraft:overworld").get_block(BlockPos::new(0, 64, 0)) {
                info!("[example-mod] block at (0, 64, 0) is {}", block);
            }
        });

        registry.on_server_stopping(|_srv| {
            info!("[example-mod] server stopping — the gate closes.");
        });

        // Networking demo: client logs the server's packet and replies back.
        registry.on_client_packet("yog:pong", |e, srv| {
            info!(
                "[example-mod] client received pong: {}",
                String::from_utf8_lossy(&e.payload)
            );
            // client -> server reply
            srv.send_to_server("yog:ack", b"client got it");
        });

        // Server receives the client's reply.
        registry.on_packet("yog:ack", |e, _srv| {
            info!(
                "[example-mod] server got ack from {}: {}",
                e.player,
                String::from_utf8_lossy(&e.payload)
            );
        });

        // /ping -> server sends a raw-byte packet to the caller's client.
        registry.on_command("ping", |ctx, srv| {
            Player::new(srv, &ctx.source).send_packet("yog:pong", b"pong from server");
            Some("Sent a packet to your client.".into())
        });

        registry.on_command("yog", |ctx, _srv| {
            info!("[example-mod] /{} by {} args='{}'", ctx.name, ctx.source, ctx.args);
            Some(format!("Yog here, {}! You said: '{}'", ctx.source, ctx.args))
        });

        // /ruby -> give the caller our custom item.
        registry.on_command("ruby", |ctx, srv| {
            let ok = Player::new(srv, &ctx.source).give("yog:ruby", 1);
            Some(if ok { "Here's a ruby!".into() } else { "Failed.".into() })
        });

        // /heal -> set the caller's health via the player wrapper (entity layer).
        registry.on_command("heal", |ctx, srv| {
            let ok = Player::with_uuid(srv, &ctx.source, &ctx.uuid).set_health(20.0);
            Some(if ok { "Healed.".into() } else { "Failed (are you a living entity?).".into() })
        });

        // /pig -> read the caller's position (via player wrapper) and spawn a pig there.
        registry.on_command("pig", |ctx, srv| {
            match Player::with_uuid(srv, &ctx.source, &ctx.uuid).position() {
                Some((x, y, z)) => {
                    srv.spawn_entity("minecraft:pig", "minecraft:overworld", x, y, z);
                    Some("Oink!".into())
                }
                None => Some("No position (run as a player).".into()),
            }
        });

        // /tp -> teleport the caller to (0, 100, 0) via entity layer.
        registry.on_command("tp", |ctx, srv| {
            let ok = Player::with_uuid(srv, &ctx.source, &ctx.uuid).teleport(0.0, 100.0, 0.0);
            Some(if ok {
                "Teleported to (0, 100, 0).".into()
            } else {
                "Teleport failed (are you a player?).".into()
            })
        });
    }
}

yog_api::export_mod!(ExampleMod);

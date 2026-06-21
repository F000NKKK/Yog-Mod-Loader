//! A minimal example mod showing the Yog API surface.
//!
//! This is exactly what a mod author writes: depend on `yog-api`, implement
//! [`Mod`](yog_api::Mod), and register the events, world actions, and commands
//! you care about. Each handler also gets a [`Server`](yog_api::Server) handle
//! to act on the game.

use yog_api::world::World;
use yog_api::{info, BlockPos, Mod, Registry};

pub struct ExampleMod;

impl Mod for ExampleMod {
    fn register(registry: &mut Registry) {
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
            // Rust -> Minecraft: greet everyone from a Rust mod.
            srv.broadcast(&format!("Welcome, {}! (greeted by a Rust mod)", e.player_name));
        });

        registry.on_player_leave(|e, _srv| {
            info!("[example-mod] {} left", e.player_name);
        });

        registry.on_server_started(|srv| {
            info!("[example-mod] server started — Yog is awake.");
            // World read: peek at a block near spawn.
            if let Some(block) = World::new(srv, "minecraft:overworld").get_block(BlockPos::new(0, 64, 0)) {
                info!("[example-mod] block at (0, 64, 0) is {}", block);
            }
        });

        registry.on_server_stopping(|_srv| {
            info!("[example-mod] server stopping — the gate closes.");
        });

        // Command: /yog <args> -> replies to the caller, from Rust.
        registry.on_command("yog", |ctx, _srv| {
            info!("[example-mod] /{} by {} args='{}'", ctx.name, ctx.source, ctx.args);
            Some(format!(
                "Yog here, {}! You said: '{}'",
                ctx.source, ctx.args
            ))
        });
    }
}

/// Entry point the runtime calls.
///
/// In the MVP, mods are compiled into the runtime. Dynamic `.so`/`.dll` loading
/// via a stable C-ABI plugin interface is on the roadmap (stage 3).
pub fn register(registry: &mut Registry) {
    ExampleMod::register(registry);
}

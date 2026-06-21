//! A minimal example mod showing the Yog API surface.
//!
//! This is exactly what a mod author writes: depend on `yog-api`, implement
//! [`Mod`](yog_api::Mod), and subscribe to the events you care about. Each
//! handler also gets a [`Server`](yog_api::Server) handle to act on the game
//! (here: broadcasting a welcome message).

use yog_api::world::World;
use yog_api::{BlockPos, Mod, Registry};

pub struct ExampleMod;

impl Mod for ExampleMod {
    fn register(registry: &mut Registry) {
        registry.on_block_break(|e, srv| {
            println!(
                "[example-mod] {} broke {} at ({}, {}, {})",
                e.player_name, e.block_id, e.pos.x, e.pos.y, e.pos.z
            );
            // World access: replace the broken block with glass.
            World::new(srv, "minecraft:overworld").set_block(e.pos, "minecraft:glass");
        });

        registry.on_chat(|e, _srv| {
            println!("[example-mod] <{}> {}", e.player_name, e.message);
        });

        registry.on_player_join(|e, srv| {
            println!("[example-mod] {} joined ({})", e.player_name, e.uuid);
            // Rust -> Minecraft: greet everyone from a Rust mod.
            srv.broadcast(&format!("Welcome, {}! (greeted by a Rust mod)", e.player_name));
        });

        registry.on_player_leave(|e, _srv| {
            println!("[example-mod] {} left", e.player_name);
        });

        registry.on_server_started(|srv| {
            println!("[example-mod] server started — Yog is awake.");
            // World read: peek at a block near spawn.
            if let Some(block) = World::new(srv, "minecraft:overworld").get_block(BlockPos::new(0, 64, 0)) {
                println!("[example-mod] block at (0, 64, 0) is {}", block);
            }
        });

        registry.on_server_stopping(|_srv| {
            println!("[example-mod] server stopping — the gate closes.");
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

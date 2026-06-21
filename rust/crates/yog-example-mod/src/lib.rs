//! A minimal example mod showing the Yog API surface.
//!
//! This is exactly what a mod author writes: depend on `yog-api`, implement
//! [`Mod`](yog_api::Mod), and subscribe to the events you care about. Each
//! handler also gets a [`Server`](yog_api::Server) handle to act on the game
//! (here: broadcasting a welcome message).

use yog_api::{Mod, Registry};

pub struct ExampleMod;

impl Mod for ExampleMod {
    fn register(registry: &mut Registry) {
        registry.on_block_break(|e, _srv| {
            println!(
                "[example-mod] {} broke {} at ({}, {}, {})",
                e.player_name, e.block_id, e.pos.x, e.pos.y, e.pos.z
            );
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

        registry.on_server_started(|_srv| {
            println!("[example-mod] server started — Yog is awake.");
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

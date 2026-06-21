//! A minimal example mod showing the Yog API surface.
//!
//! This is exactly what a mod author writes: depend on `yog-api`, implement
//! [`Mod`], and subscribe to the events you care about.

use yog_api::{Mod, Registry};

pub struct ExampleMod;

impl Mod for ExampleMod {
    fn register(registry: &mut Registry) {
        registry.on_block_break(|e| {
            println!(
                "[example-mod] {} broke {} at ({}, {}, {})",
                e.player_name, e.block_id, e.pos.x, e.pos.y, e.pos.z
            );
        });

        registry.on_chat(|e| {
            println!("[example-mod] <{}> {}", e.player_name, e.message);
        });

        registry.on_player_join(|e| {
            println!("[example-mod] {} joined ({})", e.player_name, e.uuid);
        });

        registry.on_player_leave(|e| {
            println!("[example-mod] {} left", e.player_name);
        });

        registry.on_server_started(|| {
            println!("[example-mod] server started — Yog is awake.");
        });

        registry.on_server_stopping(|| {
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

//! Example Yog mod — built on its own into a `.yog` artifact via `yog build`.
//!
//! A mod author writes exactly this: depend on `yog-api`, implement [`Mod`], and
//! export it with [`export_mod!`]. The runtime dlopen's the resulting library at
//! startup and calls into it.

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
            srv.broadcast(&format!("Welcome, {}! (greeted by a Rust mod)", e.player_name));
        });

        registry.on_player_leave(|e, _srv| {
            info!("[example-mod] {} left", e.player_name);
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

        registry.on_command("yog", |ctx, _srv| {
            info!("[example-mod] /{} by {} args='{}'", ctx.name, ctx.source, ctx.args);
            Some(format!("Yog here, {}! You said: '{}'", ctx.source, ctx.args))
        });
    }
}

yog_api::export_mod!(ExampleMod);

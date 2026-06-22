//! Commands registered by this mod.

use yog_api::player::Player;
use yog_api::{info, Registry, Storage};

pub fn register(registry: &mut Registry) {
    registry.on_command("yog", |ctx, _srv| {
        info!("[example-mod] /{} by {} args='{}'", ctx.name, ctx.source, ctx.args);
        Some(format!("Yog here, {}! You said: '{}'", ctx.source, ctx.args))
    });

    // Give the caller a ruby.
    registry.on_command("ruby", |ctx, srv| {
        let ok = Player::new(srv, &ctx.source).give("yog:ruby", 1);
        Some(if ok { "Here's a ruby!".into() } else { "Failed.".into() })
    });

    // Heal to full via the player wrapper (entity layer).
    registry.on_command("heal", |ctx, srv| {
        let ok = Player::with_uuid(srv, &ctx.source, &ctx.uuid).set_health(20.0);
        Some(if ok { "Healed.".into() } else { "Failed (are you a living entity?).".into() })
    });

    // Spawn a pig at the caller's position.
    registry.on_command("pig", |ctx, srv| {
        match Player::with_uuid(srv, &ctx.source, &ctx.uuid).position() {
            Some((x, y, z)) => {
                srv.spawn_entity("minecraft:pig", "minecraft:overworld", x, y, z);
                Some("Oink!".into())
            }
            None => Some("No position (run as a player).".into()),
        }
    });

    // Teleport to (0, 100, 0) via entity layer.
    registry.on_command("tp", |ctx, srv| {
        let ok = Player::with_uuid(srv, &ctx.source, &ctx.uuid).teleport(0.0, 100.0, 0.0);
        Some(if ok {
            "Teleported to (0, 100, 0).".into()
        } else {
            "Teleport failed (are you a player?).".into()
        })
    });

    // Send a raw packet to the caller's client.
    registry.on_command("ping", |ctx, srv| {
        Player::new(srv, &ctx.source).send_packet("yog:pong", b"pong from server");
        Some("Sent a packet to your client.".into())
    });

    // Apply regeneration II for 5 seconds.
    registry.on_command("regen", |ctx, srv| {
        let ok = Player::with_uuid(srv, &ctx.source, &ctx.uuid)
            .add_effect("minecraft:regeneration", 100, 1, true);
        Some(if ok { "Regeneration applied!".into() } else { "Failed.".into() })
    });

    // Clear all effects.
    registry.on_command("clear_effects", |ctx, srv| {
        let ok = Player::with_uuid(srv, &ctx.source, &ctx.uuid).clear_effects();
        Some(if ok { "Effects cleared.".into() } else { "Failed.".into() })
    });

    // Check if the held item (hardcoded demo: stick) is in #minecraft:logs.
    registry.on_command("tag_check", |_ctx, srv| {
        let is_log = srv.has_block_tag("minecraft:oak_log", "minecraft:logs");
        Some(format!("oak_log in #minecraft:logs: {}", is_log))
    });

    // Roll the zombie loot table at (0, 64, 0).
    registry.on_command("loot", |_ctx, srv| {
        let ok = srv.drop_loot("minecraft:entities/zombie", "minecraft:overworld", 0.0, 64.0, 0.0);
        Some(if ok { "Loot dropped at (0, 64, 0).".into() } else { "Loot table empty or not found.".into() })
    });

    // Launch the caller into the air.
    registry.on_command("launch", |ctx, srv| {
        use yog_api::entity::Entity;
        let ok = Entity::new(srv, &ctx.uuid).add_velocity(0.0, 2.0, 0.0);
        Some(if ok { "Wheee!".into() } else { "Failed.".into() })
    });

    // Persistent coin balance demo.
    registry.on_command("coins", |ctx, srv| {
        let mut store = Storage::open(&srv.game_dir(), "yog:economy");
        let balance: i64 = store.get(&ctx.source).and_then(|v| v.parse().ok()).unwrap_or(0);
        // Award 10 coins each time.
        let new_balance = balance + 10;
        store.set(&ctx.source, new_balance.to_string());
        store.save().ok();
        Some(format!("Coins: {} (+10)", new_balance))
    });

    // Send a title screen to the caller.
    registry.on_command("title", |ctx, srv| {
        let ok = Player::new(srv, &ctx.source)
            .send_title("\u{a7}6Yog Loaded!", "\u{a7}7This is Rust in Minecraft.", 10, 70, 20);
        Some(if ok { "Title sent.".into() } else { "Failed (are you a player?).".into() })
    });

    // Send an action-bar message.
    registry.on_command("bar", |ctx, srv| {
        let ok = Player::new(srv, &ctx.source)
            .send_actionbar("\u{a7}aYog action bar!");
        Some(if ok { "Action-bar sent.".into() } else { "Failed.".into() })
    });

    // Play a level-up sound at the caller's position.
    registry.on_command("sound", |ctx, srv| {
        let ok = Player::new(srv, &ctx.source)
            .play_sound("minecraft:entity.player.levelup", 1.0, 1.0);
        Some(if ok { "Sound played.".into() } else { "Failed.".into() })
    });

    // Switch to creative mode.
    registry.on_command("creative", |ctx, srv| {
        let ok = Player::new(srv, &ctx.source).set_gamemode("creative");
        Some(if ok { "Creative mode.".into() } else { "Failed.".into() })
    });

    // Switch to survival mode.
    registry.on_command("survival", |ctx, srv| {
        let ok = Player::new(srv, &ctx.source).set_gamemode("survival");
        Some(if ok { "Survival mode.".into() } else { "Failed.".into() })
    });

    // Boss-bar demo: create a progress bar and add the caller to it.
    registry.on_command("boss", |ctx, srv| {
        let bid = "yog:demo";
        srv.bossbar_create(bid, "\u{a7}6Yog Demo", "yellow", "notched_10");
        srv.bossbar_set_progress(bid, 0.75);
        srv.bossbar_add_player(bid, &ctx.source);
        Some("Boss bar shown.".into())
    });

    // Remove the demo boss bar entirely.
    registry.on_command("unboss", |ctx, srv| {
        let bid = "yog:demo";
        srv.bossbar_remove_player(bid, &ctx.source);
        srv.bossbar_remove(bid);
        Some("Boss bar removed.".into())
    });

    // Show current world time.
    registry.on_command("time", |_ctx, srv| {
        use yog_api::world::World;
        let w = World::new(srv, "minecraft:overworld");
        match w.time() {
            Some(t) => Some(format!("World time: {} ticks ({})", t, t % 24000)),
            None => Some("Dimension not found.".into()),
        }
    });

    // Toggle rain.
    registry.on_command("weather", |_ctx, srv| {
        use yog_api::world::World;
        let w = World::new(srv, "minecraft:overworld");
        let raining = w.is_raining();
        w.set_weather(!raining, 6000);
        Some(if raining { "Rain stopped.".into() } else { "Rain started.".into() })
    });

    // List online players.
    registry.on_command("players", |_ctx, srv| {
        let list = srv.online_players();
        if list.is_empty() {
            Some("No players online.".into())
        } else {
            Some(format!("Online ({}): {}", list.len(), list.join(", ")))
        }
    });

    // ── typed commands & new features ─────────────────────────────────────────

    // /tp_dim <dim_word> <x> <y> <z>  — cross-dimension teleport
    registry.on_typed_command("tp_dim", "word int int int", |ctx, srv| {
        let dim = ctx.arg_str(0).unwrap_or("minecraft:overworld");
        let x   = ctx.arg_int(1).unwrap_or(0) as f64;
        let y   = ctx.arg_int(2).unwrap_or(64) as f64;
        let z   = ctx.arg_int(3).unwrap_or(0) as f64;
        let ok = Player::new(srv, &ctx.source).teleport_to_dim(dim, x, y, z);
        Some(if ok {
            format!("Teleported to {} ({}, {}, {})", dim, x, y, z)
        } else {
            "Failed (player offline or unknown dimension).".into()
        })
    });

    // /give_slot <slot:int> <item:word>  — put one item in a specific slot
    registry.on_typed_command("give_slot", "int word", |ctx, srv| {
        let slot    = ctx.arg_int(0).unwrap_or(0) as u32;
        let item_id = ctx.arg_str(1).unwrap_or("minecraft:stone");
        let ok = Player::new(srv, &ctx.source).set_slot(slot, item_id, 1);
        Some(if ok { format!("Placed {} in slot {}", item_id, slot) } else { "Failed.".into() })
    });

    // /inv  — list the first 8 occupied slots in your inventory
    registry.on_command("inv", |ctx, srv| {
        let slots = Player::new(srv, &ctx.source).inventory();
        if slots.is_empty() {
            return Some("Your inventory is empty.".into());
        }
        let list: String = slots.iter().take(8)
            .map(|(slot, id, count)| format!("[{}] {}×{}", slot, id, count))
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!("Inventory: {}", list))
    });

    // /nbt <x:int> <y:int> <z:int>  — show SNBT of a block entity
    registry.on_typed_command("nbt", "int int int", |ctx, srv| {
        use yog_api::BlockPos;
        let x = ctx.arg_int(0).unwrap_or(0);
        let y = ctx.arg_int(1).unwrap_or(64);
        let z = ctx.arg_int(2).unwrap_or(0);
        let nbt = srv.get_block_nbt("minecraft:overworld", BlockPos::new(x, y, z));
        Some(nbt.unwrap_or_else(|| format!("No block entity at ({}, {}, {})", x, y, z)))
    });

    // /mob_count <type:word>  — count loaded entities of a type in the overworld
    registry.on_typed_command("mob_count", "word", |ctx, srv| {
        use yog_api::world::World;
        let type_id = ctx.arg_str(0).unwrap_or("minecraft:zombie");
        let count = World::new(srv, "minecraft:overworld").entity_count(type_id);
        if count < 0 {
            Some(format!("Unknown entity type or dimension: {}", type_id))
        } else {
            Some(format!("Loaded {} of {} in overworld", count, type_id))
        }
    });
}

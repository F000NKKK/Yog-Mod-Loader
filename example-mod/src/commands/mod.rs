//! Commands registered by this mod.

use yog_api::player::Player;
use yog_api::{info, Registry};

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
}

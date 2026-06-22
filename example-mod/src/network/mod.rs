//! Network packet handlers.

use yog_api::{info, Registry};

pub fn register(registry: &mut Registry) {
    // Client receives a "pong" packet from the server and replies back.
    registry.on_client_packet("yog:pong", |e, srv| {
        info!(
            "[example-mod] client received pong: {}",
            String::from_utf8_lossy(&e.payload)
        );
        srv.send_to_server("yog:ack", b"client got it");
    });

    // Server receives the client's ack.
    registry.on_packet("yog:ack", |e, _srv| {
        info!(
            "[example-mod] server got ack from {}: {}",
            e.player,
            String::from_utf8_lossy(&e.payload)
        );
    });
}

//! The handle Rust mods use to act on the running server — the Rust → Minecraft
//! path.

/// Capabilities a mod can call (e.g. from inside an event handler) to affect the
/// running game.
///
/// The Yog runtime provides the concrete implementation, backed by JNI calls
/// into the Java host. This crate itself stays JVM-free.
///
/// More capabilities (world access, commands, networking) are added as those
/// domains land.
pub trait Server {
    /// Broadcast a chat message to all players on the server.
    fn broadcast(&self, message: &str);
}

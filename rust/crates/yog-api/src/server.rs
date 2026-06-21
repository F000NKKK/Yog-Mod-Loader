//! The handle Rust mods use to act on the server — the Rust → Minecraft path.

/// Capabilities a mod can call (e.g. from inside an event handler) to affect the
/// running game.
///
/// `yog-api` stays JVM-free: this is only a trait. The Yog runtime provides the
/// concrete implementation, backed by JNI calls into the Java host.
pub trait Server {
    /// Broadcast a chat message to all players on the server.
    fn broadcast(&self, message: &str);
}

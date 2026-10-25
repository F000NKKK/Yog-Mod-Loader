//! Sending over the network. To a player (server side) always; to the
//! server (client side) only when the mod says so in `yog.toml`:
//!
//! ```toml
//! [network]
//! to_server = true
//! ```
//!
//! Then it needs Yog on the server, and `yog build` builds it with this
//! crate's `to-server` feature. A mod that sends without saying so doesn't
//! compile: the channel it names isn't a [`ServerChannel`].
//!
//! ```compile_fail
//! // Without `to-server`: "this mod sends to the server without saying so".
//! yog_api::net::send_to_server("mymod:hello", vec![1u8]);
//! ```

pub use yog_event::net::{SendToPlayer, send_to_player};

/// A channel the mod sends to the server on: any name, once the mod says
/// it sends there (`[network] to_server = true`, the `to-server` feature).
#[diagnostic::on_unimplemented(
    message = "this mod sends to the server without saying so",
    label = "sent to the server here",
    note = "say it in yog.toml — `[network]` `to_server = true`: the mod then needs Yog on the server too"
)]
pub trait ServerChannel: yog_event::Arg<String> {}

#[cfg(feature = "to-server")]
impl ServerChannel for &str {}
#[cfg(feature = "to-server")]
impl ServerChannel for String {}

/// Sends `payload` on `channel` to the server (client side): whether it
/// went (a server without Yog has no channel to take it).
pub fn send_to_server(channel: impl ServerChannel, payload: impl yog_event::Arg<Vec<u8>>) -> bool {
    yog_event::net::send_to_server(channel, payload)
}

//! The bus's message machinery: a message and its address, its
//! arguments, events and services as kinds of message, calls, the handles
//! (entity, player, world) messages carry, the phases an event runs in.

#[macro_use]
pub(crate) mod message;
pub(crate) mod address;
pub(crate) mod arg;
pub(crate) mod call;
pub(crate) mod event;
pub(crate) mod handles;
pub(crate) mod phase;
pub(crate) mod service;
#[cfg(test)]
mod tests;

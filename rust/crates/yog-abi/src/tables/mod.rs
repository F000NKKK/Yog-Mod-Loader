//! The tables that cross the boundary: what a mod registers with, the bus, the GPU pipeline,
//! the byte sinks, mirrors' shared memory.

pub(crate) mod api;
pub(crate) mod bus;
pub(crate) mod gfx;
pub(crate) mod handlers;
pub(crate) mod mirror;

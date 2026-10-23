//! The bus as a mod sees it: events (one subscription for every kind),
//! services, other mods' exported functions, drawing in render events.

pub(crate) mod bus;
pub(crate) mod draw;
pub(crate) mod interop;
pub(crate) mod service;

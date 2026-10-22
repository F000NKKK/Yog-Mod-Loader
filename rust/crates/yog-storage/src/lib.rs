//! Where a mod keeps its data: typed values by key, in stores scoped the
//! way web storage is — a session's (while the game runs), the game's
//! (the install, whatever world), the world's and its parts' (a dimension,
//! a player, an entity, a chunk).
//!
//! ```no_run
//! use yog_storage::{Scope, Store};
//! # #[derive(yog_data::Schema, Default)] struct Board { tier: u8 }
//! # let world = std::path::Path::new("/path/to/world");
//! let boards = Store::open(world, "mymod", Scope::World);
//! boards.set("alu-1", &Board { tier: 2 });
//! let board: Option<Board> = boards.get("alu-1");
//! yog_storage::flush_all().unwrap(); // the world being saved
//! ```
//!
//! Values are any [`yog_data::Schema`] type, kept as Yog Data. A store is
//! read from its file once a process and shared by every handle to it;
//! changes are written by [`flush_all`] (as the world is saved — `yog-api`
//! does that) or a store's own `flush`, atomically: to a temporary file,
//! then renamed over the store.
//!
//! # File layout
//! ```text
//! <game>/yog-data/<mod>/game.ydata
//! <world>/yog-data/<mod>/world.ydata
//! <world>/yog-data/<mod>/dimension/<dim>.ydata
//! <world>/yog-data/<mod>/player/<uuid>.ydata
//! <world>/yog-data/<mod>/entity/<uuid>.ydata
//! <world>/yog-data/<mod>/chunk/<dim>_<cx>_<cz>.ydata
//! ```

mod scope;
mod shelf;
mod store;

pub use scope::Scope;
pub use shelf::{close_under, flush_all};
pub use store::Store;

#[cfg(test)]
mod tests;

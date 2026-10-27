//! Where the mod keeps its data — typed values in stores scoped the way web
//! storage is (`yog-storage`), opened for this mod in the right folder:
//!
//! ```ignore
//! use yog_api::storage;
//!
//! storage::game().set("theme", &theme);              // the install, whatever world
//! if let Some(world) = storage::world() {            // this world, saved with it
//!     world.update("visits", |n: &mut u32| *n += 1);
//! }
//! storage::session().set("last_screen", &screen);    // while the game runs
//! ```
//!
//! Or a value kept for the mod, loaded when first used and saved with the
//! world ([`WorldData`], [`PlayerData`], [`GameData`]):
//!
//! ```ignore
//! static LABELS: WorldData<Vec<Label>> = WorldData::new("labels");
//! LABELS.with_mut(|labels| labels.push(label));
//! ```
//!
//! What's kept with the world is written as the world is saved
//! (`server::life::Saving`: an autosave, `/save-all`, the server stopping),
//! so it's saved with the world it belongs to.
//!
//! A value kept for each owner — [`PlayerData`], [`BlockData`] (gone as
//! it's broken) — is in the owner's store, at the keys a Yog Script
//! object's `saved` members have; one kept on an entity ([`EntityData`]) or
//! a stack ([`ItemData`]) is the game's, travelling with it.

mod entity;
mod item;
mod kept;
mod records;

use std::path::PathBuf;

use yog_sync::Mutex;
use yog_sync::atomic::{AtomicBool, Ordering};

pub use entity::EntityData;
pub use item::ItemData;
pub use kept::{BlockData, GameData, PlayerData, Was, WorldData};
#[doc(hidden)]
pub use records::__provide_records;
pub use yog_storage::{Scope, Store, close_under, flush_all};

use crate::Registry;
use crate::event::block::{Break, Place};
use crate::event::server::life::{Saving, Started, Stopping};
use kept::Gone;

/// The open world's folder (none on a client of another's server, or at the title screen).
static WORLD: Mutex<Option<PathBuf>> = Mutex::new(None);
/// The server is stopping: the next save is the world's last.
static CLOSING: AtomicBool = AtomicBool::new(false);

/// This mod's session store: kept while the game runs, never written.
pub fn session() -> Store {
    Store::session(crate::mod_id())
}

/// This mod's store for the install, whatever world.
pub fn game() -> Store {
    Store::open(&game_dir(), crate::mod_id(), Scope::Game)
}

/// This mod's store for the open world.
pub fn world() -> Option<Store> {
    in_world(Scope::World)
}

/// This mod's store for dimension `dim` of the open world.
pub fn dimension(dim: &str) -> Option<Store> {
    in_world(Scope::Dimension(dim))
}

/// This mod's store for player `uuid` in the open world.
pub fn player(uuid: &str) -> Option<Store> {
    in_world(Scope::Player(uuid))
}

/// This mod's store for entity `uuid` in the open world.
pub fn entity(uuid: &str) -> Option<Store> {
    in_world(Scope::Entity(uuid))
}

/// This mod's store for chunk `cx`, `cz` of dimension `dim` in the open world.
pub fn chunk(dim: &str, cx: i32, cz: i32) -> Option<Store> {
    in_world(Scope::Chunk(dim, cx, cz))
}

/// The server runs in this game: its world is open here.
fn world_open() -> bool {
    WORLD.lock().is_some()
}

fn in_world(scope: Scope<'_>) -> Option<Store> {
    let root = WORLD.lock().clone()?;
    Some(Store::open(&root, crate::mod_id(), scope))
}

/// The game's folder (the working folder when the host can't say).
fn game_dir() -> PathBuf {
    crate::game::dir().map(PathBuf::from).unwrap_or_default()
}

/// The world's folder known as it starts — registered by `export_mod!`
/// before the mod's own handlers, so theirs find its stores.
#[doc(hidden)]
pub fn __know_world(registry: &mut Registry) {
    registry.on::<Started, _>(|e, _| {
        *WORLD.lock() = Some(PathBuf::from(&e.world_dir));
    });
    // A block put in place starts fresh: what the one before kept is gone
    // before the mod's handlers set its own.
    registry.on::<Place, _>(|e, _| kept::gone(&Gone::Block(&e.dimension, e.pos)));
}

/// What's kept saved with the world — registered by `export_mod!` after
/// the mod's own handlers, so what theirs put in the stores is written.
#[doc(hidden)]
pub fn __save_with_world(registry: &mut Registry) {
    registry.on::<Stopping, _>(|_, _| CLOSING.store(true, Ordering::Release));
    registry.on::<Saving, _>(|_, _| saving());
    // Gone after the mod's handlers: theirs still read what it kept.
    registry.on::<Break, _>(|e, _| kept::gone(&Gone::Block(&e.dimension, e.pos)));
    kept::synced::listen(registry);
}

/// The world being saved: what's kept written; the world's last save closes it.
fn saving() {
    kept::keep_all();
    if let Err(e) = flush_all() {
        crate::error!("[{}] couldn't save its data: {e}", crate::mod_id());
    }
    if !CLOSING.swap(false, Ordering::AcqRel) {
        return;
    }
    kept::forget_world();
    if let Some(root) = WORLD.lock().take()
        && let Err(e) = close_under(&root)
    {
        crate::error!("[{}] couldn't save its data: {e}", crate::mod_id());
    }
}

/// Tests that open a world, or count on none open, take turns.
#[cfg(test)]
static WORLD_TESTS: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests;

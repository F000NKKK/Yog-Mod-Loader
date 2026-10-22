//! What a store belongs to, and so where its file is.

use std::path::{Path, PathBuf};

/// What a store belongs to — under a root: the game's folder for
/// [`Scope::Game`], the world's for the rest (see [the crate docs](crate)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope<'a> {
    /// The install, whatever world (settings, what a player made to keep).
    Game,
    /// The world as a whole.
    World,
    /// One dimension of the world, by its id (`minecraft:overworld`).
    Dimension(&'a str),
    /// One player, by uuid.
    Player(&'a str),
    /// One entity, by uuid.
    Entity(&'a str),
    /// One chunk: its dimension and coordinates.
    Chunk(&'a str, i32, i32),
}

use yog_ids::storage::{CHUNK, DIMENSION, DIR, ENTITY, GAME, PLAYER, SEP, WORLD};

/// A store's file.
const STORE_EXT: &str = yog_ids::Format::Data.ext();

impl Scope<'_> {
    /// The file of `mod_id`'s store of this scope under `root`.
    pub fn path(self, root: &Path, mod_id: &str) -> PathBuf {
        let base = root.join(DIR).join(file_safe(mod_id));
        let (dir, name) = match self {
            Scope::Game => (None, GAME.to_owned()),
            Scope::World => (None, WORLD.to_owned()),
            Scope::Dimension(dim) => (Some(DIMENSION), file_safe(dim)),
            Scope::Player(uuid) => (Some(PLAYER), file_safe(uuid)),
            Scope::Entity(uuid) => (Some(ENTITY), file_safe(uuid)),
            Scope::Chunk(dim, x, z) => (Some(CHUNK), format!("{}{SEP}{x}{SEP}{z}", file_safe(dim))),
        };
        let file = format!("{name}{}{STORE_EXT}", yog_ids::sep::EXT);
        match dir {
            Some(dir) => base.join(dir).join(file),
            None => base.join(file),
        }
    }
}

/// `name` (`ns:path/x`) as one file name: its separators replaced.
fn file_safe(name: &str) -> String {
    name.replace(yog_ids::sep::NAMESPACE, SEP)
        .replace(yog_ids::sep::PATH, SEP)
}

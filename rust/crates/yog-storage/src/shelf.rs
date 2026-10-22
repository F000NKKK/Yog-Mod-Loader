//! The stores open in this process: one each, however many handles; read
//! from their file once, written back when asked ([`flush_all`], a store's
//! `flush`) — atomically, so a crash mid-save leaves the old one.

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use yog_sync::{Arc, Mutex};

/// One store's values, and whether they changed since they were written.
#[derive(Default)]
pub(crate) struct Shelf {
    /// Its file; none for a session's (never written).
    pub(crate) path: Option<PathBuf>,
    pub(crate) values: yog_data::Map,
    pub(crate) dirty: bool,
}

/// Which store: a file's, or a mod's session.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    File(PathBuf),
    Session(String),
}

type Shelves = HashMap<Key, Arc<Mutex<Shelf>>>;

static SHELVES: Mutex<Option<Shelves>> = Mutex::new(None);

/// The store in the file at `path`, read the first time.
pub(crate) fn file(path: PathBuf) -> Arc<Mutex<Shelf>> {
    open(Key::File(path.clone()), || Shelf {
        values: read(&path),
        path: Some(path),
        dirty: false,
    })
}

/// `mod_id`'s session store.
pub(crate) fn session(mod_id: &str) -> Arc<Mutex<Shelf>> {
    open(Key::Session(mod_id.to_owned()), Shelf::default)
}

fn open(key: Key, new: impl FnOnce() -> Shelf) -> Arc<Mutex<Shelf>> {
    let mut shelves = SHELVES.lock();
    let shelves = shelves.get_or_insert_with(HashMap::new);
    shelves
        .entry(key)
        .or_insert_with(|| Arc::new(Mutex::new(new())))
        .clone()
}

/// Writes every store changed since it was last written; the first failure.
pub fn flush_all() -> io::Result<()> {
    let open: Vec<_> = SHELVES
        .lock()
        .iter()
        .flat_map(|s| s.values().cloned())
        .collect();
    let mut result = Ok(());
    for shelf in open {
        if let Err(e) = shelf.lock().flush()
            && result.is_ok()
        {
            result = Err(e);
        }
    }
    result
}

/// Writes and closes every store under `root` (the world closed: the next
/// one opened reads its own).
pub fn close_under(root: &Path) -> io::Result<()> {
    let closing: Vec<_> = {
        let mut shelves = SHELVES.lock();
        let Some(shelves) = shelves.as_mut() else {
            return Ok(());
        };
        let keys: Vec<Key> = shelves
            .keys()
            .filter(|k| matches!(k, Key::File(p) if p.starts_with(root)))
            .cloned()
            .collect();
        keys.iter().filter_map(|k| shelves.remove(k)).collect()
    };
    let mut result = Ok(());
    for shelf in closing {
        if let Err(e) = shelf.lock().flush()
            && result.is_ok()
        {
            result = Err(e);
        }
    }
    result
}

impl Shelf {
    /// Writes it if it changed since it was last written (a session's never is).
    pub(crate) fn flush(&mut self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if !self.dirty {
            return Ok(());
        }
        write(path, &self.values)?;
        self.dirty = false;
        Ok(())
    }
}

/// What a store's written to before it's renamed over the old one.
const TMP_EXT: &str = yog_ids::concatcp!(yog_ids::Format::Data.ext(), yog_ids::sep::DOT, "tmp");

fn write(path: &Path, values: &yog_data::Map) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(TMP_EXT);
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&yog_data::binary::encode(&values.clone().into()))?;
        f.flush()?;
    }
    std::fs::rename(&tmp, path)
}

/// The values in the file at `path`: none when there's no such file or it
/// isn't a store.
fn read(path: &Path) -> yog_data::Map {
    let Ok(bytes) = std::fs::read(path) else {
        return yog_data::Map::new();
    };
    match yog_data::binary::decode(&bytes) {
        Ok(yog_data::Value::Map(m)) => m,
        _ => yog_data::Map::new(),
    }
}

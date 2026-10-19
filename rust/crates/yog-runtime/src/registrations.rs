//! Hot reload and what can't be reloaded: each mod's registrations when
//! the game started (its `.yog`'s `registrations.txt`) — a new `.yog` that
//! adds, removes or changes some needs the game restarted for them — and
//! the game's own reload of resources and data after new code is in.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;

use yog_registry::REGISTRATIONS_FILE;
use yog_sync::Locked;

/// Mod id → its registrations when the game started.
static STARTUP: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// `name` in the `.yog` at `path`, as text.
fn read_entry(path: &Path, name: &str) -> Option<String> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let mut text = String::new();
    archive.by_name(name).ok()?.read_to_string(&mut text).ok()?;
    Some(text)
}

/// Keeps what mod `mod_id` registered at the game's start (its `.yog`).
pub(crate) fn record(mod_id: &str, yog: &Path) {
    let text = read_entry(yog, REGISTRATIONS_FILE).unwrap_or_default();
    STARTUP.locked().get_or_insert_with(HashMap::new).insert(mod_id.to_string(), text);
}

/// What the `.yog` at `new_yog` registers that the game, as started,
/// doesn't have as it says: added, removed, changed — `<kind> <id>` each.
pub(crate) fn changes(mod_id: &str, new_yog: &Path) -> (Vec<String>, Vec<String>, Vec<String>) {
    let started = STARTUP.locked().as_ref().and_then(|m| m.get(mod_id).cloned()).unwrap_or_default();
    let new = read_entry(new_yog, REGISTRATIONS_FILE).unwrap_or_default();
    yog_registry::registration_changes(&started, &new)
}

/// After new code is in: the game reloads its data (`/reload`: recipes,
/// tags, loot) and, on a client, its resources (F3+T: textures, models,
/// sounds, lang) — both read the mods' new `.yog`s.
pub(crate) fn reload_game_resources() {
    // SAFETY: static host methods taking nothing; the JVM env is this thread's.
    unsafe {
        let _: Option<()> = host!("dev/yog/YogDevReload", "reloadData", []);
        let _: Option<()> = host!("dev/yog/YogDevReloadClient", "reloadResources", []);
    }
}

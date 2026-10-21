//! What mods declare for the host to set up — commands, menu entries, key
//! bindings — kept by the runtime and provided as services: mods declare,
//! the host reads what's declared.

use yog_sync::Mutex;

use yog_event::{command, key_binding, menu};

use super::duties::provide;

static COMMANDS: Mutex<Vec<command::CommandSpec>> = Mutex::new(Vec::new());
static MENU: Mutex<Vec<menu::MenuEntry>> = Mutex::new(Vec::new());
static KEY_BINDINGS: Mutex<Vec<key_binding::KeyBindingSpec>> = Mutex::new(Vec::new());

/// Adds `item` to `list` unless one `same` as it is there — a hot reload
/// declares a mod's things again, and the game keeps the first.
fn keep<T: Clone>(list: &Mutex<Vec<T>>, item: &T, same: impl Fn(&T, &T) -> bool) -> bool {
    let mut all = list.lock();
    if !all.iter().any(|have| same(have, item)) {
        all.push(item.clone());
    }
    true
}

/// A key binding's category label: its mod's name, its namespace's when no mod has it.
fn category(id: &str) -> String {
    let ns = id.split_once(':').map_or(id, |(ns, _)| ns);
    let infos = crate::globals::MOD_INFOS.lock();
    infos
        .iter()
        .find(|m| m[0] == ns)
        .map_or_else(|| ns.to_owned(), |m| m[1].clone())
}

/// The declaration services; once, at start.
pub(crate) fn register() {
    provide::<command::CommandSpec>(|d| keep(&COMMANDS, d, PartialEq::eq));
    provide::<command::Declared>(|_| COMMANDS.lock().clone());
    provide::<menu::MenuEntry>(|d| keep(&MENU, d, PartialEq::eq));
    provide::<menu::Declared>(|_| MENU.lock().clone());
    provide::<key_binding::KeyBindingSpec>(|d| keep(&KEY_BINDINGS, d, |a, b| a.id == b.id));
    provide::<key_binding::Declared>(|_| {
        let all = KEY_BINDINGS.lock().clone();
        all.into_iter()
            .map(|b| key_binding::CategorizedBinding {
                category: category(&b.id),
                id: b.id,
                name: b.name,
                default_key: b.default_key,
            })
            .collect()
    });
}

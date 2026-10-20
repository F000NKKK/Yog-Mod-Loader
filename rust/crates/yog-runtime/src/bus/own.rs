//! The runtime's own services: what only it knows — the content mods
//! register, the mods it loaded, what this client was told about entities.

use yog_event::{client, content, entity, game};
use yog_sync::Locked;

use super::duties::provide;

/// Which list a mod is on: Yog's own, or the loader's (Java mods).
const YOG: &str = "yog";
const PLATFORM: &str = "platform";

/// A Yog mod's `.yres` name or description in the player's language, else as written.
fn localized(text: &str, key: &str) -> String {
    crate::translate(key).unwrap_or_else(|| text.to_owned())
}

/// Every installed mod: the Yog mods loaded, then the loader's.
fn mods() -> Vec<game::ModInfo> {
    let infos = crate::MOD_INFOS.locked().clone();
    let mut out: Vec<game::ModInfo> = infos
        .iter()
        .map(|m| game::ModInfo {
            source: YOG.to_owned(),
            id: m[0].clone(),
            name: localized(&m[1], &m[5]),
            version: m[2].clone(),
            authors: m[3].clone(),
            description: localized(&m[4], &m[6]),
        })
        .collect();
    // The loader's mods, `id\tname\tversion\tauthors\tdescription` a line.
    let platform: Option<String> = unsafe { bridge!("listPlatformMods", []) };
    for line in platform.iter().flat_map(|s| s.lines()).filter(|l| !l.trim().is_empty()) {
        let mut f = line.split(yog_ids::sep::FIELD).map(str::to_owned);
        let mut next = || f.next().unwrap_or_default();
        out.push(game::ModInfo { source: PLATFORM.to_owned(), id: next(), name: next(), version: next(), authors: next(), description: next() });
    }
    out
}

/// The runtime's own services; once, at start.
pub(crate) fn register() {
    provide::<content::Register>(|r| {
        crate::register_blob(&mut crate::handlers_write(), &r.kind, r.id.clone(), &r.data);
        true
    });
    provide::<game::Mods>(|_| mods());
    provide::<game::ModLoaded>(|m| crate::mod_loaded(&m.id));
    provide::<client::EntityData>(|d| crate::entity_data::client_get(&d.uuid, &d.key).map(|bytes| entity::DataValue { bytes }));
}

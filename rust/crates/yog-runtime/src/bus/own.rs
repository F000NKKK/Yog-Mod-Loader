//! The runtime's own services: what only it knows — the content mods
//! register, the mods it loaded, what this client was told about entities.

use yog_event::{client, content, entity, game, media};

use super::duties::provide;

/// A Yog mod's `.yres` name or description in the player's language, else as written.
fn localized(text: &str, key: Option<&str>) -> String {
    key.and_then(crate::machine_api::translate)
        .unwrap_or_else(|| text.to_owned())
}

/// Every installed mod: the Yog mods loaded, then the loader's.
fn mods() -> Vec<game::ModInfo> {
    let infos = crate::globals::MOD_INFOS.lock().clone();
    let mut out: Vec<game::ModInfo> = infos
        .iter()
        .map(|m| game::ModInfo {
            source: yog_ids::mod_source::YOG.to_owned(),
            id: m.id.clone(),
            name: localized(&m.name, m.name_key.as_deref()),
            version: m.version.clone(),
            authors: m.authors.join(", "),
            description: localized(&m.description, m.description_key.as_deref()),
        })
        .collect();
    out.extend(super::call(&game::PlatformMods {}).unwrap_or_default());
    out
}

/// The runtime's own services; once, at start.
pub(crate) fn register() {
    provide::<content::Register>(|r| {
        crate::api_fns::register_blob(
            &mut crate::handlers::handlers_write(),
            &r.kind,
            r.id.clone(),
            &r.data,
        );
        true
    });
    provide::<game::Mods>(|_| mods());
    provide::<game::ModLoaded>(|m| crate::machine_api::mod_loaded(&m.id));
    provide::<media::Png>(png);
    provide::<client::EntityData>(|d| {
        crate::entity_data::client_get(&d.uuid, &d.key).map(|bytes| entity::DataValue { bytes })
    });
}

/// A `.yog`'s yimage as the PNG the game reads (none when it isn't one).
fn png(p: &media::Png) -> media::PngBytes {
    match yog_media::yimage::to_png(&p.yimage) {
        Ok(png) => media::PngBytes { png },
        Err(e) => {
            yog_logging::warn!("a texture the game asked for: {e}");
            media::PngBytes::default()
        }
    }
}

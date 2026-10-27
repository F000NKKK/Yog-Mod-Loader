//! A sound for a designer to play: the mod's own (`@res:Sounds.abacus1`, its
//! `.yres` bytes as given) or a file the game has (`minecraft:dig/sand1`,
//! from its assets on this machine) — the variants of a `.ysound`.

use std::path::Path;

use base64::Engine;

use crate::design::sources::project::mod_root;
use crate::serve::ServiceError;

/// What a sound file's bytes start with, and what it is then.
const KINDS: [(&[u8], &str); 5] = [
    (b"OggS", "audio/ogg"),
    (b"RIFF", "audio/wav"),
    (b"fLaC", "audio/flac"),
    (b"ID3", "audio/mpeg"),
    (b"\xFF\xFB", "audio/mpeg"),
];
/// A sound of a kind not known: the player works it out.
const ANY_AUDIO: &str = "audio/*";

/// A sound's bytes (base64) and kind (`audio/ogg`).
#[derive(yog_data::Schema)]
pub(crate) struct SoundData {
    pub(crate) data: String,
    pub(crate) mime: String,
}

/// The sounds a montage can take: the mod's, and the game's.
#[derive(yog_data::Schema)]
pub(crate) struct SoundList {
    pub(crate) own: Vec<String>,
    pub(crate) game: Vec<String>,
}

/// The sounds of the mod at `file`, and the game's.
pub(crate) fn sound_list(file: &Path) -> Result<SoundList, ServiceError> {
    let root = mod_root(file).ok_or("not in a mod")?;
    Ok(SoundList {
        own: crate::media::assets::sounds(&root),
        game: crate::media::assets::game_sounds(),
    })
}

/// Sound `sound` as the mod at `file` plays it: `@res:Set.name` its own, else
/// a file of the game's sounds (`minecraft:dig/sand1`, no namespace: the game's).
pub(crate) fn sound_data(file: &Path, sound: &str) -> Result<SoundData, ServiceError> {
    let bytes = match sound.strip_prefix(yog_ids::reference::RES) {
        Some(res) => own(file, res)?,
        None => game(sound)?,
    };
    let mime = KINDS
        .iter()
        .find(|(magic, _)| bytes.starts_with(magic))
        .map_or(ANY_AUDIO, |(_, m)| m);
    Ok(SoundData {
        data: base64::engine::general_purpose::STANDARD.encode(&bytes),
        mime: mime.to_owned(),
    })
}

/// The mod's sound `Set.name`.
fn own(file: &Path, res: &str) -> Result<Vec<u8>, ServiceError> {
    let (set, name) = res
        .split_once(yog_ids::sep::DOT)
        .ok_or_else(|| ServiceError::from(format!("`{res}`: a sound is `@res:Set.name`")))?;
    let root = mod_root(file).ok_or("not in a mod")?;
    crate::media::assets::sound(&root, set, name)
        .ok_or_else(|| ServiceError::not_found(format!("no sound `{}{res}` in the mod's .yres", yog_ids::reference::RES)))
}

/// The game's sound file `id`, from its assets.
fn game(id: &str) -> Result<Vec<u8>, ServiceError> {
    crate::media::assets::game_sound(id).ok_or_else(|| {
        ServiceError::not_found(format!(
            "the game's sound {id}: no assets found on this machine"
        ))
    })
}

#[cfg(test)]
mod tests {
    use base64::Engine;

    #[test]
    fn the_mods_sound_is_its_yres_bytes() {
        let dir = std::env::temp_dir().join(format!("yog-sound-data-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("resources")).unwrap();
        std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"mymod\"\n").unwrap();
        let ogg = base64::engine::general_purpose::STANDARD.encode(b"OggS....");
        std::fs::write(
            dir.join("resources/Sounds.yres"),
            format!(r#"{{ "click": {{ "type": "sound", "value": "@bytes:{ogg}" }} }}"#),
        )
        .unwrap();
        let file = dir.join("resources/click.ysound");
        let s = super::sound_data(&file, "@res:Sounds.click").unwrap();
        assert_eq!(
            (s.mime.as_str(), s.data.as_str()),
            ("audio/ogg", ogg.as_str())
        );
        assert!(super::sound_data(&file, "@res:Sounds.nope").is_err());
        assert_eq!(
            super::sound_list(&file).unwrap().own,
            vec!["@res:Sounds.click".to_owned()]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

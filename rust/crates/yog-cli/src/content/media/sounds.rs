//! `.ysound` files: the mod's sound events as the game takes them — one
//! `assets/<mod id>/sounds.json` of them all, each naming the `.yres`
//! sounds it plays (bundled as Ogg Vorbis by the resources' build) and the
//! translation key of its subtitle.

use std::path::Path;

use serde_json::{Map, Value, json};
use yog_formats::sound::{SoundFile, Variant};
use yog_ids::Format;

use crate::content::Output;
use crate::content::pipeline::ctx::problem;
use crate::diagnostics::Diagnostic;
use crate::yres::{RESOURCES_DIR, Refs};

/// `sounds.json`'s fields.
mod field {
    pub const SUBTITLE: &str = "subtitle";
    pub const REPLACE: &str = "replace";
    pub const NAME: &str = "name";
    pub const VOLUME: &str = "volume";
    pub const PITCH: &str = "pitch";
    pub const WEIGHT: &str = "weight";
    pub const STREAM: &str = "stream";
    pub const PRELOAD: &str = "preload";
    pub const ATTENUATION_DISTANCE: &str = "attenuation_distance";
}

/// The mod's sound events: the `sounds.json` they make (none without any),
/// and the sounds their mixes render into.
#[derive(Default)]
pub(crate) struct Sounds {
    pub(crate) output: Option<Output>,
    pub(crate) rendered: Vec<Output>,
}

/// Where a mix renders: the mod's sounds' folder `mix/<event>/<n>`.
const MIX_DIR: &str = "mix";

/// What the events' mixes render, and where their sounds come from.
struct Mixes<'a> {
    root: &'a Path,
    mod_id: &'a str,
    rendered: Vec<Output>,
}

/// Every `.ysound` under `resources/sound/` of the mod at `root`, with the
/// events made elsewhere (`events`: videos'); problems go to `errors`.
pub(crate) fn compile(
    root: &Path,
    mod_id: &str,
    refs: &Refs,
    mut events: Map<String, Value>,
    errors: &mut Vec<Diagnostic>,
) -> Sounds {
    let resources = root.join(RESOURCES_DIR);
    let mut mixes = Mixes {
        root,
        mod_id,
        rendered: Vec::new(),
    };
    for file in crate::files::with_ext(&resources, Format::Sound.ext()) {
        let shown = crate::shown_path(root, &file);
        let rel = file
            .strip_prefix(&resources)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', yog_ids::sep::PATH);
        let Some(id) = yog_formats::sound::id_of(mod_id, &rel) else {
            let dir = yog_formats::sound::DIR;
            errors.push(
                problem(format!("a sound event lives under {RESOURCES_DIR}/{dir}/"))
                    .in_file(&shown),
            );
            continue;
        };
        let (_, name) = yog_ids::split(&id);
        match event(&file, refs, &mut mixes, name) {
            Ok(def) => {
                events.insert(name.to_owned(), def);
            }
            Err(e) => errors.push(problem(e).in_file(&shown)),
        }
    }
    if events.is_empty() {
        return Sounds::default();
    }
    let path = yog_ids::asset_file(mod_id, yog_ids::res::SOUNDS_JSON);
    let text = serde_json::to_string_pretty(&Value::Object(events)).unwrap_or_default();
    Sounds {
        output: Some((path, format!("{text}\n").into_bytes())),
        rendered: mixes.rendered,
    }
}

/// An event playing the mod's sound `name` streamed (read as it plays: a long one).
pub(crate) fn streamed(name: &str) -> Value {
    json!({ yog_ids::res::SOUNDS_WORD: [{ field::NAME: name, field::STREAM: true }] })
}

/// The `.ysound` at `file` (event `name`) as its `sounds.json` entry; its mixes rendered into `mixes`.
fn event(file: &Path, refs: &Refs, mixes: &mut Mixes, name: &str) -> Result<Value, String> {
    let text = crate::files::read_source(file).map_err(|e| e.to_string())?;
    let sound = yog_data::from_text::<SoundFile>(&text)
        .map_err(|e| e.to_string())?
        .value;
    if sound.sounds.is_empty() {
        return Err("a sound event plays at least one sound".into());
    }
    let sounds = sound
        .sounds
        .iter()
        .enumerate()
        .map(|(i, v)| variant(v, refs, mixes, &format!("{name}{}{i}", yog_ids::sep::PATH)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut def = Map::new();
    def.insert(yog_ids::res::SOUNDS_WORD.into(), Value::Array(sounds));
    if let Some(sub) = &sound.subtitle {
        def.insert(
            field::SUBTITLE.into(),
            Value::String(refs.text(sub)?.0.clone()),
        );
    }
    if sound.replace {
        def.insert(field::REPLACE.into(), Value::Bool(true));
    }
    Ok(Value::Object(def))
}

/// One of the sounds an event plays (`slot`: where its mix renders, `<event>/<n>`):
/// its file's name, and what's said of it.
fn variant(v: &Variant, refs: &Refs, mixes: &mut Mixes, slot: &str) -> Result<Value, String> {
    let name = match (&v.sound, &v.file, &v.mix) {
        (Some(own), None, None) => refs.sound(own)?.to_owned(),
        (None, Some(file), None) => yog_ids::qualify(file),
        (None, None, Some(mix)) => mixes.render(mix, slot)?,
        _ => {
            return Err("a sound is the mod's (`sound`), a file the game has (`file`), or a mix of them (`mix`)".into());
        }
    };
    let mut out = Map::new();
    out.insert(field::NAME.into(), json!(name));
    let mut put = |key: &str, value: Option<Value>| {
        if let Some(value) = value {
            out.insert(key.into(), value);
        }
    };
    put(field::VOLUME, v.volume.map(|x| json!(x)));
    put(field::PITCH, v.pitch.map(|x| json!(x)));
    put(field::WEIGHT, v.weight.map(|x| json!(x)));
    put(field::STREAM, v.stream.then_some(Value::Bool(true)));
    put(field::PRELOAD, v.preload.then_some(Value::Bool(true)));
    put(
        field::ATTENUATION_DISTANCE,
        v.attenuation_distance.map(|x| json!(x)),
    );
    Ok(Value::Object(out))
}

impl Mixes<'_> {
    /// `mix` rendered as the mod's sound `mix/<slot>`: its name.
    fn render(&mut self, mix: &yog_formats::montage::Mix, slot: &str) -> Result<String, String> {
        let sources = mix
            .clips
            .iter()
            .map(|c| crate::media::mix::clip_source(self.root, c))
            .collect::<Result<Vec<_>, _>>()?;
        let ogg = crate::media::mix::render(mix, &sources)?;
        let path = format!("{MIX_DIR}{}{slot}", yog_ids::sep::PATH);
        let file = format!("{}{path}{}", yog_ids::res::SOUNDS, yog_ids::res::OGG);
        self.rendered
            .push((yog_ids::asset_file(self.mod_id, &file), ogg));
        Ok(yog_ids::join(self.mod_id, &path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_play_the_mods_sounds_and_the_games_files() {
        let dir = std::env::temp_dir().join(format!("yog-ysound-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let sound = dir.join(RESOURCES_DIR).join(yog_formats::sound::DIR);
        std::fs::create_dir_all(sound.join("scroll")).unwrap();
        std::fs::write(
            sound.join("abacus.ysound"),
            r#"{ sounds: [{ sound: "@Sounds.abacus1", pitch: 0.8 }], subtitle: "@Lang.abacus" }"#,
        )
        .unwrap();
        std::fs::write(
            sound.join("scroll/dust.ysound"),
            r#"{ sounds: [{ file: "dig/sand1" }] }"#,
        )
        .unwrap();
        std::fs::write(sound.join("bad.ysound"), r#"{ sounds: [{}] }"#).unwrap();
        let mut refs = Refs::default();
        refs.sounds
            .insert("Sounds.abacus1".into(), "m:abacus1".into());
        refs.texts
            .insert("Lang.abacus".into(), ("m.abacus".into(), "Abacus".into()));
        let mut errors = Vec::new();
        let out = compile(&dir, "m", &refs, Map::new(), &mut errors)
            .output
            .unwrap();
        assert_eq!(
            errors.len(),
            1,
            "only the event playing nothing: {errors:?}"
        );
        let json: Value = serde_json::from_slice(&out.1).unwrap();
        assert_eq!(json["abacus"]["sounds"][0]["name"], "m:abacus1");
        assert_eq!(
            json["abacus"]["sounds"][0]["pitch"].as_f64(),
            Some(f64::from(0.8f32))
        );
        assert_eq!(json["abacus"]["subtitle"], "m.abacus");
        assert_eq!(
            json["scroll.dust"]["sounds"][0]["name"],
            "minecraft:dig/sand1"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

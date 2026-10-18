//! `.ytexture` files: every texture of the mod as the game takes it — its
//! picture (whatever it was made as, through `yog_media`) as a PNG, and its
//! animation as the game's `.mcmeta`.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};
use yog_formats::texture::{Animation, Frame, TextureFile};
use yog_ids::Format;

use super::Output;
use crate::yres::RESOURCES_DIR;

/// The mod's textures: their ids, and the files they become.
#[derive(Default)]
pub struct Textures {
    pub ids: BTreeSet<String>,
    pub outputs: Vec<Output>,
}

/// Every `.ytexture` under `resources/` of the mod at `root`; problems go to `errors`.
pub fn compile(root: &Path, mod_id: &str, errors: &mut Vec<String>) -> Textures {
    let resources = root.join(RESOURCES_DIR);
    let mut out = Textures::default();
    for file in crate::yui::find_files(&resources, Format::Texture.ext()) {
        let shown = super::display(root, &file);
        let rel = file.strip_prefix(&resources).unwrap_or(&file).to_string_lossy().replace('\\', yog_ids::sep::PATH);
        let Some(id) = yog_formats::texture::id_of(mod_id, &rel) else { continue };
        match compile_one(&file, &id) {
            Ok(files) => {
                out.ids.insert(id);
                out.outputs.extend(files);
            }
            Err(e) => errors.push(format!("{shown}: {e}")),
        }
    }
    out
}

/// One texture's PNG, and its `.mcmeta` when it animates.
fn compile_one(file: &Path, id: &str) -> Result<Vec<Output>, String> {
    let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    let texture = yog_data::from_text::<TextureFile>(&text).map_err(|e| e.to_string())?.value;
    let source = file.parent().unwrap_or(Path::new("")).join(&texture.source);
    let bytes = std::fs::read(&source).map_err(|e| format!("source {}: {e}", texture.source))?;
    let image = yog_media::image::decode(&bytes).map_err(|e| format!("source {}: {e}", texture.source))?;
    let png = yog_media::image::encode_png(&image)?;
    let (ns, path) = yog_ids::split(id);
    let target = format!("{}/{ns}/{}", yog_ids::pack::ASSETS, yog_ids::texture_file(path));
    let mut out = vec![(target.clone(), png)];
    if let Some(a) = &texture.animation {
        out.push((format!("{target}{}", yog_ids::res::MCMETA), super::pretty(&mcmeta(a))));
    }
    Ok(out)
}

/// A `.ytexture` of picture `source` with the animation a game `.mcmeta` describes.
pub fn from_mcmeta(source: &str, mcmeta: Option<&[u8]>) -> TextureFile {
    let anim = mcmeta.and_then(|m| serde_json::from_slice::<Value>(m).ok()).and_then(|v| v.get("animation").cloned());
    let u32_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok());
    let animation = anim.map(|a| Animation {
        frame_time: u32_of(&a, "frametime"),
        interpolate: a.get("interpolate").and_then(Value::as_bool).unwrap_or(false),
        frames: a
            .get("frames")
            .and_then(Value::as_array)
            .map(|fs| {
                fs.iter()
                    .filter_map(|f| match f.as_u64() {
                        Some(i) => Some(Frame { index: u32::try_from(i).ok()?, time: None }),
                        None => Some(Frame { index: u32_of(f, "index")?, time: u32_of(f, "time") }),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        width: u32_of(&a, "width"),
        height: u32_of(&a, "height"),
    });
    TextureFile { source: source.to_string(), animation }
}

/// The game's `.mcmeta` of an animation.
fn mcmeta(a: &Animation) -> Value {
    let mut anim = serde_json::Map::new();
    anim.insert("frametime".into(), json!(a.frame_time()));
    if a.interpolate {
        anim.insert("interpolate".into(), json!(true));
    }
    if !a.frames.is_empty() {
        let frames = a.frames.iter().map(|f| match f.time {
            Some(time) => json!({ "index": f.index, "time": time }),
            None => json!(f.index),
        });
        anim.insert("frames".into(), Value::Array(frames.collect()));
    }
    if let Some(w) = a.width {
        anim.insert("width".into(), json!(w));
    }
    if let Some(h) = a.height {
        anim.insert("height".into(), json!(h));
    }
    json!({ "animation": anim })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_texture_becomes_the_games_png_and_mcmeta() {
        let dir = std::env::temp_dir().join(format!("yog-ytexture-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("resources/block")).unwrap();
        let img = yog_media::image::Image { width: 1, height: 2, rgba: vec![255; 8] };
        std::fs::write(dir.join("resources/block/ruby.png"), yog_media::image::encode_png(&img).unwrap()).unwrap();
        std::fs::write(dir.join("resources/block/ruby.ytexture"), r#"{ source: "ruby.png", animation: { frame_time: 3 } }"#).unwrap();
        let mut errors = Vec::new();
        let t = compile(&dir, "mymod", &mut errors);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(t.ids.contains("mymod:block/ruby"));
        let names: Vec<&str> = t.outputs.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(names, ["assets/mymod/textures/block/ruby.png", "assets/mymod/textures/block/ruby.png.mcmeta"]);
        let meta: Value = serde_json::from_slice(&t.outputs[1].1).unwrap();
        assert_eq!(meta["animation"]["frametime"], 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mcmeta_round_trips() {
        let meta = br#"{"animation":{"frametime":2,"interpolate":true,"frames":[1,{"index":0,"time":5}]}}"#;
        let t = from_mcmeta("x.png", Some(meta));
        let back = mcmeta(t.animation.as_ref().unwrap());
        assert_eq!(back, serde_json::from_slice::<Value>(meta).unwrap());
        assert_eq!(from_mcmeta("x.png", None).animation, None);
    }
}

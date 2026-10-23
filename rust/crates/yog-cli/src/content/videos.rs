//! `.yvideo` files: the mod's videos — each rendered (`crate::video`) into
//! a definition the code registers (its AV1 packets and how they play) and,
//! when it keeps its sound, a streamed sound event `video.<name>` with its
//! Ogg file.

use std::path::Path;

use serde_json::{Map, Value};
use yog_formats::video::Video;
use yog_ids::Format;

use super::Output;
use crate::content::ctx::problem;
use crate::diagnostics::Diagnostic;
use crate::yres::RESOURCES_DIR;

/// The mod's videos: their sounds' files and events, and their definitions.
#[derive(Default)]
pub(crate) struct Videos {
    pub(crate) outputs: Vec<Output>,
    /// `sounds.json` entries, by event name.
    pub(crate) events: Map<String, Value>,
    pub(crate) definitions: Vec<(&'static str, String, Vec<u8>)>,
}

impl Videos {
    pub(crate) fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

/// Every `.yvideo` under `resources/video/` of the mod at `root` (made of
/// its `.yres` videos, pictures and sounds); problems go to `errors`.
pub(crate) fn compile(root: &Path, mod_id: &str, errors: &mut Vec<Diagnostic>) -> Videos {
    let resources = root.join(RESOURCES_DIR);
    let mut out = Videos::default();
    for file in crate::files::with_ext(&resources, Format::Video.ext()) {
        let shown = crate::shown_path(root, &file);
        let rel = file
            .strip_prefix(&resources)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', yog_ids::sep::PATH);
        let Some(id) = yog_formats::video::id_of(mod_id, &rel) else {
            let dir = yog_formats::video::DIR;
            errors.push(
                problem(format!("a video lives under {RESOURCES_DIR}/{dir}/")).in_file(&shown),
            );
            continue;
        };
        if let Err(e) = out.add(&file, root, id) {
            errors.push(problem(e).in_file(&shown));
        }
    }
    out
}

impl Videos {
    /// The `.yvideo` at `file` (video `id`), rendered.
    fn add(&mut self, file: &Path, root: &Path, id: String) -> Result<(), String> {
        let text = crate::files::read_source(file).map_err(|e| e.to_string())?;
        let video = yog_data::from_text::<Video>(&text)
            .map_err(|e| e.to_string())?
            .value;
        let mut r = crate::video::render(&video, root)?;
        let (ns, name) = yog_ids::split(&id);
        let sound = r.sound.take().map(|ogg| {
            let event = yog_formats::video::sound_event(name);
            let path = event.replace(yog_ids::sep::DOT, yog_ids::sep::PATH);
            let file = format!("{}{path}{}", yog_ids::res::SOUNDS, yog_ids::res::OGG);
            self.outputs.push((yog_ids::asset_file(ns, &file), ogg));
            let streamed = super::sounds::streamed(&yog_ids::join(ns, &path));
            self.events.insert(event.clone(), streamed);
            yog_ids::join(ns, &event)
        });
        let def = r.def(id.clone(), &video, sound);
        self.definitions.push((
            yog_ids::kind::VIDEO,
            id,
            yog_data::binary::encode(&yog_data::Schema::write(&def)),
        ));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yog_registry::VideoDef;

    #[test]
    fn a_video_needs_its_source_and_its_folder() {
        let dir = std::env::temp_dir().join(format!("yog-yvideo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let videos = dir.join(RESOURCES_DIR).join(yog_formats::video::DIR);
        std::fs::create_dir_all(&videos).unwrap();
        std::fs::write(videos.join("intro.yvideo"), r#"{ source: "@Videos.nope" }"#).unwrap();
        std::fs::write(
            dir.join(RESOURCES_DIR).join("stray.yvideo"),
            r#"{ source: "@V.a" }"#,
        )
        .unwrap();
        let mut errors = Vec::new();
        let v = compile(&dir, "m", &mut errors);
        assert!(v.is_empty());
        assert_eq!(errors.len(), 2, "{errors:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_video_is_a_definition_and_a_streamed_sound() {
        let dir = std::env::temp_dir().join(format!("yog-yvideo-ok-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let videos = dir.join(RESOURCES_DIR).join(yog_formats::video::DIR);
        std::fs::create_dir_all(videos.join("cut")).unwrap();
        std::fs::write(
            videos.join("cut/intro.yvideo"),
            r#"{ source: "@Media.intro", width: 64, looping: true }"#,
        )
        .unwrap();
        crate::video::tests::put_source(&dir, "intro");
        let mut errors = Vec::new();
        let v = compile(&dir, "m", &mut errors);
        assert!(errors.is_empty(), "{errors:?}");
        let (kind, id, bytes) = &v.definitions[0];
        assert_eq!((*kind, id.as_str()), (yog_ids::kind::VIDEO, "m:cut.intro"));
        let def: VideoDef = yog_data::read_bytes(bytes).unwrap();
        assert_eq!((def.width, def.looping), (64, true));
        assert_eq!(def.sound.as_deref(), Some("m:video.cut.intro"));
        assert_eq!(v.outputs[0].0, "assets/m/sounds/video/cut/intro.ogg");
        assert_eq!(
            v.events["video.cut.intro"]["sounds"][0]["name"],
            "m:video/cut/intro"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

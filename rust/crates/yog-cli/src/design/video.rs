//! A video for the designer (Yog-IDLE's `.yvideo` editor): rendered as the
//! build renders it, its frames decoded as the game decodes them — the
//! preview is what the game will show. The last renders are kept, by key.

use std::path::Path;
use yog_sync::{Arc, Mutex};

use base64::Engine as _;
use yog_formats::video::Video;
use yog_media::video::Decoder;
use yog_registry::VideoDef;

use super::project::mod_root;
use crate::serve::ServiceError;

/// Renders kept, newest last.
const KEPT: usize = 4;

/// The videos a `.yvideo` can show: the mod's.
#[derive(yog_data::Schema)]
pub(crate) struct VideoList {
    pub(crate) own: Vec<String>,
}

/// A render: its key (for its frames), its picture's size and frame rate,
/// how many frames, and its sound (Ogg, base64; none: silent).
#[derive(yog_data::Schema)]
pub(crate) struct Preview {
    pub(crate) key: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) fps_num: u32,
    pub(crate) fps_den: u32,
    pub(crate) frames: u32,
    pub(crate) sound: Option<String>,
}

/// One frame, as PNG (base64).
#[derive(yog_data::Schema)]
pub(crate) struct Frame {
    pub(crate) png: String,
}

/// The renders kept, by key.
static RENDERS: Mutex<Vec<(String, Arc<VideoDef>)>> = Mutex::new(Vec::new());

/// The decoder, the render it decodes, the frame it's on and its pixels.
struct Decoding {
    key: String,
    decoder: Decoder,
    shown: Option<u32>,
    rgba: Vec<u8>,
}

static DECODING: Mutex<Option<Decoding>> = Mutex::new(None);

/// The videos of the mod at `file`.
pub(crate) fn video_list(file: &Path) -> Result<VideoList, ServiceError> {
    let root = mod_root(file).ok_or("not in a mod")?;
    Ok(VideoList {
        own: crate::media::videos(&root),
    })
}

/// Video `video` (as edited) of the mod at `file`, rendered.
pub(crate) fn preview(file: &Path, video: &Video) -> Result<Preview, ServiceError> {
    let root = mod_root(file).ok_or("not in a mod")?;
    let mut rendered = crate::video::render(video, &root)?;
    // Each render its own key: the decoder never mistakes one for another.
    static MADE: yog_sync::atomic::AtomicU64 = yog_sync::atomic::AtomicU64::new(0);
    let key = MADE
        .fetch_add(1, yog_sync::atomic::Ordering::Relaxed)
        .to_string();
    let sound = rendered
        .sound
        .take()
        .map(|ogg| base64::engine::general_purpose::STANDARD.encode(ogg));
    let def = rendered.def(key.clone(), video, None);
    let preview = Preview {
        key: key.clone(),
        width: def.width,
        height: def.height,
        fps_num: def.fps_num,
        fps_den: def.fps_den,
        frames: def.frames(),
        sound,
    };
    let mut renders = RENDERS.lock();
    renders.retain(|(k, _)| *k != key);
    renders.push((key, Arc::new(def)));
    let over = renders.len().saturating_sub(KEPT);
    renders.drain(..over);
    Ok(preview)
}

/// Frame `frame` of render `key`.
pub(crate) fn frame(key: &str, frame: u32) -> Result<Frame, ServiceError> {
    let def = RENDERS
        .lock()
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, d)| d.clone())
        .ok_or_else(|| ServiceError::not_found(format!("no render `{key}` (render it again)")))?;
    let mut decoding = DECODING.lock();
    if decoding.as_ref().is_none_or(|d| d.key != key) {
        *decoding = Some(Decoding {
            key: key.to_owned(),
            decoder: Decoder::new().map_err(|e| e.to_string())?,
            shown: None,
            rgba: Vec::new(),
        });
    }
    let d = decoding.as_mut().ok_or("no decoder")?;
    decode(d, &def, frame)?;
    let image = yog_media::image::Image {
        width: def.width,
        height: def.height,
        rgba: d.rgba.clone(),
    };
    let png = yog_media::image::encode_png(&image).map_err(|e| e.to_string())?;
    Ok(Frame {
        png: base64::engine::general_purpose::STANDARD.encode(png),
    })
}

/// `d` on to frame `target` of `def` (from the keyframe before it when need be).
fn decode(d: &mut Decoding, def: &VideoDef, target: u32) -> Result<(), ServiceError> {
    let plan = def.plan(d.shown, target);
    if plan.reset {
        d.decoder.reset();
    }
    let size = (def.width as usize, def.height as usize);
    let packets = plan.frames.clone().filter_map(|f| def.packet(f));
    if let Err(e) = d.decoder.decode_last(packets, size, &mut d.rgba) {
        d.shown = None;
        return Err(e.to_string().into());
    }
    d.shown = plan.frames.clone().next_back().or(d.shown);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use yog_formats::yres::{Entry, Kind, Value};

    #[test]
    fn a_preview_renders_and_its_frames_decode() {
        let root = std::env::temp_dir().join(format!("yog-video-design-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let res = root.join(yog_ids::project::RESOURCES);
        std::fs::create_dir_all(res.join(yog_formats::video::DIR)).unwrap();
        std::fs::write(
            root.join(yog_ids::file::MANIFEST),
            "[mod]\nid = \"m\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        let entry = Entry {
            name: "intro".into(),
            kind: Kind::Video,
            value: Value::Bytes(crate::video::tests::source()),
            comment: None,
            key: None,
            line: 0,
        };
        std::fs::write(res.join("Videos.yres"), yog_formats::yres::write(&[entry])).unwrap();
        let file = res.join(yog_formats::video::DIR).join("intro.yvideo");
        assert_eq!(
            video_list(&file).unwrap().own,
            vec!["@Videos.intro".to_owned()]
        );
        let video =
            yog_data::from_text::<Video>(r#"{ source: "@Videos.intro", width: 64, fps: 10 }"#)
                .unwrap()
                .value;
        let p = preview(&file, &video).unwrap();
        assert_eq!((p.width, p.height, p.frames), (64, 36, 20));
        assert!(p.sound.is_some());
        for f in [0, 5, 19, 3] {
            let png = base64::engine::general_purpose::STANDARD
                .decode(frame(&p.key, f).unwrap().png)
                .unwrap();
            let img = yog_media::image::decode(&png).unwrap();
            assert_eq!((img.width, img.height), (64, 36));
        }
        assert!(frame("nope", 0).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}

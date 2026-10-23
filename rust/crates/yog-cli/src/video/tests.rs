use std::path::{Path, PathBuf};

use yog_formats::yres::{Entry, Kind, Value};

use super::*;
use crate::ffmpeg::ffmpeg;

fn video(text: &str) -> Video {
    yog_data::from_text::<Video>(text).unwrap().value
}

/// ffmpeg's lavfi `inputs` (each a source graph) written as WebM.
fn made_by(inputs: &[&str], codecs: &[&str]) -> Vec<u8> {
    let mut cmd = ffmpeg();
    for i in inputs {
        cmd.args(["-f", "lavfi", "-i", i]);
    }
    cmd.args(codecs).args(["-f", "webm", STDOUT]);
    run(&mut cmd, "make a test video").unwrap()
}

/// A 2-second 24 fps test picture with a tone, as WebM.
pub(crate) fn source() -> Vec<u8> {
    made_by(
        &["testsrc=size=161x91:rate=24:duration=2", "sine=duration=2"],
        &["-c:v", "libvpx-vp9", "-c:a", "libvorbis"],
    )
}

/// The `.yres` entry `name` of `kind` holding `bytes`.
fn entry(name: &str, kind: Kind, bytes: Vec<u8>) -> Entry {
    Entry {
        name: name.into(),
        kind,
        value: Value::Bytes(bytes),
        comment: None,
        key: None,
        line: 0,
    }
}

/// `entries` as the mod at `root`'s `resources/Media.yres`.
fn put(root: &Path, entries: &[Entry]) {
    let res = root.join(yog_ids::project::RESOURCES);
    std::fs::create_dir_all(res.join(yog_formats::video::DIR)).unwrap();
    std::fs::write(res.join("Media.yres"), yog_formats::yres::write(entries)).unwrap();
}

/// A mod in a folder of its own, `entries` its media.
fn a_mod(tag: &str, entries: &[Entry]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("yog-video-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    put(&root, entries);
    root
}

/// [`source`] as video `@Media.<name>` of the mod at `root`.
pub(crate) fn put_source(root: &Path, name: &str) {
    put(root, &[entry(name, Kind::Video, source())]);
}

#[test]
fn a_bad_quality_or_no_source_is_refused_before_ffmpeg_runs() {
    let root = Path::new("/nowhere");
    let err = render(&video(r#"{ source: "@V.a", quality: 64 }"#), root).unwrap_err();
    assert!(err.contains("quality 64"), "{err}");
    let err = render(&video("{}"), root).unwrap_err();
    assert!(err.contains("montage"), "{err}");
}

#[test]
fn a_video_renders_into_av1_packets_and_a_sound() {
    let root = a_mod("render", &[entry("a", Kind::Video, source())]);
    let r = render(&video(r#"{ source: "@Media.a", fps: 10 }"#), &root).unwrap();
    assert_eq!((r.width, r.height), (160, 90), "made even");
    assert_eq!((r.fps_num, r.fps_den), (10, 1));
    assert_eq!(r.sizes.len(), 20);
    assert_eq!(r.sizes.iter().sum::<u32>() as usize, r.packets.len());
    assert_eq!(r.keyframes[0], 0);
    let ogg = |s: &Option<Vec<u8>>| {
        s.as_ref()
            .is_some_and(|s| s.starts_with(yog_formats::yres::OGG_MAGIC))
    };
    assert!(ogg(&r.sound));
    let silent = render(&video(r#"{ source: "@Media.a", sound: false }"#), &root).unwrap();
    assert!(silent.sound.is_none());
    let _ = std::fs::remove_dir_all(&root);
}

/// `r`'s frames `frames` decoded (after a reset when `reset`): each one's
/// top-left pixel.
fn decode(
    d: &mut yog_media::video::Decoder,
    r: &Rendered,
    plan: &yog_registry::Plan,
) -> Vec<[u8; 4]> {
    if plan.reset {
        d.reset();
    }
    let mut at = 0usize;
    let mut out = Vec::new();
    for (i, &n) in r.sizes.iter().enumerate() {
        let packet = &r.packets[at..at + n as usize];
        at += n as usize;
        if !plan.frames.contains(&(i as u32)) {
            continue;
        }
        let mut rgba = Vec::new();
        let size = (r.width as usize, r.height as usize);
        d.decode(packet, |planes, _| {
            yog_media::video::yuv::to_rgba(planes, size, &mut rgba)
        })
        .unwrap()
        .unwrap();
        out.push([rgba[0], rgba[1], rgba[2], rgba[3]]);
    }
    out
}

fn red(p: &[u8; 4]) -> bool {
    p[0] > 240 && p[1] < 16 && p[2] < 16 && p[3] == 255
}

fn all(frames: u32) -> yog_registry::Plan {
    yog_registry::Plan {
        reset: true,
        frames: 0..frames,
    }
}

#[test]
fn rendered_packets_decode_from_the_start_and_from_a_keyframe() {
    let src = made_by(
        &["color=c=red:size=96x54:rate=10:duration=3"],
        &["-c:v", "libvpx-vp9"],
    );
    let root = a_mod("decode", &[entry("a", Kind::Video, src)]);
    let r = render(&video(r#"{ source: "@Media.a" }"#), &root).unwrap();
    assert_eq!(
        r.keyframes.len(),
        2,
        "about every two seconds: {:?}",
        r.keyframes
    );
    let key = r.keyframes[1];
    let mut d = yog_media::video::Decoder::new().unwrap();
    let px = decode(&mut d, &r, &all(30));
    assert_eq!(px.len(), 30);
    assert!(px.iter().all(red), "{:?}", px[0]);
    let seek = yog_registry::Plan {
        reset: true,
        frames: key..key + 5,
    };
    assert!(decode(&mut d, &r, &seek).iter().all(red));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_montage_lays_clips_over_each_other_with_their_sound() {
    let red_clip = made_by(
        &[
            "color=c=red:size=64x64:rate=10:duration=2",
            "sine=duration=2",
        ],
        &["-c:v", "libvpx-vp9", "-c:a", "libvorbis"],
    );
    let blue = yog_media::image::Image {
        width: 4,
        height: 4,
        rgba: [0, 0, 255, 255].repeat(16),
    };
    let blue_png = yog_media::image::encode_png(&blue).unwrap();
    let root = a_mod(
        "montage",
        &[
            entry("red", Kind::Video, red_clip),
            entry("blue", Kind::Image, blue_png),
        ],
    );
    let v = video(
        r#"{ width: 64, height: 64, fps: 10, montage: { clips: [
            { video: "@Media.red" },
            { image: "@Media.blue", at: 1, to: 0.5, track: 1 } ] } }"#,
    );
    let r = render(&v, &root).unwrap();
    assert_eq!((r.width, r.height), (64, 64));
    assert!(
        (20..=21).contains(&r.sizes.len()),
        "2 s at 10 fps: {}",
        r.sizes.len()
    );
    assert!(r.sound.is_some(), "the red clip's own sound");
    let mut d = yog_media::video::Decoder::new().unwrap();
    let px = decode(&mut d, &r, &all(20));
    assert!(red(&px[5]), "the red video alone: {:?}", px[5]);
    assert!(
        px[12][2] > 200 && px[12][0] < 40,
        "the picture over it: {:?}",
        px[12]
    );
    assert!(red(&px[17]), "the picture gone: {:?}", px[17]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_poster_is_the_first_frame_at_the_size_shown() {
    let root = a_mod("poster", &[entry("intro", Kind::Video, source())]);
    let dir = root
        .join(yog_ids::project::RESOURCES)
        .join(yog_formats::video::DIR);
    std::fs::create_dir_all(dir.join("cut")).unwrap();
    std::fs::write(
        dir.join("cut/intro.yvideo"),
        r#"{ source: "@Media.intro", width: 64 }"#,
    )
    .unwrap();
    let img = poster(&root, "m:cut.intro").unwrap();
    assert_eq!((img.width, img.height), (64, 36));
    assert!(poster(&root, "m:nope").is_none());
    let _ = std::fs::remove_dir_all(&root);
}

//! A video's first frame at the size it's shown — what the screen designer
//! draws in its place.

use std::path::Path;

use yog_formats::video::Video;
use yog_ids::Format;

use super::{probe, size};
use crate::ffmpeg::{INPUT, STDOUT, Scratch, ffmpeg, run};

/// One frame out, as PNG.
const FIRST_FRAME_PNG: &[&str] = &["-frames:v", "1", "-f", "image2pipe", "-c:v", "png"];

/// The first frame of video `id` of the mod at `root` (none when it can't be made).
pub(crate) fn poster(root: &Path, id: &str) -> Option<yog_media::image::Image> {
    let (_, name) = yog_ids::split(id);
    let file = root
        .join(yog_ids::project::RESOURCES)
        .join(yog_formats::video::DIR)
        .join(Format::Video.file(&name.replace(yog_ids::sep::DOT, yog_ids::sep::PATH)));
    let text = crate::files::read_source(&file).ok()?;
    let video = yog_data::from_text::<Video>(&text).ok()?.value;
    let source = crate::media::video(root, &video.source.set, &video.source.key)?;
    let dir = Scratch::new().ok()?;
    let input = dir.input(INPUT, &source).ok()?;
    let (w, h) = size::shown(&video, probe::probe(&input).ok()?.size);
    let mut cmd = ffmpeg();
    cmd.arg("-i")
        .arg(&input)
        .args(["-vf", &format!("scale={w}:{h}")])
        .args(FIRST_FRAME_PNG)
        .arg(STDOUT);
    let png = run(&mut cmd, "take the video's first frame").ok()?;
    yog_media::image::decode(&png).ok()
}

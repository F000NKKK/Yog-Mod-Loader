//! A video's first frame at the size it's shown — what the screen designer
//! draws in its place.

use std::path::Path;

use yog_formats::video::Video;
use yog_ids::Format;

use yog_media::video::yuv::RGBA;

use crate::ffmpeg::{STDOUT, Scratch, run};

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
    let dir = Scratch::new().ok()?;
    let made = super::made(&video, root, &dir).ok()?;
    let (mut cmd, shown) = super::picture(&video, &made).ok()?;
    cmd.args(FIRST_FRAME_PNG).arg(STDOUT);
    let png = run(&mut cmd, "take the video's first frame").ok()?;
    yog_media::image::decode(&png)
        .ok()
        .map(|img| top_left(&img, shown))
}

/// The top-left `w`×`h` of `img` (a frame padded up to the encoder's least).
fn top_left(img: &yog_media::image::Image, (w, h): (u32, u32)) -> yog_media::image::Image {
    let (w, h) = (w.min(img.width), h.min(img.height));
    let (row, stride) = (w as usize * RGBA, img.width as usize * RGBA);
    let rgba = (0..h as usize)
        .flat_map(|y| &img.rgba[y * stride..y * stride + row])
        .copied()
        .collect();
    yog_media::image::Image {
        width: w,
        height: h,
        rgba,
    }
}

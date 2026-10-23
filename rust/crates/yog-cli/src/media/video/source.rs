//! A video showing one `.yres` video (`source`): its picture resized and
//! its frame rate changed as the `.yvideo` says, its sound kept as it is.

use std::path::Path;
use std::process::Command;

use yog_formats::video::Video;

use super::{probe, size};
use crate::media::ffmpeg::{OGG_OUT, ffmpeg, run};

/// ffmpeg reading the source at `input`, filtered to the size `v` is shown
/// at (the output left to add); that size.
pub(super) fn picture(v: &Video, input: &Path) -> Result<(Command, (u32, u32)), String> {
    let src = probe::probe(input)?;
    let shown = size::shown(v, src.size);
    let mut cmd = ffmpeg();
    cmd.arg("-i")
        .arg(input)
        .args(["-an", "-vf", &size::filters(v, shown)]);
    Ok((cmd, shown))
}

/// The source's sound as Ogg Vorbis, when `v` keeps it and it has one.
pub(super) fn sound(v: &Video, input: &Path) -> Result<Option<Vec<u8>>, String> {
    if !v.sound || !probe::probe(input)?.sound {
        return Ok(None);
    }
    let mut cmd = ffmpeg();
    cmd.arg("-i").arg(input).arg("-vn").args(OGG_OUT);
    run(&mut cmd, "convert the video's sound to Ogg Vorbis").map(Some)
}

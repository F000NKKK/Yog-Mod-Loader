//! What a video's source holds, as ffprobe tells: its picture's size,
//! whether it has a sound.

use std::path::Path;

use crate::ffmpeg::{FFPROBE, ffprobe};

/// ffprobe's answer: each stream a line `<type>,<width>,<height>`.
const ENTRIES: &str = "stream=codec_type,width,height";
const CSV: &str = "csv=p=0";
const FIELD_SEP: char = ',';
const VIDEO: &str = "video";
const AUDIO: &str = "audio";

/// A source's streams.
#[derive(Debug, PartialEq)]
pub(super) struct Source {
    /// Its picture's size, pixels.
    pub(super) size: (u32, u32),
    pub(super) sound: bool,
}

/// The source at `path`.
pub(super) fn probe(path: &Path) -> Result<Source, String> {
    let mut cmd = ffprobe();
    cmd.args(["-show_entries", ENTRIES, "-of", CSV]).arg(path);
    let out = cmd
        .output()
        .map_err(|e| format!("a video needs {FFPROBE}, which can't be run: {e}"))?;
    parse(&String::from_utf8_lossy(&out.stdout)).ok_or_else(|| {
        let err = String::from_utf8_lossy(&out.stderr);
        format!("not a video ffmpeg reads: {}", err.trim())
    })
}

/// ffprobe's lines read: none without a picture.
fn parse(text: &str) -> Option<Source> {
    let mut size = None;
    let mut sound = false;
    for line in text.lines() {
        let mut f = line.trim().split(FIELD_SEP);
        match f.next() {
            Some(VIDEO) if size.is_none() => {
                let w = f.next()?.parse().ok()?;
                let h = f.next()?.parse().ok()?;
                size = Some((w, h));
            }
            Some(AUDIO) => sound = true,
            _ => {}
        }
    }
    size.map(|size| Source { size, sound })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_picture_and_any_sound_are_found() {
        let s = parse("video,161,91\naudio,,\nvideo,8,8\n").unwrap();
        assert_eq!(
            s,
            Source {
                size: (161, 91),
                sound: true
            }
        );
        assert_eq!(parse("audio,,\n"), None);
    }
}

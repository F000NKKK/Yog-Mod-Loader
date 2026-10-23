//! What a video's source holds, as ffprobe tells: its picture's size,
//! whether it has a sound, how long it lasts.

use std::path::Path;

use crate::ffmpeg::{FFPROBE, ffprobe};

/// ffprobe's answer: each stream a line `<type>,<width>,<height>`, then
/// the file's length in seconds (`N/A` for a picture).
const ENTRIES: &str = "stream=codec_type,width,height:format=duration";
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
    /// Seconds (none: a picture).
    pub(super) seconds: Option<f32>,
}

/// Its picture's packets: when each starts and how long it shows.
const PACKETS: &str = "packet=pts_time,duration_time";
const FIRST_PICTURE: &str = "v:0";

/// How long the video at `path` lasts by its packets (a file whose header
/// doesn't say — written as a stream).
fn length_by_packets(path: &Path) -> Option<f32> {
    let mut cmd = ffprobe();
    cmd.args([
        "-select_streams",
        FIRST_PICTURE,
        "-show_entries",
        PACKETS,
        "-of",
        CSV,
    ])
    .arg(path);
    let out = cmd.output().ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut f = l.trim().split(FIELD_SEP);
            let at: f32 = f.next()?.parse().ok()?;
            Some(at + f.next().and_then(|d| d.parse().ok()).unwrap_or(0.0))
        })
        .reduce(f32::max)
}

/// The source at `path`.
pub(super) fn probe(path: &Path) -> Result<Source, String> {
    let mut cmd = ffprobe();
    cmd.args(["-show_entries", ENTRIES, "-of", CSV]).arg(path);
    let out = cmd
        .output()
        .map_err(|e| format!("a video needs {FFPROBE}, which can't be run: {e}"))?;
    let mut source = parse(&String::from_utf8_lossy(&out.stdout)).ok_or_else(|| {
        let err = String::from_utf8_lossy(&out.stderr);
        format!("not a video ffmpeg reads: {}", err.trim())
    })?;
    if source.seconds.is_none() {
        source.seconds = length_by_packets(path);
    }
    Ok(source)
}

/// ffprobe's lines read: none without a picture.
fn parse(text: &str) -> Option<Source> {
    let mut size = None;
    let mut sound = false;
    let mut seconds = None;
    for line in text.lines() {
        if let Ok(s) = line.trim().parse::<f32>() {
            seconds = Some(s);
            continue;
        }
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
    size.map(|size| Source {
        size,
        sound,
        seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_picture_and_any_sound_are_found() {
        let s = parse("video,161,91\naudio,,\nvideo,8,8\n6.000000\n").unwrap();
        assert_eq!(
            s,
            Source {
                size: (161, 91),
                sound: true,
                seconds: Some(6.0)
            }
        );
        assert_eq!(parse("audio,,\n"), None);
    }
}

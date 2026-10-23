//! A video made of clips (`.yvideo`'s `montage`), rendered by one ffmpeg
//! graph: a black frame the montage's size and length, each shown clip
//! fitted into it (proportions kept), faded, made see-through and placed
//! at its time, a higher track over a lower; its sound the montage's mix
//! with the videos' own sound laid in.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use yog_formats::montage::SoundClip;
use yog_formats::video::{MONTAGE_FPS, Montage, PictureClip, Video};

use super::{probe, size};
use crate::ffmpeg::{Scratch, ffmpeg};

/// A clip as the graph takes it: where its file is, how long its video
/// lasts (none: a picture), whether that has sound.
pub(super) struct Placed<'a> {
    clip: &'a PictureClip,
    file: PathBuf,
    length: Option<f32>,
    sound: bool,
}

impl Placed<'_> {
    fn seconds(&self) -> f32 {
        self.clip.duration(self.length)
    }

    fn end(&self) -> f32 {
        self.clip.at + self.seconds()
    }
}

/// The shown clips of `m` (the mod at `root`'s videos and pictures), their
/// files written in `dir`, lowest track first.
pub(super) fn lay<'a>(
    m: &'a Montage,
    root: &Path,
    dir: &Scratch,
) -> Result<Vec<Placed<'a>>, String> {
    let mut placed = Vec::new();
    for (i, c) in m.clips.iter().enumerate().filter(|(_, c)| m.shown(c.track)) {
        let bytes = match (&c.video, &c.image) {
            (Some(r), None) => crate::media::video(root, &r.set, &r.key)
                .ok_or_else(|| format!("no video `{r}` in the mod's .yres"))?,
            (None, Some(r)) => crate::media::picture(root, &r.set, &r.key)
                .map(|p| p.bytes.to_vec())
                .ok_or_else(|| format!("no image `{r}` in the mod's .yres"))?,
            _ => return Err("a clip shows a video (`video`) or a picture (`image`)".into()),
        };
        let file = dir.input(&format!("clip{i}"), &bytes)?;
        let (length, sound) = match c.video {
            Some(_) => {
                let p = probe::probe(&file)?;
                (
                    Some(p.seconds.ok_or("a video clip's length can't be read")?),
                    p.sound,
                )
            }
            None => (None, false),
        };
        placed.push(Placed {
            clip: c,
            file,
            length,
            sound,
        });
    }
    placed.sort_by_key(|p| p.clip.track);
    if placed.is_empty() {
        return Err("a montage shows at least one clip".into());
    }
    Ok(placed)
}

/// How long the video `r` of the mod at `root` lasts, seconds (none when it can't be read).
pub(crate) fn video_seconds(root: &Path, r: &yog_data::Res) -> Option<f32> {
    let bytes = crate::media::video(root, &r.set, &r.key)?;
    let dir = Scratch::new().ok()?;
    probe::probe(&dir.input(crate::ffmpeg::INPUT, &bytes).ok()?)
        .ok()?
        .seconds
}

/// How long each clip of `m` shows, seconds (0 for one whose video can't be read).
pub(crate) fn durations(m: &Montage, root: &Path) -> Vec<f32> {
    m.clips
        .iter()
        .map(|c| match &c.video {
            Some(r) => video_seconds(root, r).map_or(0.0, |l| c.duration(Some(l))),
            None => c.duration(None),
        })
        .collect()
}

/// Clip `k`'s chain: cut, timed, fitted into `w`×`h`, faded, placed at its time.
fn chain(k: usize, p: &Placed, (w, h): (u32, u32), fps: f32) -> String {
    let c = p.clip;
    let d = p.seconds();
    let mut f = match p.length {
        Some(_) => format!(
            "[{k}:v]trim=start={}:end={},setpts=(PTS-STARTPTS)/{}",
            c.from,
            c.from + d * c.speed,
            c.speed
        ),
        None => format!("[{k}:v]setpts=PTS-STARTPTS"),
    };
    let _ = write!(
        f,
        ",fps={fps},scale={w}:{h}:force_original_aspect_ratio=decrease,\
         pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=black@0,format=yuva420p"
    );
    if c.fade_in > 0.0 {
        let _ = write!(f, ",fade=t=in:st=0:d={}:alpha=1", c.fade_in);
    }
    if c.fade_out > 0.0 {
        let _ = write!(
            f,
            ",fade=t=out:st={}:d={}:alpha=1",
            (d - c.fade_out).max(0.0),
            c.fade_out
        );
    }
    if c.opacity < 1.0 {
        let _ = write!(f, ",colorchannelmixer=aa={}", c.opacity.max(0.0));
    }
    let _ = write!(f, ",setpts=PTS+{}/TB[c{k}]", c.at);
    f
}

/// The graph of `placed` in a `w`×`h` frame at `fps`, ending in `tail` (the size's filters) as `[out]`.
pub(super) fn graph(placed: &[Placed], (w, h): (u32, u32), fps: f32, tail: &str) -> String {
    let total = placed.iter().map(Placed::end).fold(0.0, f32::max);
    let mut g = vec![format!("color=c=black:s={w}x{h}:r={fps}:d={total}[b0]")];
    for (k, p) in placed.iter().enumerate() {
        g.push(chain(k, p, (w, h), fps));
        g.push(format!(
            "[b{k}][c{k}]overlay=eof_action=pass:enable='between(t,{},{})'[b{}]",
            p.clip.at,
            p.end(),
            k + 1
        ));
    }
    g.push(format!("[b{}]{tail}[out]", placed.len()));
    g.join(";")
}

/// ffmpeg reading `placed` into `v`'s picture (the output left to add); its size.
pub(super) fn picture(v: &Video, placed: &[Placed]) -> Result<(Command, (u32, u32)), String> {
    let shown = size::exact(v).ok_or("a montage needs its `width` and `height`")?;
    let fps = v.fps.unwrap_or(MONTAGE_FPS);
    let mut cmd = ffmpeg();
    for p in placed {
        if p.length.is_none() {
            let (rate, secs) = (fps.to_string(), p.seconds().to_string());
            cmd.args(["-loop", "1", "-framerate", &rate, "-t", &secs]);
        }
        cmd.arg("-i").arg(&p.file);
    }
    let tail = size::filters(v, shown);
    cmd.args(["-filter_complex", &graph(placed, shown, fps, &tail)])
        .args(["-map", "[out]", "-an"]);
    Ok((cmd, shown))
}

/// The montage's sound: its mix, the videos' own sound laid in on a track
/// of their own; none when `v` keeps no sound or there's none.
pub(super) fn sound(
    v: &Video,
    m: &Montage,
    placed: &[Placed],
    root: &Path,
) -> Result<Option<Vec<u8>>, String> {
    if !v.sound {
        return Ok(None);
    }
    let mut mix = m.sound.clone().unwrap_or_default();
    let mut sources = mix
        .clips
        .iter()
        .map(|c| crate::mix::clip_source(root, c))
        .collect::<Result<Vec<_>, _>>()?;
    let own_track = mix.clips.iter().map(|c| c.track + 1).max().unwrap_or(0);
    let track = own_track.max(mix.tracks.len() as u32);
    for p in placed.iter().filter(|p| p.sound && p.clip.gain > 0.0) {
        let c = p.clip;
        mix.clips.push(SoundClip {
            track,
            sound: None,
            file: None,
            at: c.at,
            from: c.from,
            to: Some(c.from + p.seconds() * c.speed),
            gain: c.gain,
            fade_in: c.fade_in,
            fade_out: c.fade_out,
            speed: c.speed,
            reverse: false,
            effects: Vec::new(),
        });
        sources.push(std::fs::read(&p.file).map_err(|e| e.to_string())?);
    }
    if mix.clips.is_empty() {
        return Ok(None);
    }
    crate::mix::render(&mix, &sources).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(text: &str) -> PictureClip {
        yog_data::from_text::<PictureClip>(text).unwrap().value
    }

    #[test]
    fn clips_are_cut_fitted_faded_and_laid_over() {
        let (a, b) = (
            clip(r#"{ video: "@V.a", from: 1, to: 3, fade_in: 0.5 }"#),
            clip(r#"{ image: "@I.b", at: 1, to: 2, opacity: 0.5, track: 1 }"#),
        );
        let placed = [
            Placed {
                clip: &a,
                file: PathBuf::new(),
                length: Some(10.0),
                sound: true,
            },
            Placed {
                clip: &b,
                file: PathBuf::new(),
                length: None,
                sound: false,
            },
        ];
        let g = graph(&placed, (64, 36), 30.0, "TAIL");
        assert!(g.starts_with("color=c=black:s=64x36:r=30:d=3[b0];"), "{g}");
        assert!(
            g.contains("[0:v]trim=start=1:end=3,setpts=(PTS-STARTPTS)/1,fps=30,scale=64:36:"),
            "{g}"
        );
        assert!(g.contains("fade=t=in:st=0:d=0.5:alpha=1"), "{g}");
        assert!(
            g.contains("colorchannelmixer=aa=0.5,setpts=PTS+1/TB[c1]"),
            "{g}"
        );
        assert!(
            g.contains("[b1][c1]overlay=eof_action=pass:enable='between(t,1,3)'[b2]"),
            "{g}"
        );
        assert!(g.ends_with("[b2]TAIL[out]"), "{g}");
    }
}

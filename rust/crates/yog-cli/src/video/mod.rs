//! A `.yvideo` rendered as the game plays it (`docs/design/video.md`): its
//! picture as AV1 packets (SVT-AV1, cut out of an IVF stream), its sound as
//! Ogg Vorbis — by ffmpeg.

mod av1;
mod ivf;
mod montage;
mod poster;
mod probe;
mod size;
mod source;

pub(crate) use poster::poster;

use std::path::{Path, PathBuf};
use std::process::Command;

use yog_formats::video::{BEST_QUALITY, Video, WORST_QUALITY};

use crate::ffmpeg::{INPUT, STDOUT, Scratch, run};

/// The picture written as AV1 in IVF on stdout, 8-bit 4:2:0, a keyframe
/// every two seconds (how far back a seek decodes from).
const AV1_OUT: &[&str] = &[
    "-c:v",
    "libsvtav1",
    "-svtav1-params",
    "keyint=2s",
    "-pix_fmt",
    "yuv420p",
    "-colorspace",
    "bt709",
    "-color_range",
    "tv",
    "-f",
    "ivf",
    STDOUT,
];
/// SVT-AV1 logs to stderr unless told otherwise: errors alone.
const SVT_LOG: (&str, &str) = ("SVT_LOG", "1");
/// A video, rendered.
#[derive(Debug)]
pub(crate) struct Rendered {
    /// The size it's shown at; its frames may be bigger (padded up to the
    /// encoder's least), shown from their top-left.
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) fps_num: u32,
    pub(crate) fps_den: u32,
    pub(crate) sizes: Vec<u32>,
    pub(crate) packets: Vec<u8>,
    pub(crate) keyframes: Vec<u32>,
    /// Its sound, when it keeps one and the source has it.
    pub(crate) sound: Option<Vec<u8>>,
}

impl Rendered {
    /// Its definition as video `id`, played as `v` says, with sound event
    /// `sound` (the sound itself stays the caller's).
    pub(crate) fn def(
        self,
        id: String,
        v: &Video,
        sound: Option<String>,
    ) -> yog_registry::VideoDef {
        yog_registry::VideoDef {
            id,
            width: self.width,
            height: self.height,
            fps_num: self.fps_num,
            fps_den: self.fps_den,
            sizes: self.sizes,
            packets: self.packets,
            keyframes: self.keyframes,
            sound,
            volume: v.volume,
            looping: v.looping,
        }
    }
}

/// What a video is made of, its files written in a scratch folder.
enum Made<'a> {
    /// One `.yres` video, at this path.
    Source(PathBuf),
    /// A montage's clips.
    Montage(&'a yog_formats::video::Montage, Vec<montage::Placed<'a>>),
}

/// What `v` (of the mod at `root`) is made of, its files written in `dir`.
fn made<'a>(v: &'a Video, root: &Path, dir: &Scratch) -> Result<Made<'a>, String> {
    match (&v.source, &v.montage) {
        (Some(r), None) => {
            let bytes = crate::media::video(root, &r.set, &r.key)
                .ok_or_else(|| format!("no video `{r}` in the mod's .yres"))?;
            Ok(Made::Source(dir.input(INPUT, &bytes)?))
        }
        (None, Some(m)) => Ok(Made::Montage(m, montage::lay(m, root, dir)?)),
        _ => Err("a video shows a `.yres` video (`source`) or a `montage`".into()),
    }
}

/// ffmpeg making `v`'s picture from `made` (the output left to add); its size.
fn picture(v: &Video, made: &Made) -> Result<(Command, (u32, u32)), String> {
    match made {
        Made::Source(input) => source::picture(v, input),
        Made::Montage(_, placed) => montage::picture(v, placed),
    }
}

/// `v` (of the mod at `root`) rendered.
pub(crate) fn render(v: &Video, root: &Path) -> Result<Rendered, String> {
    if v.quality > WORST_QUALITY {
        return Err(format!(
            "quality {}: AV1's is {BEST_QUALITY} (best) to {WORST_QUALITY}",
            v.quality
        ));
    }
    let dir = Scratch::new()?;
    let made = made(v, root, &dir)?;
    let (mut cmd, (width, height)) = picture(v, &made)?;
    cmd.env(SVT_LOG.0, SVT_LOG.1)
        .args(["-crf", &v.quality.to_string()])
        .args(AV1_OUT);
    let ivf = run(&mut cmd, "encode the video as AV1")?;
    let sound = match &made {
        Made::Source(input) => source::sound(v, input)?,
        Made::Montage(m, placed) => montage::sound(v, m, placed, root)?,
    };
    Ok(Rendered {
        width,
        height,
        sound,
        ..cut(&ivf)?
    })
}

/// The IVF stream `bytes` as a video's packets, its keyframes found.
fn cut(bytes: &[u8]) -> Result<Rendered, String> {
    let ivf = ivf::parse(bytes)?;
    if ivf.packets.is_empty() {
        return Err("the video has no frames".into());
    }
    let keyframes = (0u32..)
        .zip(&ivf.packets)
        .filter(|(_, p)| av1::is_keyframe(p))
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if keyframes.first() != Some(&0) {
        return Err("the video's first frame isn't a keyframe".into());
    }
    Ok(Rendered {
        width: ivf.width,
        height: ivf.height,
        fps_num: ivf.rate,
        fps_den: ivf.scale,
        sizes: ivf.packets.iter().map(|p| p.len() as u32).collect(),
        packets: ivf.packets.concat(),
        keyframes,
        sound: None,
    })
}

#[cfg(test)]
pub(crate) mod tests;

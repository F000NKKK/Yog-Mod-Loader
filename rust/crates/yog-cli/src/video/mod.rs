//! A `.yvideo` rendered as the game plays it (`docs/design/video.md`): its
//! picture as AV1 packets (SVT-AV1, cut out of an IVF stream), its sound as
//! Ogg Vorbis — by ffmpeg.

mod av1;
mod ivf;
mod probe;
mod size;

use yog_formats::video::{BEST_QUALITY, Video, WORST_QUALITY};

use crate::ffmpeg::{INPUT, OGG_OUT, STDOUT, Scratch, ffmpeg, run};

/// The picture written as AV1 in IVF on stdout, 8-bit 4:2:0, a keyframe
/// every two seconds (how far back a seek decodes from).
const AV1_OUT: &[&str] = &[
    "-c:v",
    "libsvtav1",
    "-svtav1-params",
    "keyint=2s",
    "-pix_fmt",
    "yuv420p",
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

/// `v` rendered from its source's bytes `source` (any file ffmpeg reads).
pub(crate) fn render(v: &Video, source: &[u8]) -> Result<Rendered, String> {
    if v.quality > WORST_QUALITY {
        return Err(format!(
            "quality {}: AV1's is {BEST_QUALITY} (best) to {WORST_QUALITY}",
            v.quality
        ));
    }
    let dir = Scratch::new()?;
    let input = dir.input(INPUT, source)?;
    let src = probe::probe(&input)?;
    let (width, height) = size::shown(v, src.size);
    let mut cmd = ffmpeg();
    cmd.env(SVT_LOG.0, SVT_LOG.1)
        .arg("-i")
        .arg(&input)
        .args(["-an", "-vf", &size::filters(v, (width, height))])
        .args(["-crf", &v.quality.to_string()])
        .args(AV1_OUT);
    let ivf = run(&mut cmd, "encode the video as AV1")?;
    let sound = if v.sound && src.sound {
        let mut cmd = ffmpeg();
        cmd.arg("-i").arg(&input).arg("-vn").args(OGG_OUT);
        Some(run(&mut cmd, "convert the video's sound to Ogg Vorbis")?)
    } else {
        None
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

//! A `.ysound` montage (`yog_formats::montage::Mix`) rendered into the one
//! sound the game plays: each clip's sound cut, reversed, sped up, made
//! louder or quieter, faded and given its effects, then placed; all mixed,
//! the mix's effects over it; Ogg Vorbis out — one ffmpeg run.

use std::fmt::Write as _;

use yog_formats::montage::{Mix, SoundClip, SoundEffect};

/// The rate every clip is brought to before it's sped up (its pitch with it).
const RATE: u32 = 48_000;
/// An echo's loudness in and out (ffmpeg's `aecho` gains).
const ECHO_IN: f32 = 0.8;
const ECHO_OUT: f32 = 0.9;
const MS: f32 = 1000.0;

/// `effect` as an ffmpeg audio filter.
fn filter(effect: &SoundEffect) -> String {
    match effect {
        SoundEffect::Echo { delay, decay } => {
            format!("aecho={ECHO_IN}:{ECHO_OUT}:{}:{decay}", delay * MS)
        }
        SoundEffect::LowPass { hz } => format!("lowpass=f={hz}"),
        SoundEffect::HighPass { hz } => format!("highpass=f={hz}"),
    }
}

/// Clip `c` (input `i`) as a chain of filters ending in `[c<i>]`.
fn clip_chain(i: usize, c: &SoundClip) -> String {
    let mut f = format!("[{i}:a]aresample={RATE},atrim=start={}", c.from);
    if let Some(to) = c.to {
        let _ = write!(f, ":end={to}");
    }
    f.push_str(",asetpts=PTS-STARTPTS");
    if c.reverse {
        f.push_str(",areverse");
    }
    if c.speed != 1.0 {
        let _ = write!(
            f,
            ",asetrate={},aresample={RATE}",
            (RATE as f32 * c.speed).round()
        );
    }
    if c.gain != 1.0 {
        let _ = write!(f, ",volume={}", c.gain);
    }
    if c.fade_in > 0.0 {
        let _ = write!(f, ",afade=t=in:d={}", c.fade_in);
    }
    if c.fade_out > 0.0 {
        // Faded in backwards: out at its end, whatever its length.
        let _ = write!(f, ",areverse,afade=t=in:d={},areverse", c.fade_out);
    }
    effects(&mut f, &c.effects);
    let _ = write!(f, ",adelay={}:all=1[c{i}]", (c.at * MS).round());
    f
}

/// `effects` appended to filter chain `f`.
fn effects(f: &mut String, effects: &[SoundEffect]) {
    for e in effects {
        let _ = write!(f, ",{}", filter(e));
    }
}

/// The whole graph: every clip; each track's heard clips mixed through its
/// channel (`[t<n>]`); the tracks mixed through the master; out as `[out]`.
pub(crate) fn graph(mix: &Mix) -> String {
    let mut g: Vec<String> = mix
        .clips
        .iter()
        .enumerate()
        .map(|(i, c)| clip_chain(i, c))
        .collect();
    let mut tracks: Vec<u32> = mix
        .clips
        .iter()
        .map(|c| c.track)
        .filter(|t| mix.heard(*t))
        .collect();
    tracks.sort_unstable();
    tracks.dedup();
    for &t in &tracks {
        let ins: String = (0..mix.clips.len())
            .filter(|&i| mix.clips[i].track == t)
            .map(|i| format!("[c{i}]"))
            .collect();
        let n = ins.matches('[').count();
        let mut f = format!("{ins}amix=inputs={n}:normalize=0:duration=longest");
        if let Some(track) = mix.tracks.get(t as usize) {
            if track.gain != 1.0 {
                let _ = write!(f, ",volume={}", track.gain);
            }
            effects(&mut f, &track.effects);
        }
        let _ = write!(f, "[t{t}]");
        g.push(f);
    }
    let ins: String = tracks.iter().map(|t| format!("[t{t}]")).collect();
    let mut master = format!(
        "{ins}amix=inputs={}:normalize=0:duration=longest",
        tracks.len()
    );
    if mix.gain != 1.0 {
        let _ = write!(master, ",volume={}", mix.gain);
    }
    effects(&mut master, &mix.effects);
    master.push_str("[out]");
    g.push(master);
    g.join(";")
}

/// `mix` rendered, its clips' sounds `sources` (in order, any format ffmpeg reads).
pub(crate) fn render(mix: &Mix, sources: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    if !mix.clips.iter().any(|c| mix.heard(c.track)) {
        return Err("a mix has at least one clip on a track that's heard".into());
    }
    let dir = crate::ffmpeg::Scratch::new()?;
    let mut cmd = crate::ffmpeg::ffmpeg();
    for (i, bytes) in sources.iter().enumerate() {
        cmd.arg("-i").arg(dir.input(&i.to_string(), bytes)?);
    }
    cmd.args(["-filter_complex", &graph(mix), "-map", "[out]"])
        .args(crate::ffmpeg::OGG_OUT);
    crate::ffmpeg::run(&mut cmd, "render the mix")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_go_through_their_channels_and_the_muted_are_left_out() {
        let text = r#"{ tracks: [{ gain: 0.5, effects: [{ fx: "high_pass", hz: 900 }] }, { mute: true }],
            clips: [{ sound: "@S.a" }, { sound: "@S.b", track: 1 }, { sound: "@S.c", track: 2 }], gain: 2 }"#;
        let g = graph(&yog_data::from_text::<Mix>(text).unwrap().value);
        assert!(
            g.contains(
                "[c0]amix=inputs=1:normalize=0:duration=longest,volume=0.5,highpass=f=900[t0]"
            ),
            "{g}"
        );
        assert!(!g.contains("[t1]"), "{g}");
        assert!(
            g.ends_with("[t0][t2]amix=inputs=2:normalize=0:duration=longest,volume=2[out]"),
            "{g}"
        );
    }

    #[test]
    fn a_mix_is_one_ffmpeg_graph() {
        let text = r#"{ clips: [{ sound: "@S.a", at: 0.5, to: 1, speed: 2, fade_out: 0.1, effects: [{ fx: "low_pass", hz: 800 }] },
            { file: "dig/sand1" }], effects: [{ fx: "echo", delay: 0.2, decay: 0.4 }] }"#;
        let g = graph(&yog_data::from_text::<Mix>(text).unwrap().value);
        assert!(
            g.starts_with(
                "[0:a]aresample=48000,atrim=start=0:end=1,asetpts=PTS-STARTPTS,asetrate=96000"
            ),
            "{g}"
        );
        assert!(g.contains("lowpass=f=800,adelay=500:all=1[c0]"), "{g}");
        assert!(
            g.ends_with(
                "[c0][c1]amix=inputs=2:normalize=0:duration=longest[t0];[t0]amix=inputs=1:normalize=0:duration=longest,aecho=0.8:0.9:200:0.4[out]"
            ),
            "{g}"
        );
    }
}

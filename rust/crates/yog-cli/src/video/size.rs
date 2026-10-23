//! A video's picture size and the filters that make it: the size asked for
//! (one side given, the other keeps the source's proportion), even (4:2:0
//! halves the colour's resolution), padded at the right and bottom up to
//! the encoder's least — the game shows the top-left part it asked for.

use yog_formats::video::Video;

/// SVT-AV1's least width and height.
pub(super) const ENCODER_MIN: u32 = 64;
/// Sizes are multiples of this, and at least it.
const EVEN: u32 = 2;

/// `n` rounded down to even, at least [`EVEN`].
fn even(n: u32) -> u32 {
    (n - n % EVEN).max(EVEN)
}

/// `side` of a source `(along, across)` scaled so `across` becomes `to`.
fn proportion(side: u32, across: u32, to: u32) -> u32 {
    (f64::from(side) * f64::from(to) / f64::from(across.max(1))).round() as u32
}

/// The size `v` is shown at, its source `(sw, sh)`.
pub(super) fn shown(v: &Video, (sw, sh): (u32, u32)) -> (u32, u32) {
    let (w, h) = match (v.width, v.height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, proportion(sh, sw, w)),
        (None, Some(h)) => (proportion(sw, sh, h), h),
        (None, None) => (sw, sh),
    };
    (even(w), even(h))
}

/// `v`'s picture filters, shown at `(w, h)`: its frame rate, its size, its padding.
pub(super) fn filters(v: &Video, (w, h): (u32, u32)) -> String {
    let mut f = v.fps.map(|fps| format!("fps={fps},")).unwrap_or_default();
    f.push_str(&format!("scale={w}:{h}"));
    if w < ENCODER_MIN || h < ENCODER_MIN {
        let (pw, ph) = (w.max(ENCODER_MIN), h.max(ENCODER_MIN));
        f.push_str(&format!(",pad={pw}:{ph}"));
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(text: &str) -> Video {
        yog_data::from_text::<Video>(text).unwrap().value
    }

    #[test]
    fn one_side_keeps_the_proportion_and_sizes_are_even() {
        let src = (161, 91);
        assert_eq!(shown(&video(r#"{ source: "@V.a" }"#), src), (160, 90));
        assert_eq!(
            shown(&video(r#"{ source: "@V.a", width: 64 }"#), src),
            (64, 36)
        );
        assert_eq!(
            shown(&video(r#"{ source: "@V.a", height: 1 }"#), src),
            (2, 2)
        );
    }

    #[test]
    fn a_small_picture_is_padded_up_to_the_encoders_least() {
        let v = video(r#"{ source: "@V.a", fps: 12.5 }"#);
        assert_eq!(filters(&v, (320, 180)), "fps=12.5,scale=320:180");
        assert_eq!(filters(&v, (64, 36)), "fps=12.5,scale=64:36,pad=64:64");
    }
}

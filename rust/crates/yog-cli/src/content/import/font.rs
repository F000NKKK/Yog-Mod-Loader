//! `yog import font <yog:font/name> [--id <path>] [--chars <ranges>] [--force]`
//! — a copy of one of the game's fonts as the mod's own `.yfont`
//! (`content/font/<name>.yfont`): what editing a game font starts from.
//! Each glyph is the game's picture of it, tinted by the text's color, at
//! the game's metrics. `--chars 0400-04FF,2665` picks the characters
//! (hex code points); without it Unifont's tens of thousands stay out.

use yog_font::{CharGlyph, Font, Glyph};
use yog_graphics::{Image, ImageOp, Op, Rect};
use yog_ids::Format;

use super::Target;
use crate::design::assets::Resources;
use crate::design::font::{Font as GameFont, Glyph as GameGlyph, GlyphBitmap};

/// Font units a GUI pixel is: fine enough for Unifont's half pixels and
/// TrueType's fractional advances.
const UNITS_PER_PX: f32 = 8.0;
/// GUI pixels an em is: the game's line height.
const EM_PX: f32 = 9.0;
/// A pixel's alpha, in its RGBA.
const ALPHA: usize = 3;
/// `--chars`: ranges apart, a range's ends apart, the radix.
const RANGE_LIST: char = ',';
const RANGE_TO: char = '-';
const HEX: u32 = 16;

/// Imports the game font Yog's `alias` names into `target`, as `id` (its
/// path under `content/`; by default `font/<the game's name>`).
pub(super) fn import_font(
    target: &mut Target,
    alias: &str,
    id: Option<&str>,
    ranges: Option<&str>,
) -> Result<(), String> {
    let game = yog_ids::game_font_of(alias).ok_or_else(|| {
        format!(
            "{alias} isn't Yog's alias of a game font ({}<name>)",
            yog_ids::yog::FONT_ALIAS
        )
    })?;
    let res = Resources::for_mod(Some(&target.root));
    if res.vanilla.is_none() {
        return Err("no Minecraft client jar found to read the game's fonts from".into());
    }
    let source = GameFont::load(&res, alias);
    let chars = match ranges {
        Some(ranges) => {
            let ranges = parse_ranges(ranges)?;
            source
                .chars(true)
                .into_iter()
                .filter(|&c| ranges.iter().any(|r| r.contains(&(c as u32))))
                .collect()
        }
        None => {
            let own = source.chars(false);
            if source.chars(true).len() > own.len() {
                target.warnings.push(format!(
                    "{game}: Unifont's glyphs left out (--chars picks some)"
                ));
            }
            own
        }
    };
    if !chars.iter().any(|&c| source.glyph(c).bitmap.is_some()) {
        return Err(format!(
            "{game}: no glyphs to import — pick its characters with --chars <from>-<to>,… (hex)"
        ));
    }
    let font = convert(&source, &chars);
    font.validate().map_err(|e| e.join("; "))?;
    let path = id.map_or_else(|| copy_path(yog_ids::split(&game).1), str::to_owned);
    let rel = format!("{}/{}", yog_ids::project::CONTENT, Format::Font.file(&path));
    target.write_file(&rel, yog_data::to_text(&font).as_bytes())
}

/// Where the copy of the game's font `name` goes by default, under
/// `content/` (and so its id's path): `font/<name>`.
pub(crate) fn copy_path(name: &str) -> String {
    format!(
        "{}{}{name}",
        yog_ids::project::FONT_COPIES,
        yog_ids::sep::PATH
    )
}

/// `0400-04FF,2665`: code point ranges, hex.
fn parse_ranges(text: &str) -> Result<Vec<std::ops::RangeInclusive<u32>>, String> {
    let point = |p: &str| {
        u32::from_str_radix(p.trim().trim_start_matches("U+"), HEX)
            .map_err(|_| format!("--chars: {p} isn't a hex code point"))
    };
    text.split(RANGE_LIST)
        .map(|part| match part.split_once(RANGE_TO) {
            Some((from, to)) => Ok(point(from)?..=point(to)?),
            None => point(part).map(|p| p..=p),
        })
        .collect()
}

/// `chars` of `source`, each its own glyph.
fn convert(source: &GameFont, chars: &[char]) -> Font {
    let units = |px: f32| (px * UNITS_PER_PX).round() as i32;
    let mut font = Font {
        units_per_em: units(EM_PX) as u32,
        ascent: units(GameFont::BASELINE),
        descent: units(GameFont::BASELINE - EM_PX),
        ..Font::default()
    };
    for &ch in chars {
        font.chars.push(CharGlyph {
            char: ch as u32,
            glyph: font.glyphs.len() as u32,
        });
        font.glyphs.push(glyph(ch, &source.glyph(ch)));
    }
    font
}

/// The game's glyph for `ch` as a `.yfont` one: its picture, cut to what
/// shows, placed where the game draws it, `y` up from the baseline.
fn glyph(ch: char, g: &GameGlyph) -> Glyph {
    let ops = g.bitmap.as_ref().and_then(picture).into_iter().collect();
    Glyph {
        name: ch.to_string(),
        advance: (g.advance * UNITS_PER_PX).round() as i32,
        ops,
        entry: None,
        exit: None,
    }
}

/// A glyph's bitmap as an image op: the box its visible pixels fill.
fn picture(b: &GlyphBitmap) -> Option<Op> {
    let image = Image {
        width: b.w,
        height: b.h,
        rgba: b.rgba.clone(),
    };
    let pixels = image.pixels();
    let shows = |x: u32, y: u32| pixels[(y * b.w + x) as usize][ALPHA] != 0;
    let cols: Vec<u32> = (0..b.w)
        .filter(|&x| (0..b.h).any(|y| shows(x, y)))
        .collect();
    let rows: Vec<u32> = (0..b.h)
        .filter(|&y| (0..b.w).any(|x| shows(x, y)))
        .collect();
    let (&left, &right, &top, &bottom) = (cols.first()?, cols.last()?, rows.first()?, rows.last()?);
    let (w, h) = (right - left + 1, bottom - top + 1);
    let mut cut = Image::new(w, h);
    for (y, row) in cut.pixels_mut().chunks_mut(w as usize).enumerate() {
        let from = ((top + y as u32) * b.w + left) as usize;
        row.copy_from_slice(&pixels[from..from + w as usize]);
    }
    let px = |n: u32| n as f32 / b.oversample;
    let baseline_to_top = GameFont::BASELINE - b.top - px(top);
    let rect = Rect::new(
        px(left) * UNITS_PER_PX,
        (baseline_to_top - px(h)) * UNITS_PER_PX,
        px(w) * UNITS_PER_PX,
        px(h) * UNITS_PER_PX,
    );
    Some(Op::Image(ImageOp {
        rect,
        image: cut,
        tinted: true,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_keeps_the_games_picture_and_metrics() {
        let g = GameGlyph {
            advance: 6.0,
            shadow_offset: 1.0,
            bold_offset: 1.0,
            bitmap: Some(GlyphBitmap {
                w: 8,
                h: 8,
                rgba: (0..8 * 8)
                    .flat_map(|i| if i % 8 < 5 { [0xFF; 4] } else { [0; 4] })
                    .collect(),
                oversample: 1.0,
                top: 0.0,
            }),
        };
        let out = glyph('A', &g);
        assert_eq!(out.advance, 48);
        let [Op::Image(image)] = out.ops.as_slice() else {
            panic!("{:?}", out.ops)
        };
        // Cut to its 5 visible columns; its top at the line's top (7 px above the baseline), 8 px tall.
        assert_eq!(image.rect, Rect::new(0.0, -8.0, 40.0, 64.0));
        assert_eq!((image.image.width, image.image.height), (5, 8));
        assert!(image.tinted);
        let blank = GameGlyph {
            bitmap: Some(GlyphBitmap {
                w: 2,
                h: 2,
                rgba: vec![0; 16],
                oversample: 1.0,
                top: 0.0,
            }),
            ..g
        };
        assert!(glyph(' ', &blank).ops.is_empty());
    }

    #[test]
    fn chars_are_hex_ranges() {
        assert_eq!(
            parse_ranges("0400-04FF,U+2665").unwrap(),
            vec![0x400..=0x4FF, 0x2665..=0x2665]
        );
        assert!(parse_ranges("zz").is_err());
    }
}

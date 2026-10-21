//! Minecraft's text renderer, reproduced from its font definitions: the
//! `font/*.json` provider lists (`reference`, `space`, `bitmap`, `unihex`,
//! `ttf`)
//! resolved through [`Resources`], glyph metrics computed the way the game
//! computes them (bitmap advance = trimmed width × scale + 1, unifont
//! advance = width / 2 + 1, ...), and `§` formatting codes. Text drawn
//! through this matches the game pixel for pixel.

use std::collections::HashMap;
use std::io::Read;
use yog_sync::{Arc, Mutex, OnceLock};

use serde_json::Value;

use super::assets::{Image, Resources, decode_png};

/// One glyph's metrics and (unless it's blank) its bitmap.
pub struct Glyph {
    /// Pen advance in GUI pixels.
    pub advance: f32,
    pub shadow_offset: f32,
    pub bold_offset: f32,
    pub bitmap: Option<GlyphBitmap>,
}

/// RGBA texels; drawn `w / oversample` × `h / oversample` GUI pixels big,
/// `top` GUI pixels below the text's y.
pub struct GlyphBitmap {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
    pub oversample: f32,
    pub top: f32,
}

struct BitmapProvider {
    image: Arc<Image>,
    cell_w: u32,
    cell_h: u32,
    scale: f32,
    ascent: i32,
    chars: HashMap<char, (u32, u32)>,
}

struct UnihexProvider {
    /// Rows left-aligned in a u32 (bit 31 = leftmost pixel) + bit width.
    glyphs: Arc<HashMap<u32, ([u32; 16], u32)>>,
    overrides: Vec<(u32, u32, i32, i32)>,
}

/// A TrueType font (`"type": "ttf"`): glyphs rasterized at `size` ×
/// `oversample` pixels, drawn `size` GUI pixels high.
struct TtfProvider {
    font: fontdue::Font,
    size: f32,
    oversample: f32,
    shift: (f32, f32),
    skip: Vec<char>,
}

enum Provider {
    Space(HashMap<char, f32>),
    Bitmap(BitmapProvider),
    Unihex(Arc<UnihexProvider>),
    Ttf(Arc<TtfProvider>),
    /// One of the mod's own fonts.
    Yog(Arc<super::font_yog::YogProvider>),
}

pub struct Font {
    providers: Vec<Provider>,
    cache: Mutex<HashMap<char, Arc<Glyph>>>,
}

/// JSON the way Minecraft reads it (Gson, lenient): trailing commas, as
/// in the game's own `font/include/unifont.json`, are fine.
fn lenient_json(bytes: &[u8]) -> Option<Value> {
    if let Ok(v) = serde_json::from_slice(bytes) {
        return Some(v);
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut out = String::with_capacity(text.len());
    let (mut in_str, mut escaped) = (false, false);
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if in_str {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        if c == '"' {
            in_str = true;
        } else if c == ','
            && chars[i + 1..]
                .iter()
                .find(|c| !c.is_whitespace())
                .is_some_and(|&n| n == ']' || n == '}')
        {
            continue;
        }
        out.push(c);
    }
    serde_json::from_str(&out).ok()
}

fn json(res: &Resources, ns: &str, path: &str) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some(v) = &res.vanilla {
        out.extend(
            v.read_all(&format!("{ns}/{path}"))
                .iter()
                .filter_map(|b| lenient_json(b)),
        );
    }
    out
}

/// A provider's `filter` passes with every font option off (no forced
/// unicode font, no Japanese glyph variants) — the game's defaults.
fn filter_passes(p: &Value) -> bool {
    p.get("filter")
        .and_then(Value::as_object)
        .is_none_or(|f| f.values().all(|v| v.as_bool() == Some(false)))
}

/// Unifont `.hex` files from a zip, cached per resource id (the file is big).
fn unihex(res: &Resources, file: &str, overrides: &[Value]) -> Option<Arc<UnihexProvider>> {
    type Cache = Mutex<HashMap<String, Arc<HashMap<u32, ([u32; 16], u32)>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let glyphs = if let Some(hit) = cache.lock().get(file).cloned() {
        hit
    } else {
        let (ns, path) = yog_ids::split(file);
        let bytes = res.read(ns, path)?;
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
        let mut glyphs = HashMap::new();
        for i in 0..zip.len() {
            let Ok(mut entry) = zip.by_index(i) else {
                continue;
            };
            if !entry.name().is_ok_and(|n| n.ends_with(".hex")) {
                continue;
            }
            let mut text = String::new();
            if entry.read_to_string(&mut text).is_err() {
                continue;
            }
            for line in text.lines() {
                let Some((cp, data)) = line.split_once(':') else {
                    continue;
                };
                let Ok(cp) = u32::from_str_radix(cp.trim(), 16) else {
                    continue;
                };
                let data = data.trim();
                let row_digits = data.len() / 16;
                if !matches!(row_digits, 2 | 4 | 6 | 8) || data.len() % 16 != 0 {
                    continue;
                }
                let mut rows = [0u32; 16];
                let mut ok = true;
                for (r, row) in rows.iter_mut().enumerate() {
                    match u32::from_str_radix(&data[r * row_digits..(r + 1) * row_digits], 16) {
                        Ok(v) => *row = v << (32 - row_digits * 4),
                        Err(_) => ok = false,
                    }
                }
                if ok {
                    glyphs.insert(cp, (rows, (row_digits * 4) as u32));
                }
            }
        }
        let glyphs = Arc::new(glyphs);
        let mut c = cache.lock();
        c.insert(file.to_string(), glyphs.clone());
        glyphs
    };
    let overrides = overrides
        .iter()
        .filter_map(|o| {
            let ch = |k: &str| o.get(k)?.as_str()?.chars().next().map(|c| c as u32);
            Some((
                ch("from")?,
                ch("to")?,
                o.get("left")?.as_i64()? as i32,
                o.get("right")?.as_i64()? as i32,
            ))
        })
        .collect();
    Some(Arc::new(UnihexProvider { glyphs, overrides }))
}

fn load_providers(res: &Resources, id: &str, depth: usize, out: &mut Vec<Provider>) {
    if depth > 8 {
        return;
    }
    let (ns, path) = yog_ids::split(id);
    for def in json(res, ns, &format!("font/{path}.json")) {
        let Some(list) = def.get("providers").and_then(Value::as_array) else {
            continue;
        };
        for p in list {
            if !filter_passes(p) {
                continue;
            }
            match p.get("type").and_then(Value::as_str) {
                Some("reference") => {
                    if let Some(id) = p.get("id").and_then(Value::as_str) {
                        load_providers(res, id, depth + 1, out);
                    }
                }
                Some("space") => {
                    let advances = p
                        .get("advances")
                        .and_then(Value::as_object)
                        .map(|a| {
                            a.iter()
                                .filter_map(|(k, v)| Some((k.chars().next()?, v.as_f64()? as f32)))
                                .collect()
                        })
                        .unwrap_or_default();
                    out.push(Provider::Space(advances));
                }
                Some("bitmap") => {
                    if let Some(b) = bitmap(res, p) {
                        out.push(Provider::Bitmap(b));
                    }
                }
                Some("unihex") => {
                    let file = p
                        .get("hex_file")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let overrides = p
                        .get("size_overrides")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    if let Some(u) = unihex(res, file, &overrides) {
                        out.push(Provider::Unihex(u));
                    }
                }
                Some("ttf") => {
                    if let Some(t) = ttf(res, p) {
                        out.push(Provider::Ttf(Arc::new(t)));
                    }
                }
                _ => {} // legacy_unicode: not reproduced — the next provider (or the missing glyph) answers
            }
        }
    }
}

/// The least a TrueType font is oversampled by.
const MIN_OVERSAMPLE: f64 = 0.1;
/// A TrueType provider's size when it gives none, and a bitmap one's height
/// (the game's defaults); where the default font's baseline is, from a
/// line's top.
const TTF_SIZE: f64 = 11.0;
const BITMAP_HEIGHT: f64 = 8.0;
const BASELINE: f32 = 7.0;

fn ttf(res: &Resources, p: &Value) -> Option<TtfProvider> {
    let file = p.get("file")?.as_str()?;
    let (ns, path) = yog_ids::split(file);
    let bytes = res.read(ns, &format!("font/{path}"))?;
    let size = p.get("size").and_then(Value::as_f64).unwrap_or(TTF_SIZE) as f32;
    let oversample = p.get("oversample").and_then(Value::as_f64).unwrap_or(1.0).max(MIN_OVERSAMPLE) as f32;
    let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings { scale: size * oversample, ..Default::default() }).ok()?;
    let shift = p
        .get("shift")
        .and_then(Value::as_array)
        .map(|a| (a.first().and_then(Value::as_f64).unwrap_or(0.0) as f32, a.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32))
        .unwrap_or((0.0, 0.0));
    let skip = match p.get("skip") {
        Some(Value::String(s)) => s.chars().collect(),
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).flat_map(str::chars).collect(),
        _ => Vec::new(),
    };
    Some(TtfProvider { font, size, oversample, shift, skip })
}

impl TtfProvider {
    fn glyph(&self, ch: char) -> Option<Glyph> {
        if self.skip.contains(&ch) || self.font.lookup_glyph_index(ch) == 0 {
            return None;
        }
        let px = self.size * self.oversample;
        let (m, coverage) = self.font.rasterize(ch, px);
        let advance = m.advance_width / self.oversample;
        if m.width == 0 || m.height == 0 {
            return Some(Glyph { advance, shadow_offset: 1.0, bold_offset: 1.0, bitmap: None });
        }
        // Left bearing as blank columns, so the bitmap starts at the pen.
        let pad = (m.xmin.max(0) as f32 + self.shift.0 * self.oversample).round().max(0.0) as usize;
        let w = m.width + pad;
        let mut rgba = vec![0u8; w * m.height * 4];
        for y in 0..m.height {
            for x in 0..m.width {
                let a = coverage[y * m.width + x];
                let i = (y * w + x + pad) * 4;
                rgba[i..i + 4].copy_from_slice(&[255, 255, 255, a]);
            }
        }
        // The baseline sits where the default font's does.
        let top = BASELINE - (m.ymin as f32 + m.height as f32) / self.oversample + self.shift.1;
        Some(Glyph {
            advance,
            shadow_offset: 1.0,
            bold_offset: 1.0,
            bitmap: Some(GlyphBitmap { w: w as u32, h: m.height as u32, rgba, oversample: self.oversample, top }),
        })
    }
}

fn bitmap(res: &Resources, p: &Value) -> Option<BitmapProvider> {
    let file = p.get("file")?.as_str()?;
    let (ns, path) = yog_ids::split(file);
    let image = Arc::new(decode_png(&res.read(ns, &format!("{}{path}", yog_ids::res::TEXTURES))?)?);
    let rows: Vec<Vec<char>> = p
        .get("chars")?
        .as_array()?
        .iter()
        .filter_map(|r| Some(r.as_str()?.chars().collect()))
        .collect();
    let cols = rows.first()?.len() as u32;
    if cols == 0 || rows.is_empty() {
        return None;
    }
    let cell_w = image.width / cols;
    let cell_h = image.height / rows.len() as u32;
    let height = p.get("height").and_then(Value::as_f64).unwrap_or(BITMAP_HEIGHT) as f32;
    let mut chars = HashMap::new();
    for (r, row) in rows.iter().enumerate() {
        for (c, &ch) in row.iter().enumerate() {
            if ch != '\0' {
                chars.entry(ch).or_insert((c as u32, r as u32));
            }
        }
    }
    Some(BitmapProvider {
        image,
        cell_w,
        cell_h,
        scale: height / cell_h as f32,
        ascent: p.get("ascent").and_then(Value::as_i64).unwrap_or(7) as i32,
        chars,
    })
}

impl BitmapProvider {
    fn glyph(&self, ch: char) -> Option<Glyph> {
        let &(col, row) = self.chars.get(&ch)?;
        let (x0, y0) = (col * self.cell_w, row * self.cell_h);
        // Trimmed width: the rightmost column with any visible texel.
        let actual = (0..self.cell_w)
            .rev()
            .find(|&x| (0..self.cell_h).any(|y| self.image.pixel(x0 + x, y0 + y).a() != 0))
            .map_or(0, |x| x + 1);
        let mut rgba = Vec::with_capacity((self.cell_w * self.cell_h * 4) as usize);
        for y in 0..self.cell_h {
            for x in 0..self.cell_w {
                rgba.extend_from_slice(&self.image.pixel(x0 + x, y0 + y).to_rgba8());
            }
        }
        Some(Glyph {
            advance: (0.5 + actual as f32 * self.scale).floor() + 1.0,
            shadow_offset: 1.0,
            bold_offset: 1.0,
            bitmap: Some(GlyphBitmap {
                w: self.cell_w,
                h: self.cell_h,
                rgba,
                oversample: 1.0 / self.scale,
                top: (7 - self.ascent) as f32,
            }),
        })
    }
}

impl UnihexProvider {
    fn glyph(&self, ch: char) -> Option<Glyph> {
        let cp = ch as u32;
        let (rows, bit_width) = self.glyphs.get(&cp)?;
        let (left, right) = match self.overrides.iter().find(|o| (o.0..=o.1).contains(&cp)) {
            Some(&(_, _, l, r)) => (l, r),
            None => {
                let all = rows.iter().fold(0u32, |a, r| a | r);
                if all == 0 {
                    (0, *bit_width as i32 - 1)
                } else {
                    (all.leading_zeros() as i32, 31 - all.trailing_zeros() as i32)
                }
            }
        };
        let width = (right - left + 1).max(0) as u32;
        let mut rgba = Vec::with_capacity((width * 16 * 4) as usize);
        for row in rows {
            for x in 0..width {
                let bit = left + x as i32;
                let on = (0..32).contains(&bit) && row & (1u32 << (31 - bit)) != 0;
                rgba.extend_from_slice(if on { &[0xFF; 4] } else { &[0; 4] });
            }
        }
        Some(Glyph {
            advance: (width / 2 + 1) as f32,
            shadow_offset: 0.5,
            bold_offset: 0.5,
            bitmap: (width > 0).then_some(GlyphBitmap {
                w: width,
                h: 16,
                rgba,
                oversample: 2.0,
                top: 0.0,
            }),
        })
    }
}

/// The game's "missing" glyph: a 5×8 box outline, advance 6.
fn missing_glyph() -> Glyph {
    let mut rgba = Vec::with_capacity(5 * 8 * 4);
    for y in 0..8 {
        for x in 0..5 {
            let edge = x == 0 || x == 4 || y == 0 || y == 7;
            rgba.extend_from_slice(if edge { &[0xFF; 4] } else { &[0; 4] });
        }
    }
    Glyph {
        advance: yog_ui::text::CHAR_W,
        shadow_offset: 1.0,
        bold_offset: 1.0,
        bitmap: Some(GlyphBitmap {
            w: 5,
            h: 8,
            rgba,
            oversample: 1.0,
            top: 0.0,
        }),
    }
}

impl Font {
    /// `font_id` like `yog:font/uniform` (Yog's alias of a game font) or
    /// one of the mod's own (`mymod:font/fancy`); `""` is the default font.
    pub fn load(res: &Resources, font_id: &str) -> Arc<Font> {
        // A mod's own font is read again once its file changes.
        type Cache = Mutex<HashMap<(Option<std::path::PathBuf>, String, Option<std::time::SystemTime>), Arc<Font>>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let named = font_id.trim();
        let game = yog_ids::game_font_of(named);
        let id = match &game {
            Some(game) => game.as_str(),
            None if named.is_empty() => yog_ids::game::DEFAULT_FONT,
            None => named,
        };
        let key = (res.mod_root.clone(), id.to_string(), super::font_yog::changed(res, id));
        let cache = CACHE.get_or_init(Default::default);
        if let Some(f) = cache.lock().get(&key).cloned() {
            return f;
        }
        let mut providers = Vec::new();
        match super::font_yog::load(res, id) {
            Some(own) => providers.push(Provider::Yog(own)),
            None => load_providers(res, id, 0, &mut providers),
        }
        if providers.is_empty() && id != yog_ids::game::DEFAULT_FONT {
            // An unknown font falls back to the default one, as in game.
            load_providers(res, yog_ids::game::DEFAULT_FONT, 0, &mut providers);
        }
        let font = Arc::new(Font {
            providers,
            cache: Mutex::new(HashMap::new()),
        });
        let mut c = cache.lock();
        c.insert(key, font.clone());
        font
    }

    /// The characters its providers draw, in order; Unifont's (tens of
    /// thousands) only when `unihex`.
    pub fn chars(&self, unihex: bool) -> Vec<char> {
        let mut out: Vec<char> = self
            .providers
            .iter()
            .flat_map(|p| -> Vec<char> {
                match p {
                    Provider::Space(a) => a.keys().copied().collect(),
                    Provider::Bitmap(b) => b.chars.keys().copied().collect(),
                    Provider::Unihex(u) if unihex => u.glyphs.keys().filter_map(|&c| char::from_u32(c)).collect(),
                    Provider::Unihex(_) => Vec::new(),
                    Provider::Ttf(t) => t.font.chars().keys().filter(|c| !t.skip.contains(c)).copied().collect(),
                    Provider::Yog(y) => y.chars().collect(),
                }
            })
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// The text's baseline, in GUI pixels below its top.
    pub const BASELINE: f32 = BASELINE;

    /// Whether any glyph source was found (else everything is the missing box).
    pub fn is_real(&self) -> bool {
        !self.providers.is_empty()
    }

    pub fn glyph(&self, ch: char) -> Arc<Glyph> {
        if let Some(g) = self.cache.lock().get(&ch).cloned() {
            return g;
        }
        let g = Arc::new(
            self.providers
                .iter()
                .find_map(|p| match p {
                    Provider::Space(a) => a.get(&ch).map(|&advance| Glyph {
                        advance,
                        shadow_offset: 1.0,
                        bold_offset: 1.0,
                        bitmap: None,
                    }),
                    Provider::Bitmap(b) => b.glyph(ch),
                    Provider::Unihex(u) => u.glyph(ch),
                    Provider::Ttf(t) => t.glyph(ch),
                    Provider::Yog(y) => y.glyph(ch, BASELINE),
                })
                .unwrap_or_else(missing_glyph),
        );
        let mut c = self.cache.lock();
        c.insert(ch, g.clone());
        g
    }

    /// Width in GUI pixels, as the game's `Font.width` measures it (`§` codes skipped, bold +offset).
    pub fn width(&self, text: &str) -> f32 {
        styled(text, yog_gfx::Color::WHITE.0)
            .iter()
            .map(|s| {
                self.glyph(s.ch).advance
                    + if s.bold {
                        self.glyph(s.ch).bold_offset
                    } else {
                        0.0
                    }
            })
            .sum()
    }
}

/// One character of a string after `§` formatting codes.
#[derive(Clone, Copy)]
pub struct Styled {
    pub ch: char,
    pub color: u32,
    pub bold: bool,
    pub underline: bool,
    pub strike: bool,
}

/// A legacy chat color's code: `5` dark purple, `a` green.
pub fn chat_color(code: char) -> Option<u32> {
    code.to_digit(CODE_RADIX).map(|i| yog_gfx::Color::rgb(CODES[i as usize]).0)
}

/// Chat color codes are hex digits.
const CODE_RADIX: u32 = 16;

/// Legacy chat colors `§0`–`§f`.
const CODES: [u32; 16] = [
    0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xFFAA00, 0xAAAAAA, 0x555555,
    0x5555FF, 0x55FF55, 0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF,
];

/// `text` split into characters with the style `§` codes give them —
/// `StringDecomposer.iterateFormatted`.
/// Color `rgb` with `color`'s alpha.
fn with_alpha_of(color: u32, rgb: u32) -> u32 {
    yog_gfx::Color::rgb(rgb).with_alpha(yog_gfx::Color(color).a()).0
}

pub fn styled(text: &str, color: u32) -> Vec<Styled> {
    // Yog rich text spans first (`{b|…}`, `{#rrggbb|…}`), then the game's
    // own `§` codes inside each run.
    let mut out = Vec::new();
    for (run, st) in yog_ui::rich::runs(text) {
        let base = Styled {
            ch: ' ',
            color: st.color.map_or(color, |rgb| with_alpha_of(color, rgb)),
            bold: st.bold,
            underline: st.underline,
            strike: st.strike,
        };
        let mut cur = base;
        let mut it = run.chars();
        while let Some(c) = it.next() {
            if c == '§' {
                let Some(code) = it.next() else { break };
                let code = code.to_ascii_lowercase();
                if let Some(i) = code.to_digit(16) {
                    cur = Styled {
                        color: with_alpha_of(color, CODES[i as usize]),
                        ..base
                    };
                } else {
                    match code {
                        'l' => cur.bold = true,
                        'n' => cur.underline = true,
                        'm' => cur.strike = true,
                        'r' => cur = base,
                        _ => {} // italic / obfuscated: drawn plain
                    }
                }
                continue;
            }
            out.push(Styled { ch: c, ..cur });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_is_read_leniently_like_gson() {
        let v = lenient_json(br#"{"a": [1, 2,], "s": "x,]",}"#).unwrap();
        assert_eq!(v["a"][1], 2);
        assert_eq!(v["s"], "x,]");
    }

    #[test]
    fn styled_text_follows_rich_spans() {
        let s = styled("a{b #102030|b}c", 0xFF_FFFFFF);
        assert_eq!(s.iter().map(|c| c.ch).collect::<String>(), "abc");
        assert!(s[1].bold && s[1].color == 0xFF_102030 && !s[2].bold);
    }

    #[test]
    fn styled_text_follows_section_codes() {
        let s = styled("a§cb§lc§rd", 0xFF_FFFFFF);
        assert_eq!(s.iter().map(|c| c.ch).collect::<String>(), "abcd");
        assert_eq!(s[1].color, 0xFF_FF5555);
        assert!(s[2].bold && !s[3].bold);
        assert_eq!(s[3].color, 0xFF_FFFFFF);
    }

    #[test]
    fn without_game_files_text_measures_with_the_missing_glyph() {
        let f = Font::load(
            &Resources {
                mod_root: None,
                vanilla: None,
            },
            yog_ids::game::DEFAULT_FONT,
        );
        assert!(!f.is_real());
        assert_eq!(f.width("ab"), 12.0);
    }
}

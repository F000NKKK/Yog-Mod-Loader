//! Colors: `0xAARRGGBB`, straight alpha — what every Yog format and API
//! passes.

/// A color, `0xAARRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Color(pub u32);

/// Where alpha sits in `0xAARRGGBB`, and the bits it and the rest take.
const ALPHA_SHIFT: u32 = 24;
const ALPHA_BITS: u32 = 0xff00_0000;
const RGB_BITS: u32 = 0x00ff_ffff;
/// A channel's top, as a fraction's 1.
const CHANNEL_MAX: f32 = u8::MAX as f32;

impl Color {
    pub const TRANSPARENT: Color = Color(0);
    pub const WHITE: Color = Color(0xffff_ffff);
    pub const BLACK: Color = Color(0xff00_0000);

    pub const fn argb(a: u8, r: u8, g: u8, b: u8) -> Color {
        Color((a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32)
    }

    /// Opaque `0xRRGGBB`.
    pub const fn rgb(rgb: u32) -> Color {
        Color(ALPHA_BITS | rgb)
    }

    pub const fn a(self) -> u8 {
        (self.0 >> ALPHA_SHIFT) as u8
    }
    pub const fn r(self) -> u8 {
        (self.0 >> 16) as u8
    }
    pub const fn g(self) -> u8 {
        (self.0 >> 8) as u8
    }
    pub const fn b(self) -> u8 {
        self.0 as u8
    }

    /// `[r, g, b, a]` bytes, as an RGBA image keeps them.
    pub const fn to_rgba8(self) -> crate::Rgba8 {
        [self.r(), self.g(), self.b(), self.a()]
    }

    pub const fn from_rgba8(p: crate::Rgba8) -> Color {
        Color::argb(p[3], p[0], p[1], p[2])
    }

    /// `[r, g, b, a]` in `0..=1`.
    pub fn to_f32(self) -> [f32; 4] {
        [self.r(), self.g(), self.b(), self.a()].map(|v| v as f32 / CHANNEL_MAX)
    }

    pub fn from_f32(c: [f32; 4]) -> Color {
        let [r, g, b, a] = c.map(|v| (v.clamp(0.0, 1.0) * CHANNEL_MAX).round() as u8);
        Color::argb(a, r, g, b)
    }

    /// Each channel multiplied by `o`'s (a tint).
    pub fn multiply(self, o: Color) -> Color {
        let (a, b) = (self.to_f32(), o.to_f32());
        Color::from_f32([a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]])
    }

    /// `#RRGGBB` / `#AARRGGBB` (or `0x…`, `_` allowed), or a decimal
    /// `0xAARRGGBB`.
    pub fn parse(value: &str) -> Option<Color> {
        let v = value.trim();
        if !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) {
            return v.parse().ok().map(Color);
        }
        let hex = v
            .strip_prefix('#')
            .or_else(|| v.strip_prefix("0x"))
            .or_else(|| v.strip_prefix("0X"))?
            .replace('_', "");
        let n = u32::from_str_radix(&hex, 16).ok()?;
        match hex.len() {
            6 => Some(Color::rgb(n)),
            8 => Some(Color(n)),
            _ => None,
        }
    }

    /// With alpha `a`.
    pub const fn with_alpha(self, a: u8) -> Color {
        Color(self.0 & RGB_BITS | (a as u32) << ALPHA_SHIFT)
    }
}

impl From<u32> for Color {
    fn from(v: u32) -> Self {
        Color(v)
    }
}

/// In Yog Data, a colour as written: `#RRGGBB`/`#AARRGGBB` (Yog Data's
/// [`yog_data::Color`]; a `.yres` colour isn't one here).
impl yog_data::Schema for Color {
    fn shape() -> yog_data::schema::Shape {
        yog_data::Color::shape()
    }
    fn read(v: &yog_data::ValueRef<'_>, cx: &mut yog_data::schema::Reader<'_>) -> Option<Self> {
        match yog_data::Color::read(v, cx)? {
            yog_data::Color::Argb(c) => Some(Color(c)),
            yog_data::Color::Res(r) => {
                cx.problem(format!("`{r}`: a colour here is `#RRGGBB` or `#AARRGGBB`"));
                None
            }
        }
    }
    fn write(&self) -> yog_data::Value {
        yog_data::Color::Argb(self.0).write()
    }
}

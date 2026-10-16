//! Colours: `0xAARRGGBB`, straight alpha — what every Yog format and API
//! passes.

/// A colour, `0xAARRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Color(pub u32);

impl Color {
    pub const TRANSPARENT: Color = Color(0);
    pub const WHITE: Color = Color(0xffff_ffff);
    pub const BLACK: Color = Color(0xff00_0000);

    pub const fn argb(a: u8, r: u8, g: u8, b: u8) -> Color {
        Color((a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32)
    }

    /// Opaque `0xRRGGBB`.
    pub const fn rgb(rgb: u32) -> Color {
        Color(0xff00_0000 | rgb)
    }

    pub const fn a(self) -> u8 {
        (self.0 >> 24) as u8
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
    pub const fn to_rgba8(self) -> [u8; 4] {
        [self.r(), self.g(), self.b(), self.a()]
    }

    pub const fn from_rgba8(p: [u8; 4]) -> Color {
        Color::argb(p[3], p[0], p[1], p[2])
    }

    /// `[r, g, b, a]` in `0..=1`.
    pub fn to_f32(self) -> [f32; 4] {
        [self.r(), self.g(), self.b(), self.a()].map(|v| v as f32 / 255.0)
    }

    pub fn from_f32(c: [f32; 4]) -> Color {
        let [r, g, b, a] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
        Color::argb(a, r, g, b)
    }

    /// Each channel multiplied by `o`'s (a tint).
    pub fn multiply(self, o: Color) -> Color {
        let (a, b) = (self.to_f32(), o.to_f32());
        Color::from_f32([a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]])
    }

    /// With alpha `a`.
    pub const fn with_alpha(self, a: u8) -> Color {
        Color(self.0 & 0x00ff_ffff | (a as u32) << 24)
    }
}

impl From<u32> for Color {
    fn from(v: u32) -> Self {
        Color(v)
    }
}

/// In Yog Data, a colour value: `#RRGGBB`/`#AARRGGBB` in text.
impl yog_data::Schema for Color {
    fn shape() -> yog_data::schema::Shape {
        yog_data::schema::Shape::Color
    }
    fn read(v: &yog_data::ValueRef<'_>, cx: &mut yog_data::schema::Reader) -> Option<Self> {
        match v {
            yog_data::ValueRef::Color(c) => Some(Color(*c)),
            yog_data::ValueRef::Str(s) if s.starts_with('#') => yog_data::typed::parse_color(s).map(Color).or_else(|| {
                cx.problem(format!("`{s}` isn't a color (#RRGGBB or #AARRGGBB)"));
                None
            }),
            other => {
                cx.expected("a color (#RRGGBB or #AARRGGBB)", other);
                None
            }
        }
    }
    fn write(&self) -> yog_data::Value {
        yog_data::Value::Color(self.0)
    }
}

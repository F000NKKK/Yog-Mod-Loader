//! Visual theme for book rendering.

use yog_gfx::Color;

#[derive(Debug, Clone)]
pub struct BookTheme {
    /// Outer book background (dark border frame).
    pub bg: Color,
    /// Left page background (light parchment).
    pub page_bg: Color,
    /// Right page background (slightly warmer parchment).
    pub page_bg_right: Color,
    /// Default body text.
    pub text: Color,
    /// Entry/category titles.
    pub title: Color,
    /// Unselected navigation text.
    pub nav: Color,
    /// Selected navigation item text.
    pub nav_selected: Color,
    /// Selected navigation item background.
    pub nav_selected_bg: Color,
    /// Border/separator line color.
    pub border: Color,
    /// Book nameplate (title) color.
    pub nameplate: Color,
    /// Divider line between sections.
    pub divider: Color,
}

/// The book's colours by default — Patchouli's where it has them
/// (`textColor`, `headerColor`, `nameplateColor`).
const FRAME: Color = Color(0xFF_2A1A08);
const PARCHMENT: Color = Color(0xFF_F5E6C8);
const PARCHMENT_GOLDEN: Color = Color(0xFF_EECF9A);
const TEXT: Color = Color(0xFF_000000);
const HEADER: Color = Color(0xFF_333333);
const NAV: Color = Color(0xFF_5C4020);
const AMBER: Color = Color(0xFF_C87820);
const AMBER_TINT: Color = Color(0x50_C87820);
const SPINE: Color = Color(0xFF_5C3A10);
const NAMEPLATE: Color = Color(0xFF_FFDD98);

impl Default for BookTheme {
    fn default() -> Self {
        Self {
            bg: FRAME,
            page_bg: PARCHMENT,
            page_bg_right: PARCHMENT_GOLDEN,
            text: TEXT,
            title: HEADER,
            nav: NAV,
            nav_selected: AMBER,
            nav_selected_bg: AMBER_TINT,
            border: SPINE,
            nameplate: NAMEPLATE,
            divider: HEADER,
        }
    }
}

/// What a hex colour starts with.
const HASH: char = '#';

impl BookTheme {
    /// Override nameplate color from the book's hex string (e.g. "0066cc").
    /// Like Patchouli, this only affects the nameplate text — headers keep
    /// their own headerColor.
    pub fn with_nameplate(mut self, hex: &str) -> Self {
        if let Some(c) = Color::parse(&format!("{HASH}{}", hex.trim_start_matches(HASH))) {
            self.nameplate = c.with_alpha(u8::MAX);
        }
        self
    }
}

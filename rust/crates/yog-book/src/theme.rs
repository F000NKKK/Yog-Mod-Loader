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

impl Default for BookTheme {
    fn default() -> Self {
        Self {
            // Patchouli defaults: textColor=000000, headerColor=333333,
            // nameplateColor=FFDD98.
            bg: Color(0xFF_2A1A08),              // dark brown outer frame
            page_bg: Color(0xFF_F5E6C8),         // left page: warm cream parchment
            page_bg_right: Color(0xFF_EECF9A),   // right page: slightly golden parchment
            text: Color(0xFF_000000),            // body text (Patchouli textColor)
            title: Color(0xFF_333333),           // headers (Patchouli headerColor)
            nav: Color(0xFF_5C4020),             // nav links (unselected)
            nav_selected: Color(0xFF_C87820),    // nav links (selected, amber)
            nav_selected_bg: Color(0x50_C87820), // selected row tint (semi-transparent amber)
            border: Color(0xFF_5C3A10),          // spine + divider lines
            nameplate: Color(0xFF_FFDD98),       // book title color (Patchouli nameplateColor)
            divider: Color(0xFF_333333),         // section headers ("Categories" etc.)
        }
    }
}

impl BookTheme {
    /// Override nameplate color from the book's hex string (e.g. "0066cc").
    /// Like Patchouli, this only affects the nameplate text — headers keep
    /// their own headerColor.
    pub fn with_nameplate(mut self, hex: &str) -> Self {
        if let Some(c) = Color::parse(&format!("#{}", hex.trim_start_matches('#'))) {
            self.nameplate = c.with_alpha(0xff);
        }
        self
    }
}

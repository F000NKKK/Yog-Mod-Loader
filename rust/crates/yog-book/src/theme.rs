//! Visual theme for book rendering.

/// All colors are 0xAARRGGBB.
#[derive(Debug, Clone)]
pub struct BookTheme {
    /// Outer book background (dark parchment).
    pub bg:            u32,
    /// Inner page area background.
    pub page_bg:       u32,
    /// Left panel (TOC) background.
    pub sidebar_bg:    u32,
    /// Default body text.
    pub text:          u32,
    /// Entry/category titles and selected items.
    pub title:         u32,
    /// Unselected navigation button text.
    pub nav:           u32,
    /// Selected navigation button highlight.
    pub nav_selected:  u32,
    /// Border/separator color.
    pub border:        u32,
    /// Book nameplate color.
    pub nameplate:     u32,
    /// Divider line between sections.
    pub divider:       u32,
}

impl Default for BookTheme {
    fn default() -> Self {
        Self {
            bg:           0xFF_1C1008,
            page_bg:      0xFF_F5E6C8,
            sidebar_bg:   0xFF_2A1A08,
            text:         0xFF_3B2008,
            title:        0xFF_7A3A00,
            nav:          0xFF_5C4020,
            nav_selected: 0xFF_C87820,
            border:       0xFF_5C3A10,
            nameplate:    0xFF_C8A050,
            divider:      0xFF_8B6030,
        }
    }
}

impl BookTheme {
    /// Override with colors from the book's `nameplate_color` hex string.
    pub fn with_nameplate(mut self, hex: &str) -> Self {
        if let Ok(v) = u32::from_str_radix(hex.trim_start_matches('#'), 16) {
            self.nameplate = 0xFF_000000 | v;
            self.title = 0xFF_000000 | v;
        }
        self
    }
}

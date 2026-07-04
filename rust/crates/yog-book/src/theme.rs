//! Visual theme for book rendering.

/// All colors are 0xAARRGGBB.
#[derive(Debug, Clone)]
pub struct BookTheme {
    /// Outer book background (dark border frame).
    pub bg:               u32,
    /// Left page background (light parchment).
    pub page_bg:          u32,
    /// Right page background (slightly warmer parchment).
    pub page_bg_right:    u32,
    /// Default body text.
    pub text:             u32,
    /// Entry/category titles.
    pub title:            u32,
    /// Unselected navigation text.
    pub nav:              u32,
    /// Selected navigation item text.
    pub nav_selected:     u32,
    /// Selected navigation item background.
    pub nav_selected_bg:  u32,
    /// Border/separator line color.
    pub border:           u32,
    /// Book nameplate (title) color.
    pub nameplate:        u32,
    /// Divider line between sections.
    pub divider:          u32,
    // kept for compatibility — no longer used for sidebar
    #[allow(dead_code)]
    pub sidebar_bg:       u32,
}

impl Default for BookTheme {
    fn default() -> Self {
        Self {
            // Patchouli defaults: textColor=000000, headerColor=333333,
            // nameplateColor=FFDD98.
            bg:              0xFF_2A1A08,   // dark brown outer frame
            page_bg:         0xFF_F5E6C8,   // left page: warm cream parchment
            page_bg_right:   0xFF_EECF9A,   // right page: slightly golden parchment
            text:            0xFF_000000,   // body text (Patchouli textColor)
            title:           0xFF_333333,   // headers (Patchouli headerColor)
            nav:             0xFF_5C4020,   // nav links (unselected)
            nav_selected:    0xFF_C87820,   // nav links (selected, amber)
            nav_selected_bg: 0x50_C87820,   // selected row tint (semi-transparent amber)
            border:          0xFF_5C3A10,   // spine + divider lines
            nameplate:       0xFF_FFDD98,   // book title color (Patchouli nameplateColor)
            divider:         0xFF_333333,   // section headers ("Categories" etc.)
            sidebar_bg:      0xFF_2A1A08,   // unused, kept for API compat
        }
    }
}

impl BookTheme {
    /// Override nameplate color from the book's hex string (e.g. "0066cc").
    /// Like Patchouli, this only affects the nameplate text — headers keep
    /// their own headerColor.
    pub fn with_nameplate(mut self, hex: &str) -> Self {
        if let Ok(v) = u32::from_str_radix(hex.trim_start_matches('#'), 16) {
            self.nameplate = 0xFF_000000 | v;
        }
        self
    }
}

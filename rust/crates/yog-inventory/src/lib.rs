//! yog-inventory — real Minecraft Container/Menu screens for Yog mods.
//!
//! Unlike `yog-ui` (a HUD-drawn overlay screen with no real item slots), this
//! crate describes actual vanilla-style inventory screens: a block's own
//! slots plus, optionally, the player's own inventory grid underneath —
//! backed by a real Minecraft `BlockEntity`/`AbstractContainerMenu` on the
//! Java side, with real drag-and-drop and network sync for free.
//!
//! See `DESIGN.md` for the full architecture and phased plan. This crate
//! currently covers the data model (phase 2); the Java-side `BlockEntity`/
//! `Menu`/`Screen` plumbing lands in later phases.

use serde::Deserialize;

/// Pixel position of a single slot in vanilla GUI coordinate space (the same
/// space vanilla screens use: origin at the panel's top-left corner, one
/// vanilla slot = 18×18px).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct SlotLayout {
    pub x: f32,
    pub y: f32,
}

/// Vanilla spacing between adjacent slots in the default grid.
pub const SLOT_SIZE: f32 = 18.0;

/// Default position of the player's main inventory (3×9) relative to the
/// panel's top-left corner — matches vanilla furnace/chest-style screens.
pub const DEFAULT_PLAYER_INV_OFFSET: (f32, f32) = (8.0, 84.0);
/// Extra vertical gap between the player's main inventory and their hotbar,
/// on top of the normal per-row `SLOT_SIZE` spacing (also a vanilla convention).
pub const PLAYER_INV_TO_HOTBAR_GAP: f32 = 4.0;

/// Describes one inventory-backed screen: how many slots a mod block has,
/// where they sit, whether the player's own inventory is appended below,
/// and which background texture to draw (vanilla default, or a mod's own).
#[derive(Debug, Clone)]
pub struct InventoryDef {
    pub id: String,
    pub slot_count: usize,
    /// Per-slot pixel positions. Empty = auto-generate a default grid (see
    /// [`InventoryDef::default_grid`]).
    pub layout: Vec<SlotLayout>,
    /// Appends the player's main inventory (3×9) + hotbar (9) below the
    /// custom slots — no armor, no offhand. Native vanilla slot rendering.
    pub include_player_inventory: bool,
    pub player_inv_offset: (f32, f32),
    /// `None` = default vanilla-style panel texture.
    pub background_texture: Option<String>,
    pub title: String,
}

impl InventoryDef {
    pub fn new(id: impl Into<String>, slot_count: usize) -> Self {
        Self {
            id: id.into(),
            slot_count,
            layout: Vec::new(),
            include_player_inventory: true,
            player_inv_offset: DEFAULT_PLAYER_INV_OFFSET,
            background_texture: None,
            title: String::new(),
        }
    }

    /// Explicit per-slot pixel positions, overriding the default grid.
    pub fn layout(mut self, layout: Vec<SlotLayout>) -> Self {
        self.layout = layout;
        self
    }

    /// Remap the slot grid from a JSON array of `{"x":.., "y":..}` objects —
    /// one entry per slot, in slot-index order. Lets users/mod-packs override
    /// the layout without recompiling. Invalid or short JSON is ignored
    /// (falls back to whatever layout was set before, or the default grid).
    pub fn layout_from_json(mut self, json: &str) -> Self {
        if let Ok(parsed) = serde_json::from_str::<Vec<SlotLayout>>(json) {
            self.layout = parsed;
        }
        self
    }

    /// Whether the player's main inventory + hotbar (no armor/offhand) is
    /// appended below the custom slots. Default: `true`.
    pub fn include_player_inventory(mut self, v: bool) -> Self {
        self.include_player_inventory = v;
        self
    }

    pub fn player_inv_offset(mut self, x: f32, y: f32) -> Self {
        self.player_inv_offset = (x, y);
        self
    }

    /// Custom background texture (resource path); `None` keeps the default
    /// vanilla-style panel.
    pub fn background_texture(mut self, path: impl Into<String>) -> Self {
        self.background_texture = Some(path.into());
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Resolve the effective slot layout: the explicit one if set, otherwise
    /// a default vanilla-style grid (9 columns, wrapping downward, 18px
    /// spacing, starting at (8, 18) — same convention as vanilla containers).
    pub fn resolved_layout(&self) -> Vec<SlotLayout> {
        if !self.layout.is_empty() {
            return self.layout.clone();
        }
        Self::default_grid(self.slot_count)
    }

    /// Default vanilla-style grid: up to 9 columns, 18px spacing, starting at
    /// (8, 18) — leaves room for a title above, matches vanilla containers.
    pub fn default_grid(slot_count: usize) -> Vec<SlotLayout> {
        const COLS: usize = 9;
        (0..slot_count)
            .map(|i| {
                let col = (i % COLS) as f32;
                let row = (i / COLS) as f32;
                SlotLayout {
                    x: 8.0 + col * SLOT_SIZE,
                    y: 18.0 + row * SLOT_SIZE,
                }
            })
            .collect()
    }
}

/// Encode an [`InventoryDef`]'s slot layout as `"x:y,x:y,..."` — the
/// wire format used when handing the layout to the Java host (mirrors how
/// `BlockDef::connect_groups` is comma-joined for its ABI struct).
pub fn encode_layout(layout: &[SlotLayout]) -> String {
    layout
        .iter()
        .map(|s| format!("{}:{}", s.x, s.y))
        .collect::<Vec<_>>()
        .join(",")
}

/// Inverse of [`encode_layout`]. Malformed entries are skipped.
pub fn decode_layout(s: &str) -> Vec<SlotLayout> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split(',')
        .filter_map(|pair| {
            let (x, y) = pair.split_once(':')?;
            Some(SlotLayout {
                x: x.parse().ok()?,
                y: y.parse().ok()?,
            })
        })
        .collect()
}

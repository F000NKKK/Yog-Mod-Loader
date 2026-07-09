//! yog-inventory — real Minecraft Container/Menu screens for Yog mods.
//!
//! Unlike `yog-ui` (a HUD-drawn overlay screen with no real item slots), this
//! crate describes actual vanilla-style inventory screens: a block's own
//! slots plus, optionally, the player's own inventory grid underneath —
//! backed by a real Minecraft `BlockEntity`/`AbstractContainerMenu` on the
//! Java side, with real drag-and-drop and network sync for free.
//!
//! Stub crate — see `DESIGN.md` for the full architecture and phased plan.
//! Data model only for now; nothing is wired to the runtime/registry yet.

/// Pixel position of a single slot in vanilla GUI coordinate space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlotLayout {
    pub x: f32,
    pub y: f32,
}

/// Describes one inventory-backed screen: how many slots a mod block has,
/// where they sit, whether the player's own inventory is appended below,
/// and which background texture to draw (vanilla default, or a mod's own).
#[derive(Debug, Clone)]
pub struct InventoryDef {
    pub id: String,
    pub slot_count: usize,
    pub layout: Vec<SlotLayout>,
    pub include_player_inventory: bool,
    pub player_inv_offset: (f32, f32),
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
            player_inv_offset: (8.0, 84.0),
            background_texture: None,
            title: String::new(),
        }
    }

    /// Explicit per-slot pixel positions, overriding the default grid.
    pub fn layout(mut self, layout: Vec<SlotLayout>) -> Self {
        self.layout = layout;
        self
    }

    /// Whether the player's main inventory + hotbar (no armor/offhand) is
    /// appended below the custom slots.
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
}

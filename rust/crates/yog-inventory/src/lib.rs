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
//!
//! What the open screen's slots hold right now is the bus's
//! `client::ScreenSlots` service (`yog_api::service::call`).

/// Pixel position of a single slot in vanilla GUI coordinate space (the same
/// space vanilla screens use: origin at the panel's top-left corner, one
/// vanilla slot = 18×18px).
#[derive(Debug, Clone, PartialEq, yog_data::Schema)]
pub struct SlotLayout {
    pub x: f32,
    pub y: f32,
    /// What the slot takes: item ids (`minecraft:redstone`) and item tags
    /// (`#c:dusts`). Empty = anything.
    #[data(default)]
    pub accepts: Vec<String>,
}

impl SlotLayout {
    /// A slot at `(x, y)` taking anything.
    pub fn at(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            accepts: Vec::new(),
        }
    }
}

/// Vanilla spacing between adjacent slots in the default grid.
pub const SLOT_SIZE: f32 = 18.0;

/// Default position of the player's main inventory (3×9) relative to the
/// panel's top-left corner — matches vanilla furnace/chest-style screens.
pub const DEFAULT_PLAYER_INV_OFFSET: (f32, f32) = (8.0, 84.0);
/// Extra vertical gap between the player's main inventory and their hotbar,
/// on top of the normal per-row `SLOT_SIZE` spacing (also a vanilla convention).
pub const PLAYER_INV_TO_HOTBAR_GAP: f32 = 4.0;
/// Columns of [`InventoryDef::default_grid`] (a vanilla container row).
const DEFAULT_GRID_COLS: usize = 9;
/// Top-left slot of [`InventoryDef::default_grid`] — leaves room for the
/// title above, like vanilla containers.
const DEFAULT_GRID_ORIGIN: (f32, f32) = (8.0, 18.0);

/// Describes one inventory-backed screen: how many slots a mod block has,
/// where they sit, whether the player's own inventory is appended below,
/// and which background texture to draw (vanilla default, or a mod's own).
#[derive(Debug, Clone, yog_data::Schema)]
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
    /// Screen size to center the container on — `None` uses the vanilla
    /// default (176×166). Set this when a custom `on_ui_render` overlay
    /// draws decoration at a different size, so the real vanilla `Slot`s
    /// (positioned by `layout`/`player_inv_offset` relative to the
    /// container's top-left corner) land under that decoration instead of
    /// the two disagreeing about where the top-left corner even is.
    pub background_size: Option<(f32, f32)>,
    /// A machine's part of it (`.yui` `<RecipeArea kind="…">`): its slots
    /// make recipes of a kind, like a crafting table.
    pub machine: Option<Machine>,
}

/// A machine in an inventory: it makes recipes of `kind` from what's in
/// its `places` — each a slot of the inventory holding one ingredient of
/// one of the kind's slots — into its `result` slot.
#[derive(Debug, Clone, PartialEq, yog_data::Schema)]
pub struct Machine {
    pub kind: String,
    pub places: Vec<MachinePlace>,
    pub result: usize,
}

/// One ingredient's place: the `index`-th of the kind's slot `slot`, in the
/// inventory's slot `inv`.
#[derive(Debug, Clone, PartialEq, yog_data::Schema)]
pub struct MachinePlace {
    pub slot: String,
    pub index: usize,
    pub inv: usize,
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
            background_size: None,
            machine: None,
        }
    }

    /// Makes part of it a machine (see [`Machine`]).
    pub fn machine(mut self, machine: Machine) -> Self {
        self.machine = Some(machine);
        self
    }

    /// Explicit per-slot pixel positions, overriding the default grid.
    pub fn layout(mut self, layout: Vec<SlotLayout>) -> Self {
        self.layout = layout;
        self
    }

    /// Remap the slot grid from Yog Data text (JSON too): a list of
    /// `{x, y}` — one entry per slot, in slot-index order. Lets users/mod-packs
    /// override the layout without recompiling. Text that doesn't read is
    /// ignored (falls back to whatever layout was set before, or the default grid).
    pub fn layout_from_text(mut self, text: &str) -> Self {
        let parsed = yog_data::text::parse_value(text).ok();
        if let Some(layout) = parsed.and_then(|v| yog_data::read_value(&v).ok()) {
            self.layout = layout;
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

    /// Size (in GUI pixels) a custom `on_ui_render` overlay draws its
    /// decoration at — used to center the real vanilla container on the
    /// same footprint so its `Slot`s land under that decoration. Leave
    /// unset to keep the vanilla default 176×166 centering.
    pub fn background_size(mut self, w: f32, h: f32) -> Self {
        self.background_size = Some((w, h));
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Resolve the effective slot layout: the explicit one if set, otherwise
    /// [`InventoryDef::default_grid`].
    pub fn resolved_layout(&self) -> Vec<SlotLayout> {
        if !self.layout.is_empty() {
            return self.layout.clone();
        }
        Self::default_grid(self.slot_count)
    }

    /// Default vanilla-style grid: up to 9 columns, 18px spacing, starting at
    /// (8, 18) — leaves room for a title above, matches vanilla containers.
    pub fn default_grid(slot_count: usize) -> Vec<SlotLayout> {
        let (x0, y0) = DEFAULT_GRID_ORIGIN;
        (0..slot_count)
            .map(|i| {
                let col = (i % DEFAULT_GRID_COLS) as f32;
                let row = (i / DEFAULT_GRID_COLS) as f32;
                SlotLayout::at(x0 + col * SLOT_SIZE, y0 + row * SLOT_SIZE)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_grid_wraps_rows() {
        let g = InventoryDef::new("t", DEFAULT_GRID_COLS + 1).resolved_layout();
        assert_eq!(g[0], SlotLayout::at(8.0, 18.0));
        assert_eq!(g[DEFAULT_GRID_COLS], SlotLayout::at(8.0, 36.0));
    }

    #[test]
    fn layout_reads_from_text() {
        let def = InventoryDef::new("t", 2)
            .layout_from_text(r##"[{"x": 1.5, "y": 2}, {x: 30, y: 4, accepts: ["#c:dusts"]}]"##);
        assert_eq!(def.layout[0], SlotLayout::at(1.5, 2.0));
        assert_eq!(def.layout[1].accepts, ["#c:dusts"]);
        let kept = def.clone().layout_from_text("not a layout");
        assert_eq!(kept.layout, def.layout);
    }
}

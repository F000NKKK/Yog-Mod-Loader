//! The words content files are written in: directions, display slots,
//! render layers, model templates.

use yog_ids::texture_slot as slot;
use yog_ids::word::GUI;

/// A block's faces, in the game's order.
pub(crate) const DIRECTIONS: &[&str] = yog_ids::direction::ALL_VALUES;
/// Where an item's model is shown: its `display` transforms.
pub(crate) const DISPLAY_SLOTS: [&str; 8] = [
    "thirdperson_righthand",
    "thirdperson_lefthand",
    "firstperson_righthand",
    "firstperson_lefthand",
    GUI,
    "head",
    "ground",
    "fixed",
];
/// How a block's model is drawn.
pub const RENDER_LAYERS: [&str; 4] = ["solid", "cutout", "cutout_mipped", "translucent"];

/// A model template — `"parent": "cube_all"` — and the texture slots it takes.
pub(crate) struct Template {
    pub(crate) name: &'static str,
    /// The vanilla model it compiles to.
    pub(crate) parent: &'static str,
    /// Its texture slots; for item sprites, `layer0` (more layers allowed).
    pub(crate) slots: &'static [&'static str],
    /// An item sprite template (`generated`, `handheld`) — item models only.
    pub(crate) sprite: bool,
    /// The parent model defines `particle` itself.
    pub(crate) has_particle: bool,
}

/// A template named as the game's model it compiles to, `<dir><name>`.
macro_rules! template {
    ($dir:ident, $name:expr, $slots:expr, sprite: $sprite:expr, particle: $particle:expr) => {
        Template {
            name: $name,
            parent: yog_ids::id!(
                yog_ids::ns::MINECRAFT,
                yog_ids::concatcp!(yog_ids::res::$dir, $name)
            ),
            slots: $slots,
            sprite: $sprite,
            has_particle: $particle,
        }
    };
}

/// A sprite item's slots: its first layer.
const SPRITE_SLOTS: &[&str] = &[yog_ids::model::LAYER0];

pub(crate) const TEMPLATES: &[Template] = &[
    template!(BLOCK_DIR, "cube_all", &[slot::ALL], sprite: false, particle: true),
    template!(BLOCK_DIR, "cube_column", &[slot::END, slot::SIDE], sprite: false, particle: true),
    template!(BLOCK_DIR, "cube_bottom_top", &[slot::TOP, slot::BOTTOM, slot::SIDE], sprite: false, particle: true),
    template!(BLOCK_DIR, "cube", &DIRECTIONS, sprite: false, particle: false),
    template!(BLOCK_DIR, "orientable", &[slot::FRONT, slot::SIDE, slot::TOP], sprite: false, particle: true),
    template!(BLOCK_DIR, "cross", &["cross"], sprite: false, particle: true),
    template!(ITEM_DIR, "generated", SPRITE_SLOTS, sprite: true, particle: true),
    template!(ITEM_DIR, "handheld", SPRITE_SLOTS, sprite: true, particle: true),
];

/// The template named `name`.
pub(crate) fn template(name: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.name == name)
}

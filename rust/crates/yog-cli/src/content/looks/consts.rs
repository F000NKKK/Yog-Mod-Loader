//! The fields and layer kinds of the game's equipment asset (1.21.4+).

pub(super) const LAYERS: &str = "layers";
pub(super) const TEXTURE: &str = "texture";
/// Worn on the body: helmet, chestplate, boots.
pub(super) const HUMANOID: &str = "humanoid";
/// Worn on the legs: leggings.
pub(super) const HUMANOID_LEGGINGS: &str =
    yog_ids::concatcp!(HUMANOID, yog_ids::sep::WORD, "leggings");

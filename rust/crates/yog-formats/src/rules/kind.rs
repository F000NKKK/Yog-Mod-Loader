//! A block of one of the game's kinds (stairs, door, …): its models are the
//! kind's, so its model gives only textures — the kind's slots.

use yog_data::{Diagnostic, Path, field};
use yog_ids::model as m;

use super::{error, warning};
use crate::content::{BlockKind, Model};

/// The model of a block of kind `k`, at `at`.
pub(crate) fn kind_model_rules(k: &BlockKind, model: &Model, at: Path, out: &mut Vec<Diagnostic>) {
    if model.parent.is_some()
        || model.elements.is_some()
        || model.display.is_some()
        || model.ambient_occlusion.is_some()
        || model.gui_light.is_some()
    {
        out.push(error(
            at.clone(),
            format!(
                "kind {} has the game's models — give only textures ({})",
                k.name,
                k.slots.join(", ")
            ),
        ));
    }
    for (slot, _) in &model.textures.0 {
        if slot != m::PARTICLE && !k.slots.contains(slot) {
            out.push(warning(
                at.key(field!(Model, textures)).key(slot),
                format!(
                    "kind {} has no slot `{slot}` ({})",
                    k.name,
                    k.slots.join(", ")
                ),
            ));
        }
    }
    // A material gives textures this file can't see.
    let own = model.material.is_none();
    for slot in k
        .slots
        .iter()
        .filter(|s| own && !model.textures.0.iter().any(|(t, _)| t == *s))
    {
        out.push(error(
            at.clone(),
            format!("kind {} needs textures.{slot}", k.name),
        ));
    }
}

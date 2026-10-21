//! `.ymaterial` files: a surface's textures and render layer, which a
//! model takes by `material: "@material:<mod>:<path>"` instead of listing
//! them — the build puts the material's textures under the model's own.

use std::path::Path;

use yog_formats::content::{Model, RenderLayer};
use yog_formats::material::MaterialFile;
use yog_ids::Format;

use crate::yres::RESOURCES_DIR;

/// Material `id` of the mod at `root` (`ns` its id): its `.ymaterial` read.
pub(crate) fn load(root: &Path, ns: &str, id: &str) -> Result<MaterialFile, String> {
    let (owner, path) = yog_ids::split(id);
    if owner != ns {
        return Err(format!(
            "material `{id}`: only this mod's materials can be used ({ns}:…)"
        ));
    }
    let file = root.join(RESOURCES_DIR).join(Format::Material.file(path));
    let text = crate::files::read_source(&file).map_err(|_| {
        format!(
            "material `{id}`: no {RESOURCES_DIR}/{}",
            Format::Material.file(path)
        )
    })?;
    yog_data::from_text::<MaterialFile>(&text)
        .map(|r| r.value)
        .map_err(|e| format!("material `{id}`: {e}"))
}

/// `model` with its material's textures under its own; the material's render layer, if any.
pub(crate) fn apply(model: &mut Model, material: &MaterialFile) -> Option<RenderLayer> {
    let own = std::mem::take(&mut model.textures.0);
    let mut textures = material.textures.0.clone();
    for (slot, texture) in own {
        match textures.iter_mut().find(|(s, _)| *s == slot) {
            Some(entry) => entry.1 = texture,
            None => textures.push((slot, texture)),
        }
    }
    model.textures.0 = textures;
    material.render_layer
}

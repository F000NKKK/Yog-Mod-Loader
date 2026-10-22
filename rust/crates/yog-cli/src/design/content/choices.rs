//! What the editors' pickers offer: the mod's texts, sound groups,
//! materials, tags and textures.

use std::borrow::Cow;
use std::path::Path;
use yog_data::Value;
use yog_ids::Format;

use crate::design::assets::Resources;
use crate::design::project::mod_root;

#[derive(yog_data::Schema)]
pub(crate) struct TextureChoice {
    /// What goes in the content file.
    pub(crate) value: String,
    /// "Resources", "Resource sets", "Mod assets", "Minecraft blocks", "Minecraft items".
    pub(crate) group: Cow<'static, str>,
}

/// The `@<Set>.<name>` texts of the mod's `.yres` files — the name/tooltip picker's choices.
pub(crate) fn strings(file: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Some(root) = mod_root(file) else {
        return out;
    };
    for p in crate::files::of(&root.join(yog_ids::project::RESOURCES), Format::Res) {
        let Some(set) = p.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if set.contains('.') {
            continue; // a culture variant
        }
        let Ok(text) = crate::files::read_source(&p) else {
            continue;
        };
        let Some(doc) = crate::design::data::read_object(&text) else {
            continue;
        };
        for (name, e) in doc.as_map().into_iter().flatten() {
            let ty = e.get("type").and_then(Value::as_str);
            if e.as_str().is_some() || ty.is_none_or(|t| t == "string") {
                out.push(format!("@{set}.{name}"));
            }
        }
    }
    out.sort();
    out
}

/// What a `.ydef` reference offers: the game's built-ins by short name,
/// then the mod's own definitions of `kind` as `@<kind>:` refs.
fn definitions(
    file: &Path,
    kind: &str,
    built_ins: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let mut out: Vec<String> = built_ins
        .into_iter()
        .map(|id| {
            id.trim_start_matches(&format!(
                "{}{}",
                yog_ids::ns::MINECRAFT,
                yog_ids::sep::NAMESPACE
            ))
            .to_string()
        })
        .collect();
    let Some(root) = mod_root(file) else {
        return out;
    };
    let ns = crate::design::project::mod_id(&root).unwrap_or_else(|| yog_ids::ns::MINECRAFT.into());
    let mut own = Vec::new();
    for p in crate::files::of(&root.join(yog_ids::project::CONTENT), Format::Def) {
        let Some(doc) = crate::files::read_source(&p)
            .ok()
            .and_then(|t| crate::design::data::read_object(&t))
        else {
            continue;
        };
        if doc
            .get(yog_ids::content_field::KIND)
            .and_then(Value::as_str)
            != Some(kind)
        {
            continue;
        }
        if let Some(id) = doc.get(yog_ids::content_field::ID).and_then(Value::as_str) {
            own.push(format!(
                "{}{kind}{}{}",
                yog_ids::sep::TYPED,
                yog_ids::sep::NAMESPACE,
                yog_ids::qualify_in(&ns, id)
            ));
        }
    }
    own.sort();
    out.extend(own);
    out
}

/// What a block's `sound` offers: the game's sound groups, then the mod's.
pub(crate) fn sound_groups(file: &Path) -> Vec<String> {
    definitions(
        file,
        "sound_group",
        yog_formats::def::SoundGroup::built_ins()
            .iter()
            .map(|g| g.id.clone()),
    )
}

/// What a tool's `material` offers: the game's, then the mod's.
pub(crate) fn tool_materials(file: &Path) -> Vec<String> {
    definitions(
        file,
        "tool_material",
        yog_formats::def::ToolMaterial::built_ins()
            .iter()
            .map(|m| m.id.clone()),
    )
}

/// What an armor piece's `material` offers: the game's, then the mod's.
pub(crate) fn armor_materials(file: &Path) -> Vec<String> {
    definitions(
        file,
        "armor_material",
        yog_formats::def::ArmorMaterial::built_ins()
            .iter()
            .map(|m| m.id.clone()),
    )
}

/// What a `tags` list of content in `registry` (`item`, `block`) offers:
/// the mod's own tags by bare name (as the build last listed them), then
/// Yog's common tags.
pub(crate) fn tags(file: &Path, registry: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(root) = mod_root(file) {
        let ns = crate::design::project::mod_id(&root).unwrap_or_default();
        let listed = crate::content::read_tags(&root)
            .remove(registry)
            .unwrap_or_default();
        out.extend(
            listed
                .into_keys()
                .filter_map(|t| t.strip_prefix(&format!("{ns}:")).map(str::to_string)),
        );
    }
    out.extend(
        yog_formats::tags::yog_tags(registry)
            .into_iter()
            .map(|(p, _)| format!("yog:{p}")),
    );
    out
}

/// Everything the texture picker offers.
pub(crate) fn textures(file: &Path) -> Vec<TextureChoice> {
    let mut out = Vec::new();
    let root = mod_root(file);
    if let Some(root) = &root {
        out.extend(
            crate::media::textures(root)
                .into_iter()
                .map(|t| TextureChoice {
                    value: format!("{}{}", yog_ids::reference::TEXTURE, t.id),
                    group: Cow::Borrowed("Mod textures"),
                }),
        );
    }
    if let Some(v) = Resources::for_mod(root.as_deref()).vanilla {
        let (items, blocks) = v.texture_names();
        out.extend(blocks.into_iter().map(|b| TextureChoice {
            value: format!(
                "{}{}",
                yog_ids::reference::TEXTURE,
                yog_ids::join(
                    yog_ids::ns::MINECRAFT,
                    &format!("{}{b}", yog_ids::res::BLOCK_DIR)
                )
            ),
            group: Cow::Borrowed("Minecraft blocks"),
        }));
        out.extend(items.into_iter().map(|i| TextureChoice {
            value: format!(
                "{}{}",
                yog_ids::reference::TEXTURE,
                yog_ids::join(
                    yog_ids::ns::MINECRAFT,
                    &format!("{}{i}", yog_ids::res::ITEM_DIR)
                )
            ),
            group: Cow::Borrowed("Minecraft items"),
        }));
    }
    out
}

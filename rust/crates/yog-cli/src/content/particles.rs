//! `.yparticle` files: the mod's particles — each the game's
//! `assets/<mod id>/particles/<name>.json` (its pictures) and a definition
//! the code registers (how it moves and how it's drawn, which the hosts
//! draw it by).

use std::path::Path;

use serde_json::json;
use yog_ids::Format;
use yog_registry::{Particle, ParticleDef};

use super::Output;
use crate::content::ctx::problem;
use crate::diagnostics::Diagnostic;
use crate::yres::RESOURCES_DIR;

/// The field of `particles/<name>.json` listing its pictures.
const TEXTURES: &str = "textures";

/// The mod's particles: the files they make, and their definitions.
#[derive(Default)]
pub(crate) struct Particles {
    pub(crate) outputs: Vec<Output>,
    pub(crate) definitions: Vec<(&'static str, String, Vec<u8>)>,
}

impl Particles {
    pub(crate) fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

/// Every `.yparticle` under `resources/particle/` of the mod at `root`;
/// `textures`: the mod's own textures (a particle's pictures must be);
/// problems go to `errors`.
pub(crate) fn compile(
    root: &Path,
    mod_id: &str,
    textures: &std::collections::BTreeSet<String>,
    errors: &mut Vec<Diagnostic>,
) -> Particles {
    let resources = root.join(RESOURCES_DIR);
    let mut out = Particles::default();
    for file in crate::files::with_ext(&resources, Format::Particle.ext()) {
        let shown = crate::shown_path(root, &file);
        let rel = file
            .strip_prefix(&resources)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', yog_ids::sep::PATH);
        let Some(id) = yog_formats::particle::id_of(mod_id, &rel) else {
            let dir = yog_formats::particle::DIR;
            errors.push(problem(format!("a particle lives under {RESOURCES_DIR}/{dir}/")).in_file(&shown));
            continue;
        };
        match read(&file, textures) {
            Ok(look) => {
                let (ns, name) = yog_ids::split(&id);
                let path = format!("{}{name}{}", yog_ids::res::PARTICLES, yog_ids::res::JSON);
                let sprites: Vec<String> = look.textures.iter().map(|t| sprite(t)).collect();
                let json = json!({ TEXTURES: sprites });
                out.outputs.push((yog_ids::asset_file(ns, &path), super::pretty(&json)));
                let def = ParticleDef { id: id.clone(), look };
                out.definitions
                    .push((yog_ids::kind::PARTICLE, id, yog_data::binary::encode(&yog_data::Schema::write(&def))));
            }
            Err(e) => errors.push(problem(e).in_file(&shown)),
        }
    }
    out
}

/// Texture `id`'s sprite in the particles' atlas (`m:particle/cloud` → `m:cloud`).
fn sprite(id: &str) -> String {
    let (ns, path) = yog_ids::split(id);
    let atlas = yog_registry::Atlas::of_path(path);
    yog_ids::join(ns, atlas.map_or(path, |a| a.sprite(path)))
}

/// The `.yparticle` at `file`: at least one picture, each a texture the mod has.
fn read(file: &Path, textures: &std::collections::BTreeSet<String>) -> Result<Particle, String> {
    let text = crate::files::read_source(file).map_err(|e| e.to_string())?;
    let look = yog_data::from_text::<Particle>(&text).map_err(|e| e.to_string())?.value;
    if look.textures.is_empty() {
        return Err("a particle shows at least one texture".into());
    }
    if let Some(missing) = look.textures.iter().find(|t| !textures.contains(*t)) {
        return Err(format!("texture `{missing}`: not one of the mod's (a .ytexture under {RESOURCES_DIR}/)"));
    }
    Ok(look)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_particle_is_its_pictures_and_a_definition() {
        let dir = std::env::temp_dir().join(format!("yog-yparticle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let particles = dir.join(RESOURCES_DIR).join(yog_formats::particle::DIR);
        std::fs::create_dir_all(&particles).unwrap();
        std::fs::write(
            particles.join("conjure.yparticle"),
            r#"{ textures: ["m:particle/cloud"], fade: true, render: { blend: "additive", depth_write: false } }"#,
        )
        .unwrap();
        std::fs::write(particles.join("bad.yparticle"), r#"{ textures: ["m:nope"] }"#).unwrap();
        let own = std::collections::BTreeSet::from(["m:particle/cloud".to_owned()]);
        let mut errors = Vec::new();
        let p = compile(&dir, "m", &own, &mut errors);
        assert_eq!(errors.len(), 1, "the unknown texture: {errors:?}");
        let (path, json) = &p.outputs[0];
        assert_eq!(path, "assets/m/particles/conjure.json");
        assert_eq!(serde_json::from_slice::<serde_json::Value>(json).unwrap()[TEXTURES][0], "m:cloud");
        assert_eq!(p.definitions[0].1, "m:conjure");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

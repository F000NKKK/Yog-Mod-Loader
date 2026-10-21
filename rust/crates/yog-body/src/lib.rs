//! Bodies for Yog mods (see `docs/design/yog-body.md`).
//!
//! - [`format`]: `.ymodel` skeletons and `.yanim` clips, as written;
//! - [`model`], [`anim`], [`rig`]: those made ready, posed and layered;
//! - [`wire`]: the frame a host sends about an entity (with the data it
//!   keeps, synced from the server) and the [`Ops`] a mod answers with —
//!   pose vanilla parts, turn the body, draw meshes, disguise, hide, capture;
//! - [`morph`]: one captured body turning into another;
//! - [`camera`], [`movement`]: the local player's view and travel step.
//!
//! The models and clips a mod ships are registered here by the generated
//! code (`yog build` compiles `.ymodel`/`.yanim`); [`model`](fn@model) and
//! [`clip`] find them.

pub mod anim;
pub mod camera;
pub mod format;
pub mod math;
pub mod model;
pub mod morph;
pub mod movement;
pub mod rig;
pub mod wire;

use std::collections::HashMap;
use yog_sync::{Arc, LazyLock, RwLock};

pub use anim::{BonePose, Clip, Pose};
pub use camera::{CameraFrame, CameraOut};
pub use math::{Mat4, Vec3};
pub use model::{Model, Quad};
pub use movement::TravelFrame;
pub use rig::Rig;
pub use wire::{Anchor, BoneOp, Capture, Frame, Mesh, Mode, Ops};

#[derive(Default)]
struct Store {
    models: HashMap<String, Arc<Model>>,
    /// `(anim id, clip name)` → clip.
    clips: HashMap<(String, String), Arc<Clip>>,
}

static STORE: LazyLock<RwLock<Store>> = LazyLock::new(Default::default);

/// Registers model `id` (`ns:name`) from its compiled `.ymodel`.
pub fn register_model(id: &str, data: &[u8]) -> Result<(), String> {
    let file: format::ModelFile = yog_data::read_bytes(data).map_err(|e| format!("model {id}: {e:?}"))?;
    let model = Model::new(&file).map_err(|e| format!("model {id}: {e}"))?;
    STORE.write().models.insert(id.to_owned(), Arc::new(model));
    Ok(())
}

/// Registers the clips of `id` (`ns:name`) from its compiled `.yanim`.
pub fn register_anims(id: &str, data: &[u8]) -> Result<(), String> {
    let file: format::AnimFile = yog_data::read_bytes(data).map_err(|e| format!("animations {id}: {e:?}"))?;
    let clips = anim::Clips::new(&file).map_err(|e| format!("animations {id}: {e}"))?;
    let mut store = STORE.write();
    for c in clips.0 {
        store.clips.insert((id.to_owned(), c.name.clone()), Arc::new(c));
    }
    Ok(())
}

/// Model `id`, if registered.
pub fn model(id: &str) -> Option<Arc<Model>> {
    STORE.read().models.get(id).cloned()
}

/// Clip `name` of animations `id`, if registered.
pub fn clip(id: &str, name: &str) -> Option<Arc<Clip>> {
    STORE.read().clips.get(&(id.to_owned(), name.to_owned())).cloned()
}

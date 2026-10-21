//! Mod entities the loader draws: a type registered with a `model` gets its
//! body model, playing the clip of its state — `walk` while it walks, else
//! `idle`, or the state in its entity data `yog:state` (a string the mod
//! sets on the server) — timed by the world's clock, alike for everyone.
//!
//! A hanging entity (`kind: hanging`) tells the mods how it hangs: its
//! entity data [`HANGING`], read with [`hanging`] / [`hanging_of`].

use std::collections::HashMap;
use yog_sync::atomic::{AtomicBool, Ordering};
use yog_sync::{LazyLock, Mutex};

use yog_body::{Anchor, Frame, Ops, Rig};
use yog_registry::EntityDef;

/// The entity data keys: a mod's entity's state ([`STATE`]) and a hanging
/// entity's [`Hanging`] ([`HANGING`], set by the loader on the server and for
/// every client that sees it).
pub use yog_ids::yog::{HANGING, STATE};

pub use yog_event::entity::Hanging;
use yog_event::Message as _;

/// The hanging entity a body frame is about, if it is one.
pub fn hanging_of(frame: &Frame) -> Option<Hanging> {
    Hanging::from_bytes(frame.data(HANGING)?)
}

/// Hanging entity `uuid` on the server, if it is one.
pub fn hanging(uuid: &str) -> Option<Hanging> {
    Hanging::from_bytes(&crate::call(&yog_event::entity::Data { uuid: uuid.to_owned(), key: HANGING.to_owned() })??.bytes)
}

/// Above this limb swing an entity walks.
const WALKING: f32 = 0.01;

struct Look {
    model: String,
    animations: Option<String>,
    clips: Vec<(String, String)>,
}

static LOOKS: LazyLock<Mutex<HashMap<String, Look>>> = LazyLock::new(Default::default);
static HOOKED: AtomicBool = AtomicBool::new(false);

/// Remembers how `def` looks; `true` for the first such type (the caller
/// hooks [`draw`] then).
pub(crate) fn drawn(def: &EntityDef) -> bool {
    let Some(model) = &def.model else { return false };
    LOOKS.lock().insert(
        def.id.clone(),
        Look {
            model: yog_ids::qualify_in(yog_ids::split(&def.id).0, model),
            animations: def.animations.as_ref().map(|a| yog_ids::qualify_in(yog_ids::split(&def.id).0, a)),
            clips: def.clips.0.clone(),
        },
    );
    crate::body::track_type(&def.id, true);
    !HOOKED.swap(true, Ordering::AcqRel)
}

pub(crate) fn draw(f: &Frame, ops: &mut Ops) {
    let looks = LOOKS.lock();
    let Some(look) = looks.get(&f.entity_type) else { return };
    let Some(model) = yog_body::model(&look.model) else { return };
    let state = f
        .data(STATE)
        .and_then(|s| std::str::from_utf8(s).ok())
        .map(str::to_owned)
        .unwrap_or_else(|| if f.limb_amount > WALKING { "walk" } else { "idle" }.to_owned());
    let mut rig = Rig::new(model, Anchor::Model);
    let clip = look.clips.iter().find(|(s, _)| *s == state).map(|(_, c)| c.as_str()).unwrap_or(state.as_str());
    if let Some(anims) = &look.animations
        && let Some(c) = yog_body::clip(anims, clip)
    {
        rig.play(c, 0.0);
    }
    rig.write(ops, f.world_seconds());
}

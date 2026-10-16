//! Mod entities the loader draws: a type registered with a `model` gets its
//! body model, playing the clip of its state — `walk` while it walks, else
//! `idle`, or the state in its entity data `yog:state` (a string the mod
//! sets on the server) — timed by the world's clock, alike for everyone.
//!
//! A hanging entity (`kind: hanging`) tells the mods how it hangs: its
//! entity data [`HANGING`], read with [`Hanging`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

use yog_body::{Anchor, Frame, Ops, Rig};
use yog_registry::EntityDef;

/// The entity data key a mod's entity's state goes under.
pub const STATE: &str = "yog:state";

/// The entity data key a hanging entity's [`Hanging`] goes under — the
/// loader sets it, on the server and for every client that sees it.
pub const HANGING: &str = "yog:hanging";

/// How a hanging entity hangs: on which side, how big, and the item it was
/// placed from (what it drops).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hanging {
    /// The way it faces, out of its wall: `north`, `south`, `west`, `east`.
    pub facing: String,
    /// Whole blocks of wall, wide and high.
    pub width: u32,
    pub height: u32,
    /// The item it keeps: its id and custom data (SNBT, `{}` for none).
    pub item: String,
    pub nbt: String,
}

impl Hanging {
    /// Out of its entity data ([`HANGING`]): little-endian, `facing` and
    /// `item` a u16 length and UTF-8, `width`, `height` u16, `nbt` a u32
    /// length and UTF-8.
    pub fn decode(mut b: &[u8]) -> Option<Hanging> {
        fn take<'a>(b: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
            let (h, t) = (b.get(..n)?, b.get(n..)?);
            *b = t;
            Some(h)
        }
        fn u16(b: &mut &[u8]) -> Option<u16> {
            take(b, 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
        }
        fn str(b: &mut &[u8], n: usize) -> Option<String> {
            String::from_utf8(take(b, n)?.to_vec()).ok()
        }
        let n = u16(&mut b)? as usize;
        let facing = str(&mut b, n)?;
        let (width, height) = (u16(&mut b)? as u32, u16(&mut b)? as u32);
        let n = u16(&mut b)? as usize;
        let item = str(&mut b, n)?;
        let n = take(&mut b, 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))? as usize;
        let nbt = str(&mut b, n)?;
        Some(Hanging { facing, width, height, item, nbt })
    }

    /// The hanging entity a body frame is about, if it is one.
    pub fn of(frame: &Frame) -> Option<Hanging> {
        Hanging::decode(frame.data(HANGING)?)
    }

    /// Hanging entity `uuid` on the server, if it is one.
    pub fn get(srv: &dyn yog_core::Server, uuid: &str) -> Option<Hanging> {
        Hanging::decode(&srv.entity_data(uuid, HANGING)?)
    }
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

fn qualify(id: &str, of: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        let ns = of.split_once(':').map_or("minecraft", |(n, _)| n);
        format!("{ns}:{id}")
    }
}

/// Remembers how `def` looks; `true` for the first such type (the caller
/// hooks [`draw`] then).
pub(crate) fn drawn(def: &EntityDef) -> bool {
    let Some(model) = &def.model else { return false };
    LOOKS.lock().unwrap_or_else(|e| e.into_inner()).insert(
        def.id.clone(),
        Look {
            model: qualify(model, &def.id),
            animations: def.animations.as_ref().map(|a| qualify(a, &def.id)),
            clips: def.clips.0.clone(),
        },
    );
    crate::body::track_type(&def.id, true);
    !HOOKED.swap(true, Ordering::AcqRel)
}

pub(crate) fn draw(f: &Frame, ops: &mut Ops) {
    let looks = LOOKS.lock().unwrap_or_else(|e| e.into_inner());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hanging_entity_reads_as_the_host_writes_it() {
        let mut b = Vec::new();
        b.extend_from_slice(&5u16.to_le_bytes());
        b.extend_from_slice(b"north");
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&3u16.to_le_bytes());
        b.extend_from_slice(&10u16.to_le_bytes());
        b.extend_from_slice(b"m:painting");
        b.extend_from_slice(&5u32.to_le_bytes());
        b.extend_from_slice(b"{a:1}");
        let h = Hanging::decode(&b).unwrap();
        assert_eq!((h.facing.as_str(), h.width, h.height), ("north", 2, 3));
        assert_eq!((h.item.as_str(), h.nbt.as_str()), ("m:painting", "{a:1}"));
        assert!(Hanging::decode(&b[..b.len() - 1]).is_none(), "cut short: nothing");
    }
}

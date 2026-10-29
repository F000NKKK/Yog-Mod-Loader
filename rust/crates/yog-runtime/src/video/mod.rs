//! Videos (`docs/design/video.md`): the mods' compiled videos, and the
//! players a client opens on them — the `yog:video/…` services. A player's
//! frame is decoded when it's asked for (each frame it's drawn), so only
//! what's seen is decoded.

pub(crate) mod ambient;
pub(crate) mod cutscene;
mod player;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use substrate_lock::Locked;
use substrate_lock::ReadWrite;

use yog_event::video;
use yog_registry::VideoDef;

use crate::bus::serve as provide;
use player::Player;

static VIDEOS: RwLock<Option<HashMap<String, Arc<VideoDef>>>> = RwLock::new(None);

#[derive(Default)]
struct Players {
    open: HashMap<u32, Player>,
    /// The last handle given (0 is none).
    made: u32,
}

static PLAYERS: Mutex<Option<Players>> = Mutex::new(None);

/// Video `id`'s size, pixels.
pub(crate) fn size(id: &str) -> Option<(u32, u32)> {
    let videos = VIDEOS.read_locked();
    let def = videos.as_ref()?.get(id)?;
    Some((def.width, def.height))
}

/// Stores a mod's video.
pub(crate) fn register(def: VideoDef) {
    VIDEOS
        .write_locked()
        .get_or_insert_with(HashMap::new)
        .insert(def.id.clone(), Arc::new(def));
}

fn with_players<R>(f: impl FnOnce(&mut Players) -> R) -> R {
    f(PLAYERS.locked().get_or_insert_with(Players::default))
}

/// Player `handle`, if it's open, given to `f`.
fn with_player<R>(handle: u32, f: impl FnOnce(&mut Player) -> R) -> Option<R> {
    with_players(|p| p.open.get_mut(&handle).map(f))
}

fn open(o: &video::Open) -> Option<u32> {
    let def = VIDEOS.read_locked().as_ref()?.get(&o.id).cloned();
    let Some(def) = def else {
        yog_logging::warn!("no video `{}`", o.id);
        return None;
    };
    with_players(|p| {
        let handle = p.made.checked_add(1)?;
        let texture = format!(
            "{}{handle}{}",
            yog_ids::yog::DYNAMIC_VIDEO_PREFIX,
            yog_ids::res::PNG
        );
        p.open.insert(handle, Player::new(def, texture)?);
        p.made = handle;
        Some(handle)
    })
}

fn frame(f: &video::Frame) -> Option<video::Picture> {
    let mut env = crate::jni::util::attached_env()?;
    with_player(f.handle, |p| p.frame(&mut env)).flatten()
}

fn close(c: &video::Close) -> bool {
    let Some(mut p) = with_players(|p| p.open.remove(&c.handle)) else {
        return false;
    };
    if let Some(mut env) = crate::jni::util::attached_env() {
        p.close(&mut env);
    }
    true
}

/// The `yog:video/…` services; once, at start.
pub(crate) fn provide_services() {
    provide::<video::Open>(open);
    provide::<video::Play>(|p| with_player(p.handle, |v| v.play(p.playing)).is_some());
    provide::<video::Seek>(|s| with_player(s.handle, |v| v.seek(s.seconds)).is_some());
    provide::<video::Close>(close);
    provide::<video::Frame>(frame);
    provide::<video::Cutscene>(cutscene::start);
}

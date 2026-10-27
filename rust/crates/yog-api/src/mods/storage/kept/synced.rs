//! `synced` kept values — the Rust side of Yog Script's `synced` members
//! (`docs/design/storage.md` §4), on the same channel in the same rows
//! ([`SyncedRow`], its kind the mod's id): each tick the server sends what
//! changed to who sees its owner (a player's to that player, the rest to
//! everyone); a joining player gets every value; a client of another's
//! server takes them in, its reads the server's values.

use std::collections::HashMap;

use yog_event::storage::SyncedRow;
use yog_ids::yog::SYNCED;
use yog_sync::Mutex;

use crate::Registry;
use crate::event::client::net::Packet;
use crate::event::player::session::{Join, Leave};
use crate::event::server::game::Tick;

/// Who gets a kept value's rows.
#[derive(Clone, Copy)]
pub(in crate::mods::storage) enum Audience {
    /// Its owner alone (a player's).
    Owner,
    /// Every player.
    All,
}

/// A value to send: its owner, the player it goes to (none: everyone), its key and bytes.
pub(in crate::mods::storage) struct Outgoing {
    pub owner: String,
    pub to: Option<String>,
    pub key: &'static str,
    pub bytes: Vec<u8>,
}

/// The players online, by uuid: their names (what a packet's sent to).
static NAMES: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
/// Values' bytes by their key and owner.
type Values = HashMap<(String, String), Vec<u8>>;

/// Values arrived before their kept value was first used.
static PENDING: Mutex<Option<Values>> = Mutex::new(None);

/// The value arrived for `key` of `owner` before it was used, if one did.
pub(super) fn pending(key: &str, owner: &str) -> Option<Vec<u8>> {
    PENDING
        .lock()
        .as_mut()?
        .remove(&(key.to_owned(), owner.to_owned()))
}

/// Sending and taking synced values — registered by `export_mod!` after
/// the mod's own handlers, so what theirs changed this tick is sent.
pub(in crate::mods::storage) fn listen(registry: &mut Registry) {
    registry.on::<Join, _>(|e, _| joined(&e.uuid, &e.player_name));
    registry.on::<Leave, _>(|e, _| {
        if let Some(names) = NAMES.lock().as_mut() {
            names.remove(&e.uuid);
        }
    });
    registry.on::<Tick, _>(|_, _| super::sync(false, &mut |o| send(&o)));
    registry.on_client_packet(SYNCED, |p: &Packet| received(&p.payload));
}

/// Player `uuid` (named `name`) joined: every value it sees.
fn joined(uuid: &str, name: &str) {
    NAMES
        .lock()
        .get_or_insert_with(HashMap::new)
        .insert(uuid.to_owned(), name.to_owned());
    super::sync(true, &mut |o| {
        if o.to.as_deref().is_none_or(|to| to == uuid) {
            send_to(name, &o);
        }
    });
}

/// `o` sent to who gets it.
fn send(o: &Outgoing) {
    match &o.to {
        Some(uuid) => {
            let name = NAMES.lock().as_ref().and_then(|n| n.get(uuid).cloned());
            if let Some(name) = name {
                send_to(&name, o);
            }
        }
        None => {
            for name in crate::event::player::online() {
                send_to(&name, o);
            }
        }
    }
}

fn send_to(name: &str, o: &Outgoing) {
    let row = SyncedRow {
        kind: crate::mod_id().to_owned(),
        owner: o.owner.clone(),
        key: o.key.to_owned(),
        bytes: o.bytes.clone(),
    };
    crate::net::send_to_player(name, SYNCED, yog_data::to_bytes(&row));
}

/// A row arrived: this mod's taken in by its kept value (or kept until
/// it's used). Where the server runs in this game, its values are the
/// server's own already.
fn received(payload: &[u8]) {
    if super::super::world_open() {
        return;
    }
    let Ok(row) = yog_data::read_bytes::<SyncedRow>(payload) else {
        crate::error!("[{}] a synced value didn't arrive whole", crate::mod_id());
        return;
    };
    arrived(row);
}

/// `row` taken in by its kept value, or kept until that's used.
pub(in crate::mods::storage) fn arrived(row: SyncedRow) {
    if row.kind != crate::mod_id() || super::take(&row.key, &row.owner, &row.bytes) {
        return;
    }
    PENDING
        .lock()
        .get_or_insert_with(HashMap::new)
        .insert((row.key, row.owner), row.bytes);
}

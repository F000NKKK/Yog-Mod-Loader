//! Entity data (ABI 3.2): values mods keep on an entity by key — the
//! loader's `SynchedEntityData`/attachments. The server holds them and sends
//! each change to the players who see the entity (and to it, if a player);
//! a player who starts seeing an entity gets all of it. Changes go out once
//! a tick, batched per entity. Values marked
//! `persist` are saved with the entity. Only the server writes them: a
//! client asks for a change with its mod's packet, the server decides.
//!
//! On the wire (`yog:entity_data`), records one after another:
//! `[op][uuid][key][len u32][bytes]` — `SET`, `REMOVE`, or `CLEAR` (every key
//! of the entity, key and value empty). Strings are a u16 length and UTF-8.

use std::collections::{BTreeMap, HashMap};
use yog_sync::{LazyLock, Mutex};

use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::jbyteArray;
use yog_abi::YogStr;

pub(crate) use yog_ids::yog::{DIMENSIONS, ENTITY_DATA};

const SET: u8 = 1;
const REMOVE: u8 = 2;
const CLEAR: u8 = 3;

struct Value {
    bytes: Vec<u8>,
    persist: bool,
}

type Values = BTreeMap<String, Value>;
/// An entity's changes this tick, by key (`None`: removed).
type PendingChanges = BTreeMap<String, Option<Vec<u8>>>;
/// An entity's values as the client got them, encoded.
type EncodedValues = BTreeMap<String, Vec<u8>>;

static SERVER: LazyLock<Mutex<HashMap<String, Values>>> = LazyLock::new(Default::default);
/// Changes this tick, by entity, then key (`None`: removed) — sent once, at
/// the end of the tick: a key set twice goes once, an entity's changes in
/// one packet.
static PENDING: LazyLock<Mutex<HashMap<String, PendingChanges>>> = LazyLock::new(Default::default);
static CLIENT: LazyLock<Mutex<HashMap<String, EncodedValues>>> = LazyLock::new(Default::default);

// ── Records ──────────────────────────────────────────────────────────────────

fn put_str(b: &mut Vec<u8>, s: &str) {
    let s = &s.as_bytes()[..s.len().min(u16::MAX as usize)];
    b.extend_from_slice(&(s.len() as u16).to_le_bytes());
    b.extend_from_slice(s);
}

fn record(b: &mut Vec<u8>, op: u8, uuid: &str, key: &str, value: &[u8]) {
    b.push(op);
    put_str(b, uuid);
    put_str(b, key);
    b.extend_from_slice(&(value.len() as u32).to_le_bytes());
    b.extend_from_slice(value);
}

struct Record {
    op: u8,
    uuid: String,
    key: String,
    value: Vec<u8>,
}

fn records(mut b: &[u8]) -> Vec<Record> {
    fn take<'a>(b: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
        if b.len() < n {
            return None;
        }
        let (h, t) = b.split_at(n);
        *b = t;
        Some(h)
    }
    fn str(b: &mut &[u8]) -> Option<String> {
        let n = take(b, 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as usize)?;
        String::from_utf8(take(b, n)?.to_vec()).ok()
    }
    let mut out = Vec::new();
    while !b.is_empty() {
        let Some(r) = (|| {
            let op = take(&mut b, 1)?[0];
            let uuid = str(&mut b)?;
            let key = str(&mut b)?;
            let n =
                take(&mut b, 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize)?;
            let value = take(&mut b, n)?.to_vec();
            Some(Record {
                op,
                uuid,
                key,
                value,
            })
        })() else {
            break;
        };
        out.push(r);
    }
    out
}

// ── Server ───────────────────────────────────────────────────────────────────

/// Sends records about `uuid` to the players who see it, and to it.
fn send_to_watchers(uuid: &str, bytes: &[u8]) {
    unsafe {
        let _: Option<bool> = bridge!(
            "sendToTracking",
            [
                S(YogStr::from_str(uuid)),
                S(YogStr::from_str(ENTITY_DATA)),
                Bytes(bytes)
            ]
        );
    }
}

/// Sets (`Some`) or removes a value, and tells the watchers if it changed.
pub(crate) fn set(uuid: &str, key: &str, value: Option<&[u8]>, persist: bool) {
    let changed = {
        let mut store = SERVER.lock();
        match value {
            Some(v) => {
                let values = store.entry(uuid.to_owned()).or_default();
                let same = values
                    .get(key)
                    .is_some_and(|old| old.bytes == v && old.persist == persist);
                values.insert(
                    key.to_owned(),
                    Value {
                        bytes: v.to_vec(),
                        persist,
                    },
                );
                !same
            }
            None => {
                let removed = store
                    .get_mut(uuid)
                    .is_some_and(|values| values.remove(key).is_some());
                if store.get(uuid).is_some_and(|v| v.is_empty()) {
                    store.remove(uuid);
                }
                removed
            }
        }
    };
    if changed {
        PENDING
            .lock()
            .entry(uuid.to_owned())
            .or_default()
            .insert(key.to_owned(), value.map(<[u8]>::to_vec));
    }
}

/// Sends this tick's changes: one packet per entity, to who sees it.
pub(crate) fn flush() {
    let pending = std::mem::take(&mut *PENDING.lock());
    for (uuid, keys) in pending {
        let mut b = Vec::new();
        for (key, value) in keys {
            match value {
                Some(v) => record(&mut b, SET, &uuid, &key, &v),
                None => record(&mut b, REMOVE, &uuid, &key, &[]),
            }
        }
        send_to_watchers(&uuid, &b);
    }
}

pub(crate) fn get(uuid: &str, key: &str) -> Option<Vec<u8>> {
    SERVER.lock().get(uuid)?.get(key).map(|v| v.bytes.clone())
}

/// The entity services the runtime provides (it keeps the data); once, at start.
pub(crate) fn register() {
    use yog_event::entity::{Data, DataValue, SetData, SetDimensions, VANILLA_SIZE};
    crate::bus::serve::<Data>(|d| get(&d.uuid, &d.key).map(|bytes| DataValue { bytes }));
    crate::bus::serve::<SetData>(|d| {
        if d.uuid.is_empty() || d.key.is_empty() {
            return false;
        }
        set(
            &d.uuid,
            &d.key,
            d.value.as_ref().map(|v| v.bytes.as_slice()),
            d.persist,
        );
        true
    });
    crate::bus::serve::<SetDimensions>(|d| {
        let [w, h, eye] = d.size.unwrap_or(VANILLA_SIZE);
        let found: bool = unsafe {
            bridge!(
                "entitySetDimensions",
                [S(YogStr::from_str(&d.uuid)), F(w), F(h), F(eye)]
            )
        }
        .unwrap_or(false);
        if found {
            let value = d.size.map(|[w, h, eye]| encode_dims(w, h, eye));
            set(&d.uuid, DIMENSIONS, value.as_deref(), false);
        }
        found
    });
}

/// `byte[] nativeEntityDataFor(String uuid)`: everything about the entity,
/// for a player who starts seeing it; null for nothing.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeEntityDataFor<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    uuid: JString<'l>,
) -> jbyteArray {
    let uuid = jstr!(env, uuid, std::ptr::null_mut());
    let mut b = Vec::new();
    if let Some(values) = SERVER.lock().get(&uuid) {
        record(&mut b, CLEAR, &uuid, "", &[]);
        for (k, v) in values {
            record(&mut b, SET, &uuid, k, &v.bytes);
        }
    }
    if b.is_empty() {
        return std::ptr::null_mut();
    }
    env.byte_array_from_slice(&b)
        .map(|a| a.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// `byte[] nativeEntityDataSave(String uuid)`: the values kept with the
/// entity when it's saved; null for none.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeEntityDataSave<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    uuid: JString<'l>,
) -> jbyteArray {
    let uuid = jstr!(env, uuid, std::ptr::null_mut());
    let mut b = Vec::new();
    if let Some(values) = SERVER.lock().get(&uuid) {
        for (k, v) in values.iter().filter(|(_, v)| v.persist) {
            record(&mut b, SET, "", k, &v.bytes);
        }
    }
    if b.is_empty() {
        return std::ptr::null_mut();
    }
    env.byte_array_from_slice(&b)
        .map(|a| a.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// `nativeEntityDataLoad(String uuid, byte[] saved)`: the saved values back,
/// as the entity loads.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeEntityDataLoad<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    uuid: JString<'l>,
    saved: JByteArray<'l>,
) {
    let uuid = jstr!(env, uuid);
    let Ok(saved) = env.convert_byte_array(&saved) else {
        return;
    };
    let mut store = SERVER.lock();
    let values = store.entry(uuid).or_default();
    for r in records(&saved) {
        if r.op == SET {
            values.insert(
                r.key,
                Value {
                    bytes: r.value,
                    persist: true,
                },
            );
        }
    }
}

/// `nativeEntityDataSet(String uuid, String key, byte[] value)`: the host's
/// own value about an entity (a hanging entity's `yog:hanging`), sent like a
/// mod's; null removes it. Not saved: the host keeps its entity's state.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeEntityDataSet<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    uuid: JString<'l>,
    key: JString<'l>,
    value: JByteArray<'l>,
) {
    let uuid = jstr!(env, uuid);
    let key = jstr!(env, key);
    let value = if value.is_null() {
        None
    } else {
        env.convert_byte_array(&value).ok()
    };
    if !uuid.is_empty() && !key.is_empty() {
        set(&uuid, &key, value.as_deref(), false);
    }
}

/// `nativeEntityDataForget(String uuid)`: the entity left the server's
/// memory (unloaded, gone); what it keeps was saved with it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeEntityDataForget<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    uuid: JString<'l>,
) {
    let uuid = jstr!(env, uuid);
    SERVER.lock().remove(&uuid);
}

/// The server stops: nothing carries over.
pub(crate) fn server_stopped() {
    SERVER.lock().clear();
    PENDING.lock().clear();
}

// ── Client ───────────────────────────────────────────────────────────────────

/// Records from the server.
pub(crate) fn receive(payload: &[u8]) {
    let mut dims: Vec<(String, Option<[f32; 3]>)> = Vec::new();
    {
        let mut store = CLIENT.lock();
        for r in records(payload) {
            match r.op {
                SET => {
                    if r.key == DIMENSIONS {
                        dims.push((r.uuid.clone(), decode_dims(&r.value)));
                    }
                    store.entry(r.uuid).or_default().insert(r.key, r.value);
                }
                REMOVE => {
                    if r.key == DIMENSIONS {
                        dims.push((r.uuid.clone(), None));
                    }
                    if let Some(v) = store.get_mut(&r.uuid) {
                        v.remove(&r.key);
                    }
                }
                CLEAR => {
                    let removed = store.remove(&r.uuid);
                    if removed.is_some_and(|v| v.contains_key(DIMENSIONS)) {
                        dims.push((r.uuid.clone(), None));
                    }
                }
                _ => {}
            }
        }
    }
    for (uuid, d) in dims {
        let [w, h, e] = d.unwrap_or([-1.0, 0.0, 0.0]);
        unsafe {
            let _: Option<()> = host!(
                "dev/yog/YogClientApi",
                "setDimensions",
                [S(YogStr::from_str(&uuid)), F(w), F(h), F(e)]
            );
        }
    }
}

pub(crate) fn encode_dims(w: f32, h: f32, eye: f32) -> Vec<u8> {
    [w, h, eye].iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn decode_dims(b: &[u8]) -> Option<[f32; 3]> {
    let f = |i: usize| {
        b.get(i * 4..i * 4 + 4)
            .map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    };
    Some([f(0)?, f(1)?, f(2)?])
}

/// What this client knows of `uuid` (for body frames).
pub(crate) fn client_values(uuid: &str) -> Vec<(String, Vec<u8>)> {
    CLIENT
        .lock()
        .get(uuid)
        .map(|v| v.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}

/// `nativeEntityDataClientReset()`: the client left a world — forget it all.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeEntityDataClientReset<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    CLIENT.lock().clear();
}

/// What this client was told entity `uuid` keeps under `key`.
pub(crate) fn client_get(uuid: &str, key: &str) -> Option<Vec<u8>> {
    CLIENT.lock().get(uuid).and_then(|v| v.get(key)).cloned()
}

/// What this client knows the frame's entity keeps, put in its body frame.
pub(crate) fn fill_frame(frame: &mut yog_body::Frame) {
    frame.data = client_values(&frame.uuid)
        .into_iter()
        .map(|(key, value)| yog_body::wire::Kept { key, value })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_read_back() {
        let mut b = Vec::new();
        record(&mut b, SET, "u", "k", &[1, 2, 3]);
        record(&mut b, CLEAR, "u", "", &[]);
        let r = records(&b);
        assert_eq!(r.len(), 2);
        assert_eq!(
            (r[0].op, r[0].key.as_str(), r[0].value.as_slice()),
            (SET, "k", &[1u8, 2, 3][..])
        );
        assert_eq!(r[1].op, CLEAR);
    }

    #[test]
    fn frames_get_their_data() {
        CLIENT
            .lock()
            .entry("ab".into())
            .or_default()
            .insert("m:k".into(), vec![9]);
        let mut frame = yog_body::Frame {
            uuid: "ab".into(),
            ..Default::default()
        };
        fill_frame(&mut frame);
        assert_eq!(frame.data("m:k"), Some(&[9u8][..]));
    }
}

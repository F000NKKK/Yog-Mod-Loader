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
use std::os::raw::c_void;
use std::sync::{LazyLock, Mutex};

use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::jbyteArray;
use yog_abi::{YogBytesSink, YogStr};

/// The channel the loader syncs entity data on.
pub(crate) const CHANNEL: &str = "yog:entity_data";
/// The key an entity's own hitbox travels under (`entity_set_dimensions`):
/// width, height, eye height as three little-endian f32.
pub(crate) const DIMENSIONS: &str = "yog:dimensions";

const SET: u8 = 1;
const REMOVE: u8 = 2;
const CLEAR: u8 = 3;

/// `entity_data_set` flags.
pub(crate) const PRESENT: u32 = 1;
pub(crate) const PERSIST: u32 = 2;

struct Value {
    bytes: Vec<u8>,
    persist: bool,
}

type Values = BTreeMap<String, Value>;

static SERVER: LazyLock<Mutex<HashMap<String, Values>>> = LazyLock::new(Default::default);
/// Changes this tick, by entity, then key (`None`: removed) — sent once, at
/// the end of the tick: a key set twice goes once, an entity's changes in
/// one packet.
static PENDING: LazyLock<Mutex<HashMap<String, BTreeMap<String, Option<Vec<u8>>>>>> = LazyLock::new(Default::default);
static CLIENT: LazyLock<Mutex<HashMap<String, BTreeMap<String, Vec<u8>>>>> = LazyLock::new(Default::default);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

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
            let n = take(&mut b, 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize)?;
            let value = take(&mut b, n)?.to_vec();
            Some(Record { op, uuid, key, value })
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
        let _: Option<bool> = bridge!("sendToTracking", [S(YogStr::from_str(uuid)), S(YogStr::from_str(CHANNEL)), Bytes(bytes)]);
    }
}

/// Sets (`Some`) or removes a value, and tells the watchers if it changed.
pub(crate) fn set(uuid: &str, key: &str, value: Option<&[u8]>, persist: bool) {
    let changed = {
        let mut store = lock(&SERVER);
        match value {
            Some(v) => {
                let values = store.entry(uuid.to_owned()).or_default();
                let same = values.get(key).is_some_and(|old| old.bytes == v && old.persist == persist);
                values.insert(key.to_owned(), Value { bytes: v.to_vec(), persist });
                !same
            }
            None => {
                let removed = store.get_mut(uuid).is_some_and(|values| values.remove(key).is_some());
                if store.get(uuid).is_some_and(|v| v.is_empty()) {
                    store.remove(uuid);
                }
                removed
            }
        }
    };
    if changed {
        lock(&PENDING).entry(uuid.to_owned()).or_default().insert(key.to_owned(), value.map(<[u8]>::to_vec));
    }
}

/// Sends this tick's changes: one packet per entity, to who sees it.
pub(crate) fn flush() {
    let pending = std::mem::take(&mut *lock(&PENDING));
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
    lock(&SERVER).get(uuid)?.get(key).map(|v| v.bytes.clone())
}

pub(crate) unsafe extern "C" fn srv_entity_data_set(
    _ctx: *mut c_void,
    uuid: YogStr,
    key: YogStr,
    data: *const u8,
    len: u32,
    flags: u32,
) -> bool {
    let (uuid, key) = unsafe { (uuid.as_str(), key.as_str()) };
    if uuid.is_empty() || key.is_empty() {
        return false;
    }
    let value = (flags & PRESENT != 0).then(|| unsafe { std::slice::from_raw_parts(data, len as usize) });
    set(uuid, key, value, flags & PERSIST != 0);
    true
}

pub(crate) unsafe extern "C" fn srv_entity_data_get(
    _ctx: *mut c_void,
    uuid: YogStr,
    key: YogStr,
    sink: *mut c_void,
    write: YogBytesSink,
) -> bool {
    let Some(v) = (unsafe { get(uuid.as_str(), key.as_str()) }) else { return false };
    unsafe { write(sink, v.as_ptr(), v.len()) };
    true
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
    if let Some(values) = lock(&SERVER).get(&uuid) {
        record(&mut b, CLEAR, &uuid, "", &[]);
        for (k, v) in values {
            record(&mut b, SET, &uuid, k, &v.bytes);
        }
    }
    if b.is_empty() {
        return std::ptr::null_mut();
    }
    env.byte_array_from_slice(&b).map(|a| a.into_raw()).unwrap_or(std::ptr::null_mut())
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
    if let Some(values) = lock(&SERVER).get(&uuid) {
        for (k, v) in values.iter().filter(|(_, v)| v.persist) {
            record(&mut b, SET, "", k, &v.bytes);
        }
    }
    if b.is_empty() {
        return std::ptr::null_mut();
    }
    env.byte_array_from_slice(&b).map(|a| a.into_raw()).unwrap_or(std::ptr::null_mut())
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
    let Ok(saved) = env.convert_byte_array(&saved) else { return };
    let mut store = lock(&SERVER);
    let values = store.entry(uuid).or_default();
    for r in records(&saved) {
        if r.op == SET {
            values.insert(r.key, Value { bytes: r.value, persist: true });
        }
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
    lock(&SERVER).remove(&uuid);
}

/// The server stops: nothing carries over.
pub(crate) fn server_stopped() {
    lock(&SERVER).clear();
    lock(&PENDING).clear();
}

// ── Client ───────────────────────────────────────────────────────────────────

/// Records from the server.
pub(crate) fn receive(payload: &[u8]) {
    let mut dims: Vec<(String, Option<[f32; 3]>)> = Vec::new();
    {
        let mut store = lock(&CLIENT);
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
                    if store.remove(&r.uuid).is_some_and(|v| v.contains_key(DIMENSIONS)) {
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
            let _: Option<()> = host!("dev/yog/YogClientApi", "setDimensions", [S(YogStr::from_str(&uuid)), F(w), F(h), F(e)]);
        }
    }
}

pub(crate) fn encode_dims(w: f32, h: f32, eye: f32) -> Vec<u8> {
    [w, h, eye].iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn decode_dims(b: &[u8]) -> Option<[f32; 3]> {
    let f = |i: usize| b.get(i * 4..i * 4 + 4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]));
    Some([f(0)?, f(1)?, f(2)?])
}

/// What this client knows of `uuid` (for body frames).
pub(crate) fn client_values(uuid: &str) -> Vec<(String, Vec<u8>)> {
    lock(&CLIENT).get(uuid).map(|v| v.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default()
}

/// `nativeEntityDataClientReset()`: the client left a world — forget it all.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeEntityDataClientReset<'l>(_env: JNIEnv<'l>, _class: JClass<'l>) {
    lock(&CLIENT).clear();
}

pub(crate) unsafe extern "C" fn api_client_entity_data(
    _ctx: *mut c_void,
    uuid: YogStr,
    key: YogStr,
    sink: *mut c_void,
    write: YogBytesSink,
) -> bool {
    let (uuid, key) = unsafe { (uuid.as_str(), key.as_str()) };
    let store = lock(&CLIENT);
    let Some(v) = store.get(uuid).and_then(|v| v.get(key)) else { return false };
    unsafe { write(sink, v.as_ptr(), v.len()) };
    true
}

/// Adds what this client knows of the frame's entity to a body frame
/// (`yog_body::Frame`): the frame ends with its data.
pub(crate) fn with_data(frame: &[u8]) -> Vec<u8> {
    let uuid = frame
        .get(1..3)
        .map(|n| u16::from_le_bytes([n[0], n[1]]) as usize)
        .and_then(|n| frame.get(3..3 + n))
        .and_then(|s| std::str::from_utf8(s).ok())
        .unwrap_or("");
    let values = client_values(uuid);
    let mut out = Vec::with_capacity(frame.len() + 2);
    out.extend_from_slice(frame);
    out.extend_from_slice(&(values.len() as u16).to_le_bytes());
    for (k, v) in values {
        put_str(&mut out, &k);
        out.extend_from_slice(&(v.len() as u32).to_le_bytes());
        out.extend_from_slice(&v);
    }
    out
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
        assert_eq!((r[0].op, r[0].key.as_str(), r[0].value.as_slice()), (SET, "k", &[1u8, 2, 3][..]));
        assert_eq!(r[1].op, CLEAR);
    }

    #[test]
    fn frames_get_their_data() {
        lock(&CLIENT).entry("ab".into()).or_default().insert("m:k".into(), vec![9]);
        let frame = [1u8, 2, 0, b'a', b'b'];
        let out = with_data(&frame);
        assert_eq!(&out[..5], &frame);
        assert_eq!(&out[5..7], &1u16.to_le_bytes());
    }
}

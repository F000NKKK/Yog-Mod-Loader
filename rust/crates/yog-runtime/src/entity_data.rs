//! Entity data (ABI 3.2): values mods keep on an entity by key — the
//! loader's `SynchedEntityData`/attachments. The server holds them and sends
//! each change to the players who see the entity (and to it, if a player);
//! a player who starts seeing an entity gets all of it. Changes go out once
//! a tick, batched per entity. Values marked
//! `persist` are saved with the entity. Only the server writes them: a
//! client asks for a change with its mod's packet, the server decides.
//!
//! On the wire (`yog:entity_data`) and in a saved entity: a list of
//! [`Change`]s, Yog Data — set, remove, or clear (every key of the entity,
//! key and value empty).

use std::collections::{BTreeMap, HashMap};
use yog_sync::{LazyLock, Mutex};

use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::jbyteArray;
use yog_abi::YogStr;
use yog_data::Schema;
use yog_data::with::Raw;

pub(crate) use yog_ids::yog::{DIMENSIONS, ENTITY_DATA};

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

// ── Changes ──────────────────────────────────────────────────────────────────

/// What a change does to a key.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Set,
    Remove,
    /// Every key of the entity.
    Clear,
}

/// One change of an entity's values.
#[derive(Schema, Debug, Clone, PartialEq)]
struct Change {
    op: Op,
    uuid: String,
    key: String,
    #[data(with = "Raw")]
    value: Vec<u8>,
}

impl Change {
    fn new(op: Op, uuid: &str, key: &str, value: &[u8]) -> Change {
        Change {
            op,
            uuid: uuid.to_owned(),
            key: key.to_owned(),
            value: value.to_vec(),
        }
    }
}

/// Changes as bytes, `None` for none.
fn encode(changes: &[Change]) -> Option<Vec<u8>> {
    (!changes.is_empty()).then(|| yog_data::to_bytes(&changes.to_vec()))
}

/// Changes read back; what doesn't read is none.
fn decode(bytes: &[u8]) -> Vec<Change> {
    yog_data::read_bytes::<Vec<Change>>(bytes).unwrap_or_default()
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
        let changes: Vec<Change> = keys
            .into_iter()
            .map(|(key, value)| match value {
                Some(v) => Change::new(Op::Set, &uuid, &key, &v),
                None => Change::new(Op::Remove, &uuid, &key, &[]),
            })
            .collect();
        if let Some(bytes) = encode(&changes) {
            send_to_watchers(&uuid, &bytes);
        }
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
    crate::jni_util::guarded("nativeEntityDataFor", std::ptr::null_mut(), || {
        let uuid = jstr!(env, uuid, std::ptr::null_mut());
        let mut changes = Vec::new();
        if let Some(values) = SERVER.lock().get(&uuid) {
            changes.push(Change::new(Op::Clear, &uuid, "", &[]));
            changes.extend(
                values
                    .iter()
                    .map(|(k, v)| Change::new(Op::Set, &uuid, k, &v.bytes)),
            );
        }
        let Some(b) = encode(&changes) else {
            return std::ptr::null_mut();
        };
        env.byte_array_from_slice(&b)
            .map(|a| a.into_raw())
            .unwrap_or(std::ptr::null_mut())
    })
}

/// `byte[] nativeEntityDataSave(String uuid)`: the values kept with the
/// entity when it's saved; null for none.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeEntityDataSave<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    uuid: JString<'l>,
) -> jbyteArray {
    crate::jni_util::guarded("nativeEntityDataSave", std::ptr::null_mut(), || {
        let uuid = jstr!(env, uuid, std::ptr::null_mut());
        let changes: Vec<Change> = SERVER
            .lock()
            .get(&uuid)
            .into_iter()
            .flat_map(|values| values.iter().filter(|(_, v)| v.persist))
            .map(|(k, v)| Change::new(Op::Set, "", k, &v.bytes))
            .collect();
        let Some(b) = encode(&changes) else {
            return std::ptr::null_mut();
        };
        env.byte_array_from_slice(&b)
            .map(|a| a.into_raw())
            .unwrap_or(std::ptr::null_mut())
    })
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
    crate::jni_util::guard("nativeEntityDataLoad", || {
        let uuid = jstr!(env, uuid);
        let Ok(saved) = env.convert_byte_array(&saved) else {
            return;
        };
        let mut store = SERVER.lock();
        let values = store.entry(uuid).or_default();
        for r in decode(&saved) {
            if r.op == Op::Set {
                values.insert(
                    r.key,
                    Value {
                        bytes: r.value,
                        persist: true,
                    },
                );
            }
        }
    });
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
    crate::jni_util::guard("nativeEntityDataSet", || {
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
    });
}

/// `nativeEntityDataForget(String uuid)`: the entity left the server's
/// memory (unloaded, gone); what it keeps was saved with it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeEntityDataForget<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    uuid: JString<'l>,
) {
    crate::jni_util::guard("nativeEntityDataForget", || {
        let uuid = jstr!(env, uuid);
        SERVER.lock().remove(&uuid);
    });
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
        for r in decode(payload) {
            match r.op {
                Op::Set => {
                    if r.key == DIMENSIONS {
                        dims.push((r.uuid.clone(), decode_dims(&r.value)));
                    }
                    store.entry(r.uuid).or_default().insert(r.key, r.value);
                }
                Op::Remove => {
                    if r.key == DIMENSIONS {
                        dims.push((r.uuid.clone(), None));
                    }
                    if let Some(v) = store.get_mut(&r.uuid) {
                        v.remove(&r.key);
                    }
                }
                Op::Clear => {
                    let removed = store.remove(&r.uuid);
                    if removed.is_some_and(|v| v.contains_key(DIMENSIONS)) {
                        dims.push((r.uuid.clone(), None));
                    }
                }
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

/// An entity's size as it keeps it (`yog:dimensions`): width, height, eye height.
pub(crate) fn encode_dims(w: f32, h: f32, eye: f32) -> Vec<u8> {
    yog_data::to_bytes(&[w, h, eye])
}

fn decode_dims(b: &[u8]) -> Option<[f32; 3]> {
    yog_data::read_bytes(b).ok()
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
    crate::jni_util::guard("nativeEntityDataClientReset", || {
        CLIENT.lock().clear();
    });
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
    fn changes_read_back() {
        let changes = vec![
            Change::new(Op::Set, "u", "k", &[1, 2, 3]),
            Change::new(Op::Clear, "u", "", &[]),
        ];
        assert_eq!(decode(&encode(&changes).unwrap()), changes);
        assert_eq!(
            decode_dims(&encode_dims(0.6, 1.8, 1.62)),
            Some([0.6, 1.8, 1.62])
        );
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

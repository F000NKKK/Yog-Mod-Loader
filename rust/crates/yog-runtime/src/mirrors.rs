//! Mirrors (`docs/design/bus.md`, "Mirrors"): the runtime keeps the column
//! store, answers `yog:mirror/…`, and after each server tick calls every
//! provider whose fields are due. What a mod declared or provides goes
//! when the mod does.

use std::collections::HashMap;

use yog_event::Message;
use yog_event::mirror::{
    Declaration, Declare, Declared, Keys, Place, Provide, Release, Touch, Write,
};
use yog_hot_reload::ModuleGeneration;
use yog_ids::key_space;
use yog_mirror::{DueList, Store};
use yog_sync::{LazyLock, Mutex};

struct Mirrors {
    store: Store,
    /// Whose each declaration is.
    declared_by: HashMap<u32, ModuleGeneration>,
    /// Whose each writer (a mirror's address) is.
    written_by: HashMap<u32, ModuleGeneration>,
    tick: u64,
    /// What's due this tick (kept: no list made a tick).
    due: DueList,
}

static MIRRORS: LazyLock<Mutex<Mirrors>> = LazyLock::new(|| Mutex::new(Mirrors::new()));

/// The game's key spaces, each after its parent.
const SPACES: &[(&str, Option<&str>)] = &[
    (key_space::ENTITY, None),
    (key_space::PLAYER, Some(key_space::ENTITY)),
    (key_space::BLOCK, None),
];

impl Mirrors {
    fn new() -> Mirrors {
        let mut store = Store::default();
        for (space, parent) in SPACES {
            if let Err(e) = store.space(space, *parent) {
                yog_logging::warn!("mirrors: {e}");
            }
        }
        // The game's own mirrors: the Java host answers their addresses.
        for m in yog_mirror::host::ALL {
            let writer = crate::bus::ROUTER.resolve(m.address);
            if let Err(e) = store.provide(&m.spec(), writer, true) {
                yog_logging::warn!("mirrors: {e}");
            }
        }
        Mirrors {
            store,
            declared_by: HashMap::new(),
            written_by: HashMap::new(),
            tick: 0,
            due: DueList::default(),
        }
    }
}

fn declare(d: &Declare) -> Declared {
    let mut m = MIRRORS.lock();
    match m.store.declare(&d.spec, d.wanted, d.rate) {
        Ok(made) => {
            if let Some(by) = crate::bus::caller() {
                m.declared_by.insert(made.id, by);
            }
            let place = |l: yog_mirror::Linked| Place {
                column: l.column as u64,
                depth: l.depth,
            };
            Declared {
                declaration: Some(Declaration {
                    id: made.id,
                    space: made.space as u64,
                    rows: made.rows as u64,
                    fields: made.fields.into_iter().map(|f| f.map(place)).collect(),
                }),
                problem: None,
            }
        }
        Err(problem) => Declared {
            declaration: None,
            problem: Some(problem),
        },
    }
}

fn release(r: &Release) -> bool {
    let mut m = MIRRORS.lock();
    m.declared_by.remove(&r.id);
    m.store.release(r.id);
    true
}

fn provide(p: &Provide) -> Option<yog_data::Diagnostic> {
    let writer = crate::bus::ROUTER.resolve(&p.spec.name);
    let mut m = MIRRORS.lock();
    if let Err(problem) = m.store.provide(&p.spec, writer, p.host) {
        return Some(problem);
    }
    if let Some(by) = crate::bus::caller() {
        m.written_by.insert(writer, by);
    }
    None
}

fn keys(k: &Keys) -> Vec<u32> {
    let mut m = MIRRORS.lock();
    let Some(space) = m.store.space_id(&k.space) else {
        return Vec::new();
    };
    for key in &k.take {
        m.store.remove(space, *key);
    }
    k.put
        .iter()
        .map(|key| m.store.insert(space, key.key, key.parent))
        .collect()
}

/// After a server tick: every provider with fields due is told which.
pub(crate) fn after_tick() {
    let due = {
        let mut m = MIRRORS.lock();
        let tick = m.tick;
        m.tick += 1;
        let mut due = std::mem::take(&mut m.due);
        m.store.due(tick, &mut due);
        due
    };
    // Unlocked: a writer may put keys or touch its mirror meanwhile.
    let mut write = Write::default();
    for d in &due.turns {
        write.fields = d.fields;
        write.space = d.space;
        write.rows = d.rows;
        write.columns.clear();
        write.columns.extend_from_slice(due.columns(d));
        write.with_bytes(|payload| crate::bus::tell(d.writer, payload));
    }
    MIRRORS.lock().due = due;
}

/// Forgets what the mod of `generation` declared and provides (its
/// columns go to the next provider of their facts, or wait).
pub(crate) fn purge(generation: ModuleGeneration) {
    let mut m = MIRRORS.lock();
    let declared: Vec<u32> = owned(&m.declared_by, generation);
    for id in declared {
        m.declared_by.remove(&id);
        m.store.release(id);
    }
    let writers: Vec<u32> = owned(&m.written_by, generation);
    for writer in writers {
        m.written_by.remove(&writer);
        m.store.withdraw(writer);
    }
}

fn owned(by: &HashMap<u32, ModuleGeneration>, generation: ModuleGeneration) -> Vec<u32> {
    by.iter()
        .filter(|(_, g)| **g == generation)
        .map(|(id, _)| *id)
        .collect()
}

/// A direct buffer over `length` bytes at `address`: a column's header or
/// memory, as a `Write` handed it to the Java host.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_mirror_YogMirrorMemory_nativeBuffer<'l>(
    mut env: jni::JNIEnv<'l>,
    _class: jni::objects::JClass<'l>,
    address: jni::sys::jlong,
    length: jni::sys::jlong,
) -> jni::sys::jobject {
    let (Ok(length), false) = (usize::try_from(length), address == 0) else {
        return std::ptr::null_mut();
    };
    // SAFETY: the runtime's own memory, alive while its column is.
    let buffer = unsafe { env.new_direct_byte_buffer(address as *mut u8, length) };
    buffer.map_or(std::ptr::null_mut(), |b| {
        jni::objects::JObject::from(b).into_raw()
    })
}

/// The mirror services; once, at start.
pub(crate) fn register() {
    crate::bus::serve::<Declare>(declare);
    crate::bus::serve::<Release>(release);
    crate::bus::serve::<Provide>(provide);
    crate::bus::serve::<Touch>(|t| {
        MIRRORS.lock().store.touch(&t.name);
        true
    });
    crate::bus::serve::<Keys>(keys);
}

#[cfg(test)]
mod jvm;
#[cfg(test)]
mod tests;

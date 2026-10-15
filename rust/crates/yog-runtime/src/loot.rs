//! Loot (ABI 3.4): the hosts ask here about every loot table rolled — a
//! chest's, a block's drops, a mob's — before its items are given; the
//! mods' handlers change them one after another.

use std::os::raw::c_void;

use jni::JNIEnv;
use jni::objects::{JByteArray, JClass};
use jni::sys::{jboolean, jbyteArray};
use yog_abi::YogLootFn;
use yog_data::Schema;
use yog_registry::{Loot, LootStack};

use super::{each, guard, handlers, handlers_mut, srv_ptr};

pub(crate) unsafe extern "C" fn api_on_loot(ctx: *mut c_void, ud: *mut c_void, f: YogLootFn) {
    unsafe { handlers_mut(ctx).loot.push((super::current_registration(), ud, f)) }
}

unsafe extern "C" fn append(sink: *mut c_void, data: *const u8, len: usize) {
    // SAFETY: `sink` is the `Vec<u8>` `roll` passes; `data` is valid for the call.
    unsafe { (*(sink as *mut Vec<u8>)).extend_from_slice(std::slice::from_raw_parts(data, len)) }
}

/// `loot` after every handler, or `None` when none changed it.
fn roll(loot: &[u8]) -> Option<Vec<LootStack>> {
    let entries = handlers().loot.clone();
    if entries.is_empty() {
        return None;
    }
    let mut loot = yog_data::read_bytes::<Loot>(loot).ok()?;
    let mut changed = false;
    let srv = srv_ptr();
    guard("on_loot", || {
        each(&entries, |ud, f| {
            let bytes = yog_data::binary::encode(&loot.write());
            let mut out = Vec::new();
            unsafe { f(ud, srv, bytes.as_ptr(), bytes.len(), &mut out as *mut Vec<u8> as *mut c_void, append) };
            if out.is_empty() {
                return;
            }
            match yog_data::read_bytes::<Vec<LootStack>>(&out) {
                Ok(items) => {
                    loot.items = items;
                    changed = true;
                }
                Err(e) => yog_logging::error!("on_loot: a handler's items don't read: {e:?}"),
            }
        });
    });
    changed.then_some(loot.items)
}

/// `boolean nativeHasLoot()`: whether any mod handles loot (the hosts
/// skip the rest when none does).
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeHasLoot<'l>(_env: JNIEnv<'l>, _class: JClass<'l>) -> jboolean {
    (!handlers().loot.is_empty()) as jboolean
}

/// `byte[] nativeLoot(byte[] loot)`: the items a rolled loot table gives
/// after the mods' handlers (`Vec<LootStack>`, Yog Data), or null to give
/// what it rolled.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeLoot<'l>(env: JNIEnv<'l>, _class: JClass<'l>, loot: JByteArray<'l>) -> jbyteArray {
    let Ok(loot) = env.convert_byte_array(&loot) else { return std::ptr::null_mut() };
    let Some(items) = roll(&loot) else { return std::ptr::null_mut() };
    let bytes = yog_data::binary::encode(&items.write());
    env.byte_array_from_slice(&bytes).map(|a| a.into_raw()).unwrap_or(std::ptr::null_mut())
}

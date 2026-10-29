//! Mods' injections, asked for before the game's classes load
//! (`docs/design/nanites.md`): the host's Mixin plugin calls in while the
//! game starts; each mod's library is opened on its own (its `register`
//! not run), asked (`yog_mod_nanites`), and closed — the game loads it as
//! usual later.

use std::path::Path;
use substrate_lock::Locked;

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jbyteArray;
use libloading::{Library, Symbol};
use yog_abi::YogReply;
use yog_event::nanite::Nanites;

use crate::mods::loading::{AbiVersionFn, extract_yog};

/// A mod's injections to `reply` (`yog_api::export_mod!`).
type NanitesFn = unsafe extern "C" fn(reply: *const YogReply);

/// The injections of the mod at `yog` (a `.yog`); none when it has none, or
/// isn't of this ABI.
fn of_mod(yog: &Path) -> Nanites {
    let Some(lib_path) = extract_yog(yog) else {
        return Nanites::default();
    };
    let declared = ask(&lib_path).unwrap_or_default();
    // The extraction was only for asking: the game extracts its own.
    if let Err(e) = std::fs::remove_file(&lib_path) {
        yog_logging::warn!("nanites: {}: {e}", lib_path.display());
    }
    declared
}

fn ask(lib_path: &Path) -> Option<Nanites> {
    // SAFETY: a mod's library, opened the way the game opens it.
    let lib = unsafe { Library::new(lib_path) }.ok()?;
    // SAFETY: the symbols' types are what `export_mod!` defines.
    unsafe {
        let abi: Symbol<AbiVersionFn> = lib.get(b"yog_abi_version").ok()?;
        if !yog_abi::compatible(abi()) {
            return None;
        }
        let nanites: Symbol<NanitesFn> = lib.get(b"yog_mod_nanites").ok()?;
        let mut bytes = Vec::new();
        nanites(&YogReply::to_vec(&mut bytes));
        yog_data::read_bytes(&bytes).ok()
    }
}

/// Every mod's injections in `dir` (the game's `yog-mods`).
pub(crate) fn declared(dir: &Path) -> Nanites {
    let mut all = Nanites::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return all;
    };
    for path in entries.flatten().map(|e| e.path()) {
        if yog_ids::Format::Mod.is(&path) {
            all.injects.extend(of_mod(&path).injects);
        }
    }
    all
}

/// The answer for a mods directory, kept: the host asks again from another
/// class loader's copy of its classes, and the mods' libraries aren't
/// opened twice.
static ASKED: std::sync::Mutex<Option<(String, Vec<u8>)>> = std::sync::Mutex::new(None);

/// `YogNanites.nativeDeclarations(modsDir)`: every mod's injections, Yog
/// Data (`yog_event::nanite::Nanites`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_nanite_YogNanites_nativeDeclarations<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    mods_dir: JString<'l>,
) -> jbyteArray {
    crate::jni::util::guarded("nativeDeclarations", std::ptr::null_mut(), || {
        let dir: String = env
            .get_string(&mods_dir)
            .map(Into::into)
            .unwrap_or_default();
        let mut asked = ASKED.locked();
        let bytes = match asked.as_ref() {
            Some((d, bytes)) if *d == dir => bytes.clone(),
            _ => {
                let bytes = yog_data::to_bytes(&declared(Path::new(&dir)));
                *asked = Some((dir, bytes.clone()));
                bytes
            }
        };
        env.byte_array_from_slice(&bytes)
            .map_or(std::ptr::null_mut(), |a| a.into_raw())
    })
}

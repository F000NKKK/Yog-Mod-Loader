//! JNI: the runtime's start (`nativeInit`).
use crate::api::client::client_advancement_done;
use crate::api::client::client_tag_items;
use crate::globals::API_TABLE;
use crate::globals::JAVA_VM;
use crate::jni::util::guard;
use crate::logging::host_log;
use crate::mods::loading::load_mods;
use jni::JNIEnv;
use jni::objects::JClass;
use jni::objects::JString;
use std::path::Path;
use yog_abi::YogApi;

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeInit<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    mods_dir: JString<'l>,
) {
    if let Ok(vm) = env.get_java_vm() {
        let _ = JAVA_VM.set(vm);
    }
    // From here on the runtime's records go to the game's log.
    yog_logging::set_sink(host_log);

    let dir = env
        .get_string(&mods_dir)
        .map(String::from)
        .unwrap_or_default();

    let api: &'static YogApi = &API_TABLE;
    crate::bus::register_duties();
    crate::bus::install_caller();

    // A book's entries an advancement unlocks wait for this client's player to have it.
    yog_book::set_unlocked(client_advancement_done);
    // A book's recipe shows a tag's items in turn.
    yog_book::set_tag_items(client_tag_items);

    guard("mod loading", || {
        load_mods(Path::new(&dir), api);
    });

    #[cfg(feature = "dev")]
    crate::mods::control_socket::start_if_requested();

    yog_logging::info!("runtime initialised — the gate is open.");
}

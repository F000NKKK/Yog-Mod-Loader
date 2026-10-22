//! JNI: chunk generation, content, packet channels.
use crate::content_blob;
use crate::handlers::handlers;
use crate::jni_util::guard;
use jni::JNIEnv;
use jni::objects::JClass;
use jni::objects::JString;
use jni::sys::jint;
use jni::sys::jstring;
use yog_abi::YogStr;

/// Places one block in the chunk being generated, through
/// `YogCallbackChunkGenerator.setBlockInGeneratingChunk` (same thread: the
/// round trip is synchronous).
fn place_block(x: i32, y: i32, z: i32, block_id: &str) -> bool {
    unsafe {
        host!(
            "dev/yog/YogCallbackChunkGenerator",
            "setBlockInGeneratingChunk",
            [I(x), I(y), I(z), S(YogStr::from_str(block_id))]
        )
        .unwrap_or(false)
    }
}

/// Generates chunk column `chunk_x, chunk_z` with generator type `generator_type_id`:
/// its declarative terrain (if a mod declared one), then the handlers of
/// `world/generation/chunk` for that type, in turn; the column's blocks then go
/// to the host.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeGenerateChunk<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    generator_type_id: JString<'l>,
    chunk_x: jint,
    chunk_z: jint,
    min_y: jint,
    height: jint,
) {
    let generator = jstr!(env, generator_type_id);
    let mut chunk = yog_dimensions::GeneratedChunk {
        generator,
        chunk_x,
        chunk_z,
        min_y,
        height,
        ..Default::default()
    };
    if let Some(terrain) = handlers().terrains.get(&chunk.generator).cloned() {
        guard("terrain", || {
            terrain.generate(chunk_x, chunk_z, min_y, height, |x, y, z, block| {
                chunk.set_block(x, y, z, block);
            });
        });
    }
    let chunk = crate::bus::changed(chunk);
    for (x, y, z, block) in chunk.placed() {
        place_block(x, y, z, block);
    }
}

/// Everything the host registers as one Yog Data blob (see `content_blob`),
/// built on the first call and kept.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeContent<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jni::sys::jbyteArray {
    crate::jni_util::guarded("nativeContent", std::ptr::null_mut(), || {
        let h = handlers();
        let bytes = {
            let mut cached = h.content_blob.lock();
            cached
                .get_or_insert_with(|| yog_sync::Arc::new(content_blob::blob(&h)))
                .clone()
        };
        env.byte_array_from_slice(&bytes)
            .map(|a| a.into_raw())
            .unwrap_or(std::ptr::null_mut())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativePacketChannels<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    crate::jni_util::guarded("nativePacketChannels", std::ptr::null_mut(), || {
        let s = crate::bus::ROUTER
            .listened_keys(<yog_event::server::net::Packet as yog_event::Event>::ADDRESS)
            .join("\n");
        env.new_string(s)
            .map(jni::objects::JString::into_raw)
            .unwrap_or(std::ptr::null_mut())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeClientPacketChannels<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    crate::jni_util::guarded("nativeClientPacketChannels", std::ptr::null_mut(), || {
        let s = crate::bus::ROUTER
            .listened_keys(<yog_event::client::net::Packet as yog_event::Event>::ADDRESS)
            .join("\n");
        env.new_string(s)
            .map(jni::objects::JString::into_raw)
            .unwrap_or(std::ptr::null_mut())
    })
}

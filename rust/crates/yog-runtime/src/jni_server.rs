//! JNI: chunk generation, content, packet channels.

use super::*;

/// Called by a registered chunk-generator closure (via `ChunkWriter::set_block`)
/// to place one block in the chunk currently being generated. Calls back
/// into `YogCallbackChunkGenerator.setBlockInGeneratingChunk` on the Java
/// side — same thread, since the whole round trip is synchronous.
pub(super) unsafe extern "C" fn chunk_writer_set_block(
    _ctx: *mut c_void,
    x: i32,
    y: i32,
    z: i32,
    block_id: YogStr,
) -> bool {
    unsafe {
        host!(
            "dev/yog/YogCallbackChunkGenerator",
            "setBlockInGeneratingChunk",
            [I(x), I(y), I(z), S(block_id)]
        )
        .unwrap_or(false)
    }
}

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
    let id: String = match env.get_string(&generator_type_id) {
        Ok(s) => String::from(s),
        Err(_) => return,
    };
    let terrain = handlers().terrains.get(&id).cloned();
    if let Some(g) = terrain {
        guard("chunk_generator", || {
            g.generate(chunk_x, chunk_z, min_y, height, |x, y, z, block| unsafe {
                chunk_writer_set_block(std::ptr::null_mut(), x, y, z, YogStr::from_str(block));
            });
        });
        return;
    }
    let Some((module, ud, cb)) = handlers().chunk_generators.get(&id).cloned() else {
        return;
    };
    if module.is_retiring() {
        return;
    }
    let Some(_call) = module.enter() else { return };
    let writer = yog_abi::YogChunkWriterApi {
        chunk_x,
        chunk_z,
        min_y,
        height,
        ctx: std::ptr::null_mut(),
        set_block: chunk_writer_set_block,
    };
    guard("chunk_generator", || unsafe {
        cb(ud, &writer as *const _);
    });
}

/// Everything the host registers as one Yog Data blob (see `content_blob`),
/// built on the first call and kept.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeContent<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jni::sys::jbyteArray {
    let h = handlers();
    let bytes = {
        let mut cached = h.content_blob.locked();
        cached.get_or_insert_with(|| std::sync::Arc::new(content_blob::blob(&h))).clone()
    };
    env.byte_array_from_slice(&bytes)
        .map(|a| a.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativePacketChannels<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let s = crate::bus::ROUTER.listened_keys(<yog_event::server::net::Packet as yog_event::Event>::ADDRESS).join("\n");
    env.new_string(s)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeClientPacketChannels<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let s = crate::bus::ROUTER.listened_keys(<yog_event::client::net::Packet as yog_event::Event>::ADDRESS).join("\n");
    env.new_string(s)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

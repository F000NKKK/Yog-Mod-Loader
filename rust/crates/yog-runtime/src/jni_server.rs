//! JNI: commands, chunk generation, content, packets.

use super::*;

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeCommandNames<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let names = handlers()
        .commands
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    env.new_string(names)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

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

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeTypedCommandSchemas<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    // One line per DISTINCT (name, schema) pair — a name can have several
    // typed registrations (different arg shapes); Brigadier merges repeated
    // `dispatcher.register()` calls for the same literal name into one tree,
    // so the Java side just needs every distinct schema once each.
    let mut lines = Vec::new();
    for (name, regs) in handlers().commands.iter() {
        let mut seen = std::collections::HashSet::new();
        for (_module, schema, _, _) in regs {
            if let Some(schema) = schema
                && seen.insert(schema.as_str())
            {
                lines.push(format!("{name}\t{schema}"));
            }
        }
    }
    env.new_string(lines.join("\n"))
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnCommand<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    name: JString<'l>,
    args: JString<'l>,
    source: JString<'l>,
    uuid: JString<'l>,
    dimension: JString<'l>,
) -> jstring {
    let (n, a, s, u, d) = (
        env.get_string(&name).map(String::from).unwrap_or_default(),
        env.get_string(&args).map(String::from).unwrap_or_default(),
        env.get_string(&source)
            .map(String::from)
            .unwrap_or_default(),
        env.get_string(&uuid).map(String::from).unwrap_or_default(),
        env.get_string(&dimension)
            .map(String::from)
            .unwrap_or_default(),
    );
    let h = handlers();
    let srv = srv_ptr();
    let args: Vec<&str> = if a.is_empty() { Vec::new() } else { a.split('\t').collect() };
    // One handler with `args`; its reply, or `None` when it passed.
    let call = |ud: &*mut c_void, f: &YogCommandFn, args: &str| -> Option<String> {
        let ev = yog_abi::YogCommandEvent {
            name: YogStr::from_str(&n),
            args: YogStr::from_str(args),
            source: YogStr::from_str(&s),
            uuid: YogStr::from_str(&u),
            dimension: YogStr::from_str(&d),
        };
        let mut buf = [0u8; 4096];
        let mut reply_len: u32 = 0;
        unsafe { f(*ud, srv, &ev, buf.as_mut_ptr(), buf.len() as u32, &mut reply_len) };
        (reply_len > 0).then(|| String::from_utf8_lossy(&buf[..reply_len as usize]).into_owned())
    };
    let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(regs) = h.commands.get(&n) else {
            return String::new();
        };
        let live = regs.iter().filter(|(module, ..)| !module.is_retiring());
        // 1. Typed handlers whose schema has exactly these many args.
        // 2. Ones ending in a `string`, with the extra words joined into it
        //    (the game may have parsed them as separate words).
        // 3. Plain (schema-less) handlers, with whatever was typed.
        for pass in 0..3 {
            for (module, schema, ud, f) in live.clone() {
                let joined;
                let these = match (pass, schema) {
                    (0, Some(schema)) if schema.split_whitespace().count() == args.len() => a.as_str(),
                    (1, Some(schema)) => {
                        let types: Vec<&str> = schema.split_whitespace().collect();
                        let k = types.len();
                        if types.last() != Some(&"string") || k == 0 || args.len() <= k {
                            continue;
                        }
                        let mut parts: Vec<String> = args[..k - 1].iter().map(|x| x.to_string()).collect();
                        parts.push(args[k - 1..].join(" "));
                        joined = parts.join("\t");
                        joined.as_str()
                    }
                    (2, None) => a.as_str(),
                    _ => continue,
                };
                let Some(_call) = module.enter() else {
                    continue;
                };
                if let Some(r) = call(ud, f, these) {
                    return r;
                }
            }
        }
        String::new()
    }))
    .unwrap_or_else(|_| {
        yog_logging::error!("a mod panicked handling command `{}`", n);
        String::new()
    });

    env.new_string(reply)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
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
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPacket<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    channel: JString<'l>,
    player: JString<'l>,
    payload: JByteArray<'l>,
) {
    let ch = env
        .get_string(&channel)
        .map(String::from)
        .unwrap_or_default();
    let pl = env
        .get_string(&player)
        .map(String::from)
        .unwrap_or_default();
    let data = env.convert_byte_array(&payload).unwrap_or_default();
    let ev = yog_abi::YogPacketEvent {
        channel: YogStr::from_str(&ch),
        player: YogStr::from_str(&pl),
        payload: data.as_ptr(),
        payload_len: data.len() as u32,
    };
    let h = handlers();
    let srv = srv_ptr();
    guard("on_packet", || {
        if let Some((module, ud, f)) = h.packets.get(&ch) {
            if module.is_retiring() {
                return;
            }
            let Some(_call) = module.enter() else { return };
            unsafe { f(*ud, srv, &ev) };
        }
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnClientPacket<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    channel: JString<'l>,
    payload: JByteArray<'l>,
) {
    let ch = env
        .get_string(&channel)
        .map(String::from)
        .unwrap_or_default();
    let data = env.convert_byte_array(&payload).unwrap_or_default();
    if ch == entity_data::ENTITY_DATA {
        guard("entity data", || entity_data::receive(&data));
        return;
    }
    let ev = yog_abi::YogPacketEvent {
        channel: YogStr::from_str(&ch),
        player: YogStr::EMPTY,
        payload: data.as_ptr(),
        payload_len: data.len() as u32,
    };
    let h = handlers();
    let srv = srv_ptr();
    guard("on_client_packet", || {
        if let Some((module, ud, f)) = h.client_packets.get(&ch) {
            if module.is_retiring() {
                return;
            }
            let Some(_call) = module.enter() else { return };
            unsafe { f(*ud, srv, &ev) };
        }
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativePacketChannels<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let s = handlers()
        .packets
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    env.new_string(s)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeClientPacketChannels<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let s = handlers()
        .client_packets
        .keys()
        .cloned()
        .chain([entity_data::ENTITY_DATA.to_owned()])
        .collect::<Vec<_>>()
        .join("\n");
    env.new_string(s)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

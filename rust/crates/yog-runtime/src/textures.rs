//! Textures the runtime makes (rasterized SVGs, text in Yog's fonts):
//! handed to the host as dynamic textures, drawn like any other.

use jni::JNIEnv;
use yog_abi::YogStr;

use crate::jcall::jcall;

/// Hands the host texture `id`: `w`×`h` RGBA pixels (straight alpha); whether it took it.
pub(crate) fn register(env: &mut JNIEnv, id: &str, w: u32, h: u32, rgba: &[u8]) -> bool {
    let ok: Option<bool> = unsafe {
        jcall!(env, "dev/yog/NativeDraw", "registerTexture", [S(YogStr::from_str(id)), I(w as i32), I(h as i32), Bytes(rgba)])
    };
    ok == Some(true)
}

/// Lets go of texture `id`.
pub(crate) fn release(env: &mut JNIEnv, id: &str) {
    let _: Option<()> = unsafe { jcall!(env, "dev/yog/NativeDraw", "releaseTexture", [S(YogStr::from_str(id))]) };
}

/// Draws the whole of texture `id` (`size` its pixels) into `[x, y, w, h]` GUI pixels, colors times `tint`.
pub(crate) fn draw(env: &mut JNIEnv, id: &str, size: (u32, u32), [x, y, w, h]: [f32; 4], tint: u32) {
    let (fw, fh) = (size.0 as f32, size.1 as f32);
    let _: Option<()> = unsafe {
        jcall!(
            env,
            "dev/yog/NativeDraw",
            "drawImage",
            [S(YogStr::from_str(id)), F(x), F(y), F(w), F(h), F(0.0), F(0.0), F(fw), F(fh), F(fw), F(fh), I(tint as i32)]
        )
    };
}

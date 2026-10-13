//! Calls from Rust into the host's `dev.yog.*` static methods.
//!
//! `JNIEnv::call_static_method` looks the class and method up by name and
//! re-parses the signature on every call — several JNI round-trips per
//! `drawRect`. [`jcall!`] instead gives each call site its own
//! [`StaticMethod`], which resolves once (class pinned by a global ref) and
//! then calls straight through the cached method id. The JNI signature is
//! derived from the argument kinds and the expected return type, so the two
//! cannot drift apart.
//!
//! Self-contained (only `jni` and `yog-abi`) so the JVM benchmark can include
//! it as-is.

use std::sync::OnceLock;

use jni::JNIEnv;
use jni::objects::{GlobalRef, JClass, JObject, JStaticMethodID, JString, JValueOwned};
use jni::signature::{Primitive, ReturnType};
use jni::sys::jvalue;
use yog_abi::YogStr;

/// Most parameters any bridge method takes (`NativeDraw.drawImage`: 12).
const MAX_ARGS: usize = 16;

/// One argument of a bridge call.
#[derive(Clone, Copy)]
pub(crate) enum JArg<'a> {
    /// `String` (copied into a fresh Java string for the call).
    S(YogStr),
    D(f64),
    F(f32),
    I(i32),
    J(i64),
    Z(bool),
    /// `byte[]` (copied into a fresh Java array for the call).
    Bytes(&'a [u8]),
}

impl JArg<'_> {
    fn descriptor(&self) -> &'static str {
        match self {
            JArg::S(_) => "Ljava/lang/String;",
            JArg::D(_) => "D",
            JArg::F(_) => "F",
            JArg::I(_) => "I",
            JArg::J(_) => "J",
            JArg::Z(_) => "Z",
            JArg::Bytes(_) => "[B",
        }
    }
}

/// A Java return type a bridge call can produce.
pub(crate) trait FromJava: Sized {
    const DESCRIPTOR: &'static str;
    const RETURN: ReturnType;
    fn from_java(env: &mut JNIEnv, value: JValueOwned) -> Option<Self>;
}

macro_rules! primitive_return {
    ($($ty:ty => $desc:literal, $prim:ident, $get:ident;)*) => {$(
        impl FromJava for $ty {
            const DESCRIPTOR: &'static str = $desc;
            const RETURN: ReturnType = ReturnType::Primitive(Primitive::$prim);
            fn from_java(_: &mut JNIEnv, value: JValueOwned) -> Option<Self> {
                value.$get().ok()
            }
        }
    )*};
}

primitive_return! {
    () => "V", Void, v;
    bool => "Z", Boolean, z;
    i32 => "I", Int, i;
    i64 => "J", Long, j;
    f32 => "F", Float, f;
    f64 => "D", Double, d;
}

/// A Java `String`; `null` maps to `None`.
impl FromJava for String {
    const DESCRIPTOR: &'static str = "Ljava/lang/String;";
    const RETURN: ReturnType = ReturnType::Object;
    fn from_java(env: &mut JNIEnv, value: JValueOwned) -> Option<Self> {
        let obj = value.l().ok()?;
        if obj.as_raw().is_null() {
            return None;
        }
        let s = JString::from(obj);
        let out = env.get_string(&s).ok().map(String::from);
        let _ = env.delete_local_ref(s);
        out
    }
}

/// A static method of a host class, resolved on first use.
pub(crate) struct StaticMethod {
    class: &'static str,
    name: &'static str,
    resolved: OnceLock<(GlobalRef, JStaticMethodID)>,
}

impl StaticMethod {
    pub(crate) const fn new(class: &'static str, name: &'static str) -> Self {
        Self {
            class,
            name,
            resolved: OnceLock::new(),
        }
    }

    /// Failures are not cached: a lookup can fail on a thread whose class
    /// loader doesn't see the host classes and succeed on the next one.
    fn resolve<R: FromJava>(
        &self,
        env: &mut JNIEnv,
        args: &[JArg],
    ) -> Option<&(GlobalRef, JStaticMethodID)> {
        if let Some(r) = self.resolved.get() {
            return Some(r);
        }
        let mut sig = String::from("(");
        for a in args {
            sig.push_str(a.descriptor());
        }
        sig.push(')');
        sig.push_str(R::DESCRIPTOR);
        let class = env.find_class(self.class).ok()?;
        let id = env.get_static_method_id(&class, self.name, &sig).ok()?;
        let global = env.new_global_ref(&class).ok()?;
        let _ = env.delete_local_ref(class);
        Some(self.resolved.get_or_init(|| (global, id)))
    }

    /// Calls the method; `None` if it can't be resolved, an argument can't
    /// be converted, or it threw (the exception is cleared).
    ///
    /// # Safety
    /// Every `JArg::S` must hold valid UTF-8, and a call site must always
    /// pass the same argument kinds and expect the same `R` (the resolved
    /// signature is cached per site).
    pub(crate) unsafe fn call<R: FromJava>(&self, env: &mut JNIEnv, args: &[JArg]) -> Option<R> {
        unsafe {
            assert!(args.len() <= MAX_ARGS, "{}: too many arguments", self.name);
            let result = self.invoke::<R>(env, args);
            if result.is_none() && env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
            result
        }
    }

    unsafe fn invoke<R: FromJava>(&self, env: &mut JNIEnv, args: &[JArg]) -> Option<R> {
        unsafe {
            let (class, id) = self.resolve::<R>(env, args)?;
            let mut values = [jvalue { j: 0 }; MAX_ARGS];
            let mut locals: [Option<JObject>; MAX_ARGS] = Default::default();
            let mut converted = true;
            for (i, a) in args.iter().enumerate() {
                values[i] = match *a {
                    JArg::S(s) => match env.new_string(s.as_str()) {
                        Ok(js) => {
                            let raw = js.as_raw();
                            locals[i] = Some(js.into());
                            jvalue { l: raw }
                        }
                        Err(_) => {
                            converted = false;
                            break;
                        }
                    },
                    JArg::Bytes(b) => match env.byte_array_from_slice(b) {
                        Ok(arr) => {
                            let raw = arr.as_raw();
                            locals[i] = Some(arr.into());
                            jvalue { l: raw }
                        }
                        Err(_) => {
                            converted = false;
                            break;
                        }
                    },
                    JArg::D(d) => jvalue { d },
                    JArg::F(f) => jvalue { f },
                    JArg::I(i) => jvalue { i },
                    JArg::J(j) => jvalue { j },
                    JArg::Z(z) => jvalue { z: z as u8 },
                };
            }
            let result = if converted {
                let class = <&JClass>::from(class.as_obj());
                env.call_static_method_unchecked(class, *id, R::RETURN, &values[..args.len()])
                    .ok()
                    .and_then(|v| R::from_java(env, v))
            } else {
                None
            };
            // Free the argument copies now: a render pass can make thousands of
            // these calls inside one native frame.
            for obj in locals.into_iter().flatten() {
                let _ = env.delete_local_ref(obj);
            }
            result
        }
    }
}

/// Calls `<class>.<name>(args...)` on `env`, returning `Option<R>` (`R`
/// inferred: `()`, `bool`, `i32`, `i64`, `f32`, `f64` or `String`).
///
/// `S`, `D`, `F`, `I`, `J`, `Z` and `Bytes` ([`JArg`]) are in scope for the
/// arguments.
macro_rules! jcall {
    ($env:expr_2021, $class:literal, $name:literal, [$($arg:expr_2021),* $(,)?]) => {{
        static METHOD: $crate::jcall::StaticMethod =
            $crate::jcall::StaticMethod::new($class, $name);
        #[allow(unused_imports)]
        use $crate::jcall::JArg::*;
        METHOD.call($env, &[$($arg),*])
    }};
}
pub(crate) use jcall;

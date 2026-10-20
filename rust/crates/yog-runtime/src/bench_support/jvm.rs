//! The bus with a real JVM, for `benches/bus_calls.rs`: the natives bound,
//! the router reached as the host reaches it.

pub use crate::bus::jvm_support::{call, provide_native, register_natives, resolve};
pub use crate::test_jvm::{javac, jdk, start};

/// Hands the runtime the JVM, as the host's `JNI_OnLoad` does.
pub fn adopt(vm: &jni::JavaVM) {
    // The JVM lives as long as the process; this is the same one.
    let _ = crate::globals::JAVA_VM
        .set(unsafe { jni::JavaVM::from_raw(vm.get_java_vm_pointer()) }.expect("the JVM"));
}

//! A JVM in this process, for the tests and benches that need the host's
//! real Java: sources compiled with the JDK's `javac`, the JVM loaded from
//! its `libjvm` (one a process — JNI allows no second). Self-contained
//! (`jni`, `libloading`) so a bench can include it as-is.

use std::ffi::{CString, c_void};
use std::path::{Path, PathBuf};
use std::process::Command;

use jni::JavaVM;
use jni::sys::{
    JNI_OK, JNI_VERSION_1_8, JNIEnv as RawEnv, JavaVM as RawVm, JavaVMInitArgs, JavaVMOption, jint,
};

/// The JDK when `JAVA_HOME` names none.
const DEFAULT_JDK: &str = "/usr/lib/jvm/java-21-openjdk-amd64";

/// The JDK to use, if there's one.
pub fn jdk() -> Option<PathBuf> {
    let home = std::env::var_os("JAVA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_JDK));
    home.join("bin/javac").is_file().then_some(home)
}

/// Compiles `sources` into `out`; whether javac succeeded.
pub fn javac(jdk: &Path, out: &Path, sources: &[PathBuf]) -> bool {
    Command::new(jdk.join("bin/javac"))
        .arg("-d")
        .arg(out)
        .args(sources)
        .status()
        .is_ok_and(|s| s.success())
}

/// Starts the JVM from `jdk`'s `libjvm` with `classpath` (the library is leaked: the JVM lives as long as the process).
pub fn start(jdk: &Path, classpath: &Path) -> JavaVM {
    let lib = Box::leak(Box::new(
        unsafe { libloading::Library::new(jdk.join("lib/server/libjvm.so")) }.expect("load libjvm"),
    ));
    type CreateVm =
        unsafe extern "system" fn(*mut *mut RawVm, *mut *mut c_void, *mut c_void) -> jint;
    let create: libloading::Symbol<CreateVm> =
        unsafe { lib.get(b"JNI_CreateJavaVM\0") }.expect("JNI_CreateJavaVM");
    let cp = CString::new(format!("-Djava.class.path={}", classpath.display()))
        .expect("a class path without NUL");
    let mut options = [JavaVMOption {
        optionString: cp.as_ptr() as *mut _,
        extraInfo: std::ptr::null_mut(),
    }];
    let mut args = JavaVMInitArgs {
        version: JNI_VERSION_1_8,
        nOptions: options.len() as jint,
        options: options.as_mut_ptr(),
        ignoreUnrecognized: 0,
    };
    let mut vm: *mut RawVm = std::ptr::null_mut();
    let mut env: *mut RawEnv = std::ptr::null_mut();
    let rc = unsafe {
        create(
            &mut vm,
            (&mut env as *mut *mut RawEnv).cast(),
            (&mut args as *mut JavaVMInitArgs).cast(),
        )
    };
    assert_eq!(rc, JNI_OK, "JNI_CreateJavaVM failed");
    unsafe { JavaVM::from_raw(vm) }.expect("the new JVM")
}

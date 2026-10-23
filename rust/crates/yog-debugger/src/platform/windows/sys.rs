//! The Windows calls the backend makes, each wrapped once: a thread's
//! context, the process's memory, a file's path, a thread's name.

use std::ffi::c_void;
use std::path::PathBuf;

use windows_sys::Win32::Foundation::{HANDLE, LocalFree};
use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
use windows_sys::Win32::System::Diagnostics::Debug::{
    CONTEXT, CONTEXT_FULL_AMD64, FlushInstructionCache, GetThreadContext, ReadProcessMemory,
    SetThreadContext, WriteProcessMemory,
};
use windows_sys::Win32::System::Memory::{PAGE_EXECUTE_READWRITE, VirtualProtectEx};
use windows_sys::Win32::System::Threading::GetThreadDescription;

use crate::target::OsError;

/// The longest path asked of `GetFinalPathNameByHandleW`.
const MAX_PATH_CHARS: usize = 32_768;
/// What `GetFinalPathNameByHandleW` prefixes a local path with.
const VERBATIM: &str = r"\\?\";

/// The system's last error, as `what` failing.
pub(super) fn last_error(what: &'static str) -> OsError {
    OsError {
        what,
        message: std::io::Error::last_os_error().to_string(),
        gone: false,
    }
}

/// A thread's full context (the registers).
pub(super) fn context(thread: HANDLE) -> Result<Box<CONTEXT>, OsError> {
    // SAFETY: a zeroed CONTEXT is valid; the box keeps its 16-byte alignment.
    let mut ctx: Box<CONTEXT> = Box::new(unsafe { std::mem::zeroed() });
    ctx.ContextFlags = CONTEXT_FULL_AMD64;
    // SAFETY: `thread` is a live handle; `ctx` is a CONTEXT asking for FULL.
    if unsafe { GetThreadContext(thread, &mut *ctx) } == 0 {
        return Err(last_error("reading a thread's registers"));
    }
    Ok(ctx)
}

pub(super) fn set_context(thread: HANDLE, ctx: &CONTEXT) -> Result<(), OsError> {
    // SAFETY: `thread` is a live, suspended handle; `ctx` came from `context`.
    if unsafe { SetThreadContext(thread, ctx) } == 0 {
        return Err(last_error("setting a thread's registers"));
    }
    Ok(())
}

/// Fills `buf` from the process's `addr`; whether all of it was readable.
pub(super) fn read(process: HANDLE, addr: u64, buf: &mut [u8]) -> bool {
    let mut done = 0usize;
    // SAFETY: `buf` is writable for its length; the process handle is live.
    let ok = unsafe {
        ReadProcessMemory(
            process,
            addr as *const c_void,
            buf.as_mut_ptr().cast(),
            buf.len(),
            &mut done,
        )
    };
    ok != 0 && done == buf.len()
}

/// Writes `bytes` at the process's `addr` — code too: its page made
/// writable for the write, then put back, the instruction cache flushed.
pub(super) fn write(process: HANDLE, addr: u64, bytes: &[u8]) -> Result<(), OsError> {
    let at = addr as *const c_void;
    let mut old = 0;
    // SAFETY: the process handle is live; `at` is in its address space.
    let unprotected =
        unsafe { VirtualProtectEx(process, at, bytes.len(), PAGE_EXECUTE_READWRITE, &mut old) } != 0;
    let mut done = 0usize;
    // SAFETY: `bytes` is readable for its length.
    let ok = unsafe { WriteProcessMemory(process, at, bytes.as_ptr().cast(), bytes.len(), &mut done) };
    let failed = (ok == 0 || done != bytes.len()).then(|| last_error("writing its memory"));
    if unprotected {
        let mut ignored = 0;
        // SAFETY: as above; restores the protection the page had.
        unsafe { VirtualProtectEx(process, at, bytes.len(), old, &mut ignored) };
    }
    // SAFETY: as above.
    unsafe { FlushInstructionCache(process, at, bytes.len()) };
    failed.map_or(Ok(()), Err)
}

/// The path of the file `file` is a handle to.
pub(super) fn path_of(file: HANDLE) -> Option<PathBuf> {
    let mut buf = vec![0u16; MAX_PATH_CHARS];
    // SAFETY: `buf` is writable for the length passed.
    let n = unsafe { GetFinalPathNameByHandleW(file, buf.as_mut_ptr(), buf.len() as u32, 0) };
    let n = n as usize;
    if n == 0 || n >= buf.len() {
        return None;
    }
    let path = String::from_utf16_lossy(&buf[..n]);
    Some(PathBuf::from(path.strip_prefix(VERBATIM).unwrap_or(&path)))
}

/// The name a thread gave itself (`SetThreadDescription`); empty if none.
pub(super) fn description(thread: HANDLE) -> String {
    let mut text: *mut u16 = std::ptr::null_mut();
    // SAFETY: `thread` is a live handle; the call fills `text` on success.
    if unsafe { GetThreadDescription(thread, &mut text) } < 0 || text.is_null() {
        return String::new();
    }
    // SAFETY: a NUL-terminated wide string the system allocated.
    let len = (0..).take_while(|&i| unsafe { *text.add(i) } != 0).count();
    // SAFETY: as above, `len` characters long.
    let name = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) });
    // SAFETY: the system allocated it with LocalAlloc.
    unsafe { LocalFree(text.cast()) };
    name
}

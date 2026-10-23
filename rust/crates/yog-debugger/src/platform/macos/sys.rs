//! The Mach calls the backend makes, each wrapped once: the task and its
//! threads, a thread's identity and name, the task's memory.

use mach2::kern_return::{
    KERN_INVALID_ARGUMENT, KERN_INVALID_NAME, KERN_INVALID_RIGHT, KERN_INVALID_TASK, KERN_SUCCESS,
    KERN_TERMINATED, kern_return_t,
};
use mach2::mach_port::mach_port_deallocate;
use mach2::mach_types::{task_t, thread_act_array_t, thread_act_t};
use mach2::message::{MACH_SEND_INVALID_DEST, mach_msg_type_number_t};
use mach2::port::{MACH_PORT_NULL, mach_port_name_t, mach_port_t};
use mach2::task::task_threads;
use mach2::traps::{mach_task_self, task_for_pid};
use mach2::vm::{
    mach_vm_deallocate, mach_vm_protect, mach_vm_read_overwrite, mach_vm_region, mach_vm_write,
};
use mach2::vm_prot::{VM_PROT_COPY, VM_PROT_READ, VM_PROT_WRITE};
use mach2::vm_region::{
    VM_REGION_BASIC_INFO_64, VM_REGION_BASIC_INFO_COUNT_64, vm_region_basic_info_64,
};
use mach2::vm_types::{mach_vm_address_t, mach_vm_size_t};

use crate::target::{OsError, Tid};

unsafe extern "C" {
    /// Not in `mach2`: puts a receive right into a port set.
    fn mach_port_insert_member(
        task: mach_port_t,
        name: mach_port_name_t,
        pset: mach_port_name_t,
    ) -> kern_return_t;
}

/// Errors that mean the thread (or the whole task) isn't there any more.
const GONE: [kern_return_t; 6] = [
    KERN_INVALID_ARGUMENT,
    KERN_INVALID_NAME,
    KERN_INVALID_RIGHT,
    KERN_INVALID_TASK,
    KERN_TERMINATED,
    MACH_SEND_INVALID_DEST,
];

/// `kr` as `what` failing; `Ok` when it succeeded.
pub(super) fn check(what: &'static str, kr: kern_return_t) -> Result<(), OsError> {
    if kr == KERN_SUCCESS {
        return Ok(());
    }
    Err(OsError {
        what,
        message: format!("kern_return_t {kr}"),
        gone: GONE.contains(&kr),
    })
}

pub(super) fn this_task() -> task_t {
    // SAFETY: reads the process's own task port, set up before main.
    unsafe { mach_task_self() }
}

/// Drops our reference on a port name.
pub(super) fn release(name: mach_port_t) {
    // SAFETY: a name in our own space; a stale one is merely refused.
    unsafe { mach_port_deallocate(this_task(), name) };
}

/// The task of process `pid` — needs the debugger entitlement, or root,
/// and a target that allows it (`get-task-allow`).
pub(super) fn task_of(pid: u32) -> Result<task_t, OsError> {
    let mut task: mach_port_name_t = MACH_PORT_NULL;
    // SAFETY: plain call filling `task`.
    check("getting its task (task_for_pid)", unsafe {
        task_for_pid(this_task(), pid as i32, &mut task)
    })?;
    Ok(task)
}

pub(super) fn insert_member(port: mach_port_t, set: mach_port_t) -> Result<(), OsError> {
    // SAFETY: names of rights we hold.
    check("listening on a port", unsafe {
        mach_port_insert_member(this_task(), port, set)
    })
}

/// The task's threads, as port names (each with a reference of ours).
pub(super) fn threads(task: task_t) -> Result<Vec<thread_act_t>, OsError> {
    let mut list: thread_act_array_t = std::ptr::null_mut();
    let mut count: mach_msg_type_number_t = 0;
    // SAFETY: the call allocates `list` in our space; copied out, then freed.
    check("listing its threads", unsafe {
        task_threads(task, &mut list, &mut count)
    })?;
    // SAFETY: `count` names, as the call reported.
    let names = unsafe { std::slice::from_raw_parts(list, count as usize) }.to_vec();
    let bytes = (count as usize * std::mem::size_of::<thread_act_t>()) as mach_vm_size_t;
    // SAFETY: the array the call allocated, of that size.
    unsafe { mach_vm_deallocate(this_task(), list as mach_vm_address_t, bytes) };
    Ok(names)
}

/// The thread's system-wide id (what `Tid` is on macOS).
pub(super) fn thread_id(thread: thread_act_t) -> Result<Tid, OsError> {
    // SAFETY: a zeroed info struct is valid for the call to fill.
    let mut info: libc::thread_identifier_info = unsafe { std::mem::zeroed() };
    let mut count = libc::THREAD_IDENTIFIER_INFO_COUNT;
    // SAFETY: `info` is the flavor's struct, `count` its size in words.
    check("identifying a thread", unsafe {
        libc::thread_info(
            thread,
            libc::THREAD_IDENTIFIER_INFO as u32,
            (&mut info as *mut libc::thread_identifier_info).cast(),
            &mut count,
        )
    })?;
    Ok(info.thread_id)
}

/// The name a thread gave itself (`pthread_setname_np`); empty if none.
pub(super) fn thread_name(thread: thread_act_t) -> String {
    // SAFETY: as in `thread_id`.
    let mut info: libc::thread_extended_info = unsafe { std::mem::zeroed() };
    let mut count = libc::THREAD_EXTENDED_INFO_COUNT;
    // SAFETY: as in `thread_id`.
    let kr = unsafe {
        libc::thread_info(
            thread,
            libc::THREAD_EXTENDED_INFO as u32,
            (&mut info as *mut libc::thread_extended_info).cast(),
            &mut count,
        )
    };
    if kr != KERN_SUCCESS {
        return String::new();
    }
    let bytes: Vec<u8> = info
        .pth_name
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Fills `buf` from the task's `addr`; whether all of it was readable.
pub(super) fn read(task: task_t, addr: u64, buf: &mut [u8]) -> bool {
    let mut done: mach_vm_size_t = 0;
    // SAFETY: `buf` is writable for its length.
    let kr = unsafe {
        mach_vm_read_overwrite(
            task,
            addr,
            buf.len() as mach_vm_size_t,
            buf.as_mut_ptr() as mach_vm_address_t,
            &mut done,
        )
    };
    kr == KERN_SUCCESS && done == buf.len() as mach_vm_size_t
}

/// The protection of the region `addr` is in.
fn protection(task: task_t, addr: u64) -> Option<i32> {
    let (mut at, mut size) = (addr, 0);
    // SAFETY: a zeroed info struct is valid for the call to fill.
    let mut info: vm_region_basic_info_64 = unsafe { std::mem::zeroed() };
    let mut count = VM_REGION_BASIC_INFO_COUNT_64;
    let mut object: mach_port_t = MACH_PORT_NULL;
    // SAFETY: `info` is the flavor's struct, `count` its size in words.
    let kr = unsafe {
        mach_vm_region(
            task,
            &mut at,
            &mut size,
            VM_REGION_BASIC_INFO_64,
            (&mut info as *mut vm_region_basic_info_64).cast(),
            &mut count,
            &mut object,
        )
    };
    (kr == KERN_SUCCESS && at <= addr).then_some(info.protection)
}

/// Writes `bytes` at the task's `addr` — code too: its page made writable
/// (a private copy) for the write, then given its protection back.
pub(super) fn write(task: task_t, addr: u64, bytes: &[u8]) -> Result<(), OsError> {
    let len = bytes.len() as mach_vm_size_t;
    let old = protection(task, addr);
    // SAFETY: plain calls on the task's memory; `bytes` readable for its length.
    unsafe {
        mach_vm_protect(
            task,
            addr,
            len,
            0,
            VM_PROT_READ | VM_PROT_WRITE | VM_PROT_COPY,
        );
        let written = check(
            "writing its memory",
            mach_vm_write(task, addr, bytes.as_ptr() as usize, bytes.len() as u32),
        );
        if let Some(old) = old {
            mach_vm_protect(task, addr, len, 0, old);
        }
        written
    }
}

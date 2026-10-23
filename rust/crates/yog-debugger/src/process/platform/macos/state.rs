//! A thread's machine state through `thread_get_state`/`thread_set_state`:
//! its registers, its pc set, and single-stepping turned on and off (the
//! trap flag on x86-64, `MDSCR_EL1.SS` in the debug state on AArch64).

use mach2::mach_types::thread_act_t;
use mach2::message::mach_msg_type_number_t;
use mach2::thread_act::{thread_get_state, thread_set_state};
use mach2::thread_status::thread_state_flavor_t;
use yog_symbols::FrameRegs;

use super::sys::check;
use crate::process::target::{OsError, Regs};

/// A state struct's size in the 32-bit words the calls count in.
fn words<T>() -> mach_msg_type_number_t {
    (std::mem::size_of::<T>() / std::mem::size_of::<u32>()) as mach_msg_type_number_t
}

/// The thread's state of `flavor` (`T` its struct).
fn get<T>(thread: thread_act_t, flavor: thread_state_flavor_t) -> Result<T, OsError> {
    // SAFETY: the state structs are plain integers; zero is valid.
    let mut state: T = unsafe { std::mem::zeroed() };
    let mut count = words::<T>();
    // SAFETY: `state` is the flavor's struct, `count` its size in words.
    check("reading a thread's registers", unsafe {
        thread_get_state(thread, flavor, (&mut state as *mut T).cast(), &mut count)
    })?;
    Ok(state)
}

fn set<T>(
    thread: thread_act_t,
    flavor: thread_state_flavor_t,
    mut state: T,
) -> Result<(), OsError> {
    // SAFETY: as in `get`.
    check("setting a thread's registers", unsafe {
        thread_set_state(thread, flavor, (&mut state as *mut T).cast(), words::<T>())
    })
}

#[cfg(target_arch = "x86_64")]
mod machine {
    use mach2::structs::x86_thread_state64_t;
    use mach2::thread_status::x86_THREAD_STATE64;

    use super::{FrameRegs, OsError, Regs, get, set, thread_act_t};
    use crate::process::arch::TRAP_FLAG;

    fn general(thread: thread_act_t) -> Result<x86_thread_state64_t, OsError> {
        get(thread, x86_THREAD_STATE64)
    }

    pub(in super::super) fn regs(thread: thread_act_t) -> Result<Regs, OsError> {
        let r = general(thread)?;
        Ok(Regs {
            frame: FrameRegs::x86_64([
                r.__rax, r.__rdx, r.__rcx, r.__rbx, r.__rsi, r.__rdi, r.__rbp, r.__rsp, r.__r8,
                r.__r9, r.__r10, r.__r11, r.__r12, r.__r13, r.__r14, r.__r15, r.__rip,
            ]),
            named: vec![
                ("rip", r.__rip),
                ("rsp", r.__rsp),
                ("rbp", r.__rbp),
                ("rax", r.__rax),
                ("rbx", r.__rbx),
                ("rcx", r.__rcx),
                ("rdx", r.__rdx),
                ("rsi", r.__rsi),
                ("rdi", r.__rdi),
                ("r8", r.__r8),
                ("r9", r.__r9),
                ("r10", r.__r10),
                ("r11", r.__r11),
                ("r12", r.__r12),
                ("r13", r.__r13),
                ("r14", r.__r14),
                ("r15", r.__r15),
                ("rflags", r.__rflags),
            ],
        })
    }

    pub(in super::super) fn set_pc(thread: thread_act_t, pc: u64) -> Result<(), OsError> {
        let mut r = general(thread)?;
        r.__rip = pc;
        set(thread, x86_THREAD_STATE64, r)
    }

    /// Single-stepping on or off: the trap flag.
    pub(in super::super) fn set_stepping(thread: thread_act_t, on: bool) -> Result<(), OsError> {
        let mut r = general(thread)?;
        r.__rflags = if on {
            r.__rflags | TRAP_FLAG
        } else {
            r.__rflags & !TRAP_FLAG
        };
        set(thread, x86_THREAD_STATE64, r)
    }
}

#[cfg(target_arch = "aarch64")]
mod machine {
    use mach2::structs::arm_thread_state64_t;
    use mach2::thread_status::{ARM_DEBUG_STATE64, ARM_THREAD_STATE64};
    use yog_symbols::aarch64::GENERAL;

    use super::{FrameRegs, OsError, Regs, get, set, thread_act_t};

    /// Hardware break- and watchpoints.
    const SLOTS: usize = 16;
    /// `MDSCR_EL1.SS`: one instruction, then a software-step exception.
    const SINGLE_STEP: u64 = 1;

    /// `arm_debug_state64_t` (not in `mach2`).
    #[repr(C)]
    struct DebugState {
        bvr: [u64; SLOTS],
        bcr: [u64; SLOTS],
        wvr: [u64; SLOTS],
        wcr: [u64; SLOTS],
        mdscr_el1: u64,
    }

    fn general(thread: thread_act_t) -> Result<arm_thread_state64_t, OsError> {
        get(thread, ARM_THREAD_STATE64)
    }

    pub(in super::super) fn regs(thread: thread_act_t) -> Result<Regs, OsError> {
        let r = general(thread)?;
        let mut x = [0u64; GENERAL];
        x[..r.__x.len()].copy_from_slice(&r.__x);
        x[r.__x.len()] = r.__fp;
        x[r.__x.len() + 1] = r.__lr;
        Ok(Regs {
            frame: FrameRegs::aarch64(x, r.__sp, r.__pc),
            named: crate::process::platform::aarch64_named(&x, r.__sp, r.__pc, u64::from(r.__cpsr)),
        })
    }

    pub(in super::super) fn set_pc(thread: thread_act_t, pc: u64) -> Result<(), OsError> {
        let mut r = general(thread)?;
        r.__pc = pc;
        set(thread, ARM_THREAD_STATE64, r)
    }

    /// Single-stepping on or off: the software-step bit.
    pub(in super::super) fn set_stepping(thread: thread_act_t, on: bool) -> Result<(), OsError> {
        let mut d: DebugState = get(thread, ARM_DEBUG_STATE64)?;
        d.mdscr_el1 = if on {
            d.mdscr_el1 | SINGLE_STEP
        } else {
            d.mdscr_el1 & !SINGLE_STEP
        };
        set(thread, ARM_DEBUG_STATE64, d)
    }
}

pub(super) use machine::*;

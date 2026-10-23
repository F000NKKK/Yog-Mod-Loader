//! A stopped thread's registers through `PTRACE_GETREGS` (x86-64) /
//! `PTRACE_GETREGSET` (AArch64, which `nix` hides behind the same call).

use nix::sys::ptrace;
use nix::unistd::Pid;
use yog_symbols::FrameRegs;

use crate::target::Regs;

#[cfg(target_arch = "x86_64")]
pub(super) fn of(tid: Pid) -> nix::Result<Regs> {
    let r = ptrace::getregs(tid)?;
    Ok(Regs {
        frame: FrameRegs::x86_64([
            r.rax, r.rdx, r.rcx, r.rbx, r.rsi, r.rdi, r.rbp, r.rsp, r.r8, r.r9, r.r10, r.r11,
            r.r12, r.r13, r.r14, r.r15, r.rip,
        ]),
        named: vec![
            ("rip", r.rip),
            ("rsp", r.rsp),
            ("rbp", r.rbp),
            ("rax", r.rax),
            ("rbx", r.rbx),
            ("rcx", r.rcx),
            ("rdx", r.rdx),
            ("rsi", r.rsi),
            ("rdi", r.rdi),
            ("r8", r.r8),
            ("r9", r.r9),
            ("r10", r.r10),
            ("r11", r.r11),
            ("r12", r.r12),
            ("r13", r.r13),
            ("r14", r.r14),
            ("r15", r.r15),
            ("eflags", r.eflags),
        ],
    })
}

#[cfg(target_arch = "x86_64")]
pub(super) fn set_pc(tid: Pid, pc: u64) -> nix::Result<()> {
    let mut r = ptrace::getregs(tid)?;
    r.rip = pc;
    ptrace::setregs(tid, r)
}

#[cfg(target_arch = "aarch64")]
pub(super) fn of(tid: Pid) -> nix::Result<Regs> {
    let r = ptrace::getregs(tid)?;
    Ok(Regs {
        frame: FrameRegs::aarch64(r.regs, r.sp, r.pc),
        named: crate::platform::aarch64_named(&r.regs, r.sp, r.pc, r.pstate),
    })
}

#[cfg(target_arch = "aarch64")]
pub(super) fn set_pc(tid: Pid, pc: u64) -> nix::Result<()> {
    let mut r = ptrace::getregs(tid)?;
    r.pc = pc;
    ptrace::setregs(tid, r)
}

//! The debug events Windows reports, made into [`Event`]s. While one is
//! being looked at, the whole process waits; it's let go (continued)
//! before the next — and a thread that trapped is suspended first, so only
//! it stays stopped (non-stop, as on Linux).

use windows_sys::Win32::Foundation::{
    CloseHandle, DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED, EXCEPTION_BREAKPOINT,
    EXCEPTION_SINGLE_STEP, NTSTATUS,
};
use windows_sys::Win32::System::Diagnostics::Debug::{
    CREATE_PROCESS_DEBUG_EVENT, CREATE_THREAD_DEBUG_EVENT, ContinueDebugEvent, DEBUG_EVENT,
    EXCEPTION_DEBUG_EVENT, EXCEPTION_RECORD, EXIT_PROCESS_DEBUG_EVENT, EXIT_THREAD_DEBUG_EVENT,
    LOAD_DLL_DEBUG_EVENT, UNLOAD_DLL_DEBUG_EVENT, WaitForDebugEvent,
};
use windows_sys::Win32::System::Threading::INFINITE;

use super::{Process, sys};
use crate::process::arch::TRAP_ADVANCE;
use crate::process::target::{Event, Module, OsError, Tid};

/// The exception a program raises to name a thread for its debugger
/// (`THREADNAME_INFO`: the name's address, then the thread's id).
const MS_VC_EXCEPTION: NTSTATUS = 0x406D_1388_u32 as NTSTATUS;
const NAME_ARG: usize = 1;
const THREAD_ARG: usize = 2;
/// `-1` in `THREADNAME_INFO`: the raising thread.
const RAISING_THREAD: usize = usize::MAX;
/// The longest thread name read.
const MAX_NAME: usize = 64;

impl Process {
    /// Lets the event the process waits on go, as `how` decided.
    pub(super) fn release(&mut self) -> Result<(), OsError> {
        if let Some((tid, how)) = self.frozen.take() {
            // SAFETY: plain ids of the event being continued.
            if unsafe { ContinueDebugEvent(self.pid, tid as u32, how) } == 0 {
                return Err(sys::last_error("continuing the process"));
            }
        }
        Ok(())
    }

    /// The next debug event, as an [`Event`] when it's one the debugger
    /// hears about (`None`: handled here — a module loaded, the process's
    /// own exception handed back to it).
    pub(super) fn pump(&mut self) -> Result<Option<Event>, OsError> {
        self.release()?;
        // SAFETY: a zeroed DEBUG_EVENT is valid for the call to fill.
        let mut ev: DEBUG_EVENT = unsafe { std::mem::zeroed() };
        // SAFETY: as above.
        if unsafe { WaitForDebugEvent(&mut ev, INFINITE) } == 0 {
            return Err(sys::last_error("waiting for the process"));
        }
        let tid = Tid::from(ev.dwThreadId);
        event_log!("debug event {} on {tid}", ev.dwDebugEventCode);
        self.frozen = Some((tid, DBG_CONTINUE));
        // SAFETY (each arm): the union member the event code names.
        Ok(match ev.dwDebugEventCode {
            CREATE_PROCESS_DEBUG_EVENT => {
                let info = unsafe { ev.u.CreateProcessInfo };
                self.process = info.hProcess;
                self.leader.get_or_insert(tid);
                self.threads.insert(tid, info.hThread);
                self.loaded(info.hFile, info.lpBaseOfImage as u64);
                Some(Event::Spawned {
                    child: tid,
                    parent: None,
                })
            }
            CREATE_THREAD_DEBUG_EVENT => {
                self.threads
                    .insert(tid, unsafe { ev.u.CreateThread.hThread });
                Some(Event::Spawned {
                    child: tid,
                    parent: None,
                })
            }
            EXIT_THREAD_DEBUG_EVENT => {
                self.forget(tid);
                Some(Event::Gone(tid))
            }
            EXIT_PROCESS_DEBUG_EVENT => {
                let code = unsafe { ev.u.ExitProcess.dwExitCode } as i32;
                // The process ends only once its exit is continued: a wait
                // on it (its parent's) would hang till then.
                self.release()?;
                Some(Event::Exited(code))
            }
            LOAD_DLL_DEBUG_EVENT => {
                let info = unsafe { ev.u.LoadDll };
                self.loaded(info.hFile, info.lpBaseOfDll as u64);
                None
            }
            UNLOAD_DLL_DEBUG_EVENT => {
                let base = unsafe { ev.u.UnloadDll.lpBaseOfDll } as u64;
                self.modules.retain(|m| m.base != base);
                None
            }
            EXCEPTION_DEBUG_EVENT => {
                self.exception(tid, &unsafe { ev.u.Exception }.ExceptionRecord)
            }
            _ => None,
        })
    }

    /// A module loaded at `base` from `file` (the handle is ours to close).
    fn loaded(&mut self, file: windows_sys::Win32::Foundation::HANDLE, base: u64) {
        if file.is_null() {
            return;
        }
        if let Some(path) = sys::path_of(file) {
            self.modules.push(Module { base, path });
        }
        // SAFETY: the debugger owns the file handle a load event hands it.
        unsafe { CloseHandle(file) };
    }

    /// An exception on `tid`: ours (a breakpoint of ours, a step's end, our
    /// interrupt, a thread's name) or the process's own, handed back to it.
    fn exception(&mut self, tid: Tid, rec: &EXCEPTION_RECORD) -> Option<Event> {
        let addr = rec.ExceptionAddress as u64;
        match rec.ExceptionCode {
            EXCEPTION_BREAKPOINT if self.claimed.contains(&addr) => {
                self.trapped_at(tid, addr);
                Some(Event::Trap(tid))
            }
            EXCEPTION_SINGLE_STEP if self.stepping.remove(&tid) => {
                self.hold(tid);
                Some(Event::Trap(tid))
            }
            EXCEPTION_BREAKPOINT if super::take_interrupt(self.pid) => {
                Some(Event::Interrupted(None))
            }
            // The breakpoint Windows raises right after attaching.
            EXCEPTION_BREAKPOINT if !self.attached => {
                self.attached = true;
                None
            }
            MS_VC_EXCEPTION => {
                self.named(tid, rec);
                None
            }
            _ => {
                self.frozen = Some((tid, DBG_EXCEPTION_NOT_HANDLED));
                None
            }
        }
    }

    /// `tid` hit our breakpoint at `addr`: held there, its pc just past the
    /// breakpoint (as every system's trap leaves it for the debugger).
    fn trapped_at(&mut self, tid: Tid, addr: u64) {
        self.hold(tid);
        let Some(&thread) = self.threads.get(&tid) else {
            return;
        };
        if let Ok(mut ctx) = sys::context(thread)
            && ctx.Rip == addr
        {
            ctx.Rip = addr + TRAP_ADVANCE;
            let _ = sys::set_context(thread, &ctx);
        }
    }

    /// A thread's name, as the program told its debugger.
    fn named(&mut self, raiser: Tid, rec: &EXCEPTION_RECORD) {
        let args = &rec.ExceptionInformation;
        let mut buf = [0u8; MAX_NAME];
        if !sys::read(self.process, args[NAME_ARG] as u64, &mut buf) {
            return;
        }
        let len = buf.iter().position(|&b| b == 0).unwrap_or(MAX_NAME);
        let tid = match args[THREAD_ARG] & u32::MAX as usize {
            id if id == RAISING_THREAD & u32::MAX as usize => raiser,
            id => id as Tid,
        };
        self.names
            .insert(tid, String::from_utf8_lossy(&buf[..len]).into_owned());
    }
}

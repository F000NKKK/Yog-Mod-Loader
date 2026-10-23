//! What the debugger hears from the task: Mach exception messages on a
//! port of ours (the task's breakpoint exceptions go there), interrupts
//! posted to another, both in one port set; and the process's end through
//! a kqueue.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

use mach2::exception_types::exception_type_t;
use mach2::kern_return::kern_return_t;
use mach2::mach_port::{mach_port_allocate, mach_port_destroy, mach_port_insert_right};
use mach2::message::{
    MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_MAKE_SEND, MACH_MSGH_BITS, MACH_MSGH_BITS_REMOTE_MASK,
    MACH_RCV_MSG, MACH_RCV_TIMED_OUT, MACH_RCV_TIMEOUT, MACH_SEND_MSG, MACH_SEND_TIMEOUT, mach_msg,
    mach_msg_destroy, mach_msg_header_t, mach_msg_id_t,
};
use mach2::ndr::{NDR_record, NDR_record_t};
use mach2::port::{MACH_PORT_NULL, MACH_PORT_RIGHT_PORT_SET, MACH_PORT_RIGHT_RECEIVE, mach_port_t};

use super::sys::{self, check};
use crate::target::{Event, OsError};

/// `mach_exception_raise` (64-bit codes), and its reply's id.
const EXCEPTION_RAISE: mach_msg_id_t = 2405;
const REPLY_OFFSET: mach_msg_id_t = 100;
/// Where an exception message has its thread's and task's port names and
/// its exception type (`#pragma pack(4)`).
const THREAD_AT: usize = 28;
const TASK_AT: usize = 40;
const EXCEPTION_AT: usize = 60;
/// Room for any message the set receives (an exception message and the
/// trailer the kernel appends).
const RECEIVE_SIZE: usize = 1024;
/// How long one receive waits before the caller looks around (new threads,
/// the process's end).
pub(super) const POLL_MS: u32 = 20;

/// Our receive rights: exceptions, interrupts, and the set holding both.
pub(super) struct Ports {
    pub(super) exceptions: mach_port_t,
    pub(super) interrupts: mach_port_t,
    set: mach_port_t,
}

/// What a receive got.
pub(super) enum Received {
    /// A thread raised a breakpoint exception and waits for our [`reply`].
    Exception(Exception),
    Interrupt,
    /// Nothing within [`POLL_MS`] (or a message that's nothing to us).
    Nothing,
}

pub(super) struct Exception {
    pub(super) thread: mach_port_t,
    pub(super) task: mach_port_t,
    pub(super) kind: exception_type_t,
    request: mach_msg_header_t,
}

#[repr(C, align(8))]
struct Buffer([u8; RECEIVE_SIZE]);

#[repr(C)]
struct Reply {
    header: mach_msg_header_t,
    ndr: NDR_record_t,
    ret: kern_return_t,
}

fn receive_right() -> Result<mach_port_t, OsError> {
    let mut port = MACH_PORT_NULL;
    // SAFETY: plain calls on our own space.
    unsafe {
        check(
            "making a port",
            mach_port_allocate(sys::this_task(), MACH_PORT_RIGHT_RECEIVE, &mut port),
        )?;
        check(
            "making a port",
            mach_port_insert_right(sys::this_task(), port, port, MACH_MSG_TYPE_MAKE_SEND),
        )?;
    }
    Ok(port)
}

fn read_i32(buf: &[u8], at: usize) -> i32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&buf[at..at + 4]);
    i32::from_ne_bytes(b)
}

impl Ports {
    pub(super) fn new() -> Result<Ports, OsError> {
        let (exceptions, interrupts) = (receive_right()?, receive_right()?);
        let mut set = MACH_PORT_NULL;
        // SAFETY: a plain call on our own space.
        check("making a port set", unsafe {
            mach_port_allocate(sys::this_task(), MACH_PORT_RIGHT_PORT_SET, &mut set)
        })?;
        sys::insert_member(exceptions, set)?;
        sys::insert_member(interrupts, set)?;
        Ok(Ports {
            exceptions,
            interrupts,
            set,
        })
    }

    /// The next message, waiting at most [`POLL_MS`].
    pub(super) fn receive(&self) -> Result<Received, OsError> {
        let mut buf = Buffer([0; RECEIVE_SIZE]);
        let header = buf.0.as_mut_ptr().cast::<mach_msg_header_t>();
        // SAFETY: `buf` is aligned and big enough for the header and more.
        let kr = unsafe {
            mach_msg(
                header,
                MACH_RCV_MSG | MACH_RCV_TIMEOUT,
                0,
                RECEIVE_SIZE as u32,
                self.set,
                POLL_MS,
                MACH_PORT_NULL,
            )
        };
        if kr == MACH_RCV_TIMED_OUT {
            return Ok(Received::Nothing);
        }
        check("receiving from the process", kr)?;
        // SAFETY: a message was received into `buf`, header first.
        let request = unsafe { *header };
        if request.msgh_local_port == self.interrupts {
            return Ok(Received::Interrupt);
        }
        if request.msgh_local_port != self.exceptions || request.msgh_id != EXCEPTION_RAISE {
            // SAFETY: the message just received; its rights are let go.
            unsafe { mach_msg_destroy(header) };
            return Ok(Received::Nothing);
        }
        let b = &buf.0;
        Ok(Received::Exception(Exception {
            thread: read_i32(b, THREAD_AT) as mach_port_t,
            task: read_i32(b, TASK_AT) as mach_port_t,
            kind: read_i32(b, EXCEPTION_AT),
            request,
        }))
    }

    /// Wakes a [`receive`](Self::receive) (on any thread): an interrupt.
    pub(super) fn post_interrupt(port: mach_port_t) {
        let mut header = mach_msg_header_t {
            msgh_bits: MACH_MSGH_BITS(MACH_MSG_TYPE_COPY_SEND, 0),
            msgh_size: std::mem::size_of::<mach_msg_header_t>() as u32,
            msgh_remote_port: port,
            msgh_local_port: MACH_PORT_NULL,
            msgh_voucher_port: MACH_PORT_NULL,
            msgh_id: 0,
        };
        // SAFETY: a plain header-only message; a full queue (an interrupt
        // already waiting) times out at once, which is fine.
        unsafe {
            mach_msg(
                &mut header,
                MACH_SEND_MSG | MACH_SEND_TIMEOUT,
                header.msgh_size,
                0,
                MACH_PORT_NULL,
                0,
                MACH_PORT_NULL,
            )
        };
    }

    pub(super) fn close(&self) {
        for port in [self.exceptions, self.interrupts, self.set] {
            // SAFETY: rights of ours, not used after this.
            unsafe { mach_port_destroy(sys::this_task(), port) };
        }
    }
}

impl Exception {
    /// Lets the excepting thread go: `handled`, it runs on (or stays
    /// suspended, if we suspended it); not, the exception goes on to the
    /// process (a `SIGTRAP`). The rights the message brought are let go.
    pub(super) fn reply(self, handled: bool) {
        let mut reply = Reply {
            header: mach_msg_header_t {
                msgh_bits: MACH_MSGH_BITS(self.request.msgh_bits & MACH_MSGH_BITS_REMOTE_MASK, 0),
                msgh_size: std::mem::size_of::<Reply>() as u32,
                msgh_remote_port: self.request.msgh_remote_port,
                msgh_local_port: MACH_PORT_NULL,
                msgh_voucher_port: MACH_PORT_NULL,
                msgh_id: self.request.msgh_id + REPLY_OFFSET,
            },
            // SAFETY: the system's constant NDR record.
            ndr: unsafe { NDR_record },
            ret: if handled {
                mach2::kern_return::KERN_SUCCESS
            } else {
                mach2::kern_return::KERN_FAILURE
            },
        };
        // SAFETY: a well-formed reply to the request's send-once right. It
        // fails only for a thread gone meanwhile: nothing to tell then.
        unsafe {
            mach_msg(
                &mut reply.header,
                MACH_SEND_MSG,
                reply.header.msgh_size,
                0,
                MACH_PORT_NULL,
                0,
                MACH_PORT_NULL,
            )
        };
        sys::release(self.thread);
        sys::release(self.task);
    }
}

/// The process's end, as a kqueue sees it: with its exit status when we're
/// its parent (the system tells only a parent), else just that it ended.
pub(super) struct ExitWatch {
    queue: OwnedFd,
}

impl ExitWatch {
    pub(super) fn new(pid: u32) -> Result<ExitWatch, OsError> {
        // SAFETY: a plain call; the descriptor is owned from here on.
        let fd = unsafe { libc::kqueue() };
        if fd < 0 {
            return Err(OsError {
                what: "watching for its end",
                message: std::io::Error::last_os_error().to_string(),
                gone: false,
                denied: false,
            });
        }
        // SAFETY: a fresh descriptor nothing else owns.
        let queue = unsafe { OwnedFd::from_raw_fd(fd) };
        let watch = ExitWatch { queue };
        if !watch.register(pid, libc::NOTE_EXIT | libc::NOTE_EXITSTATUS)
            && !watch.register(pid, libc::NOTE_EXIT)
        {
            return Err(OsError {
                what: "watching for its end",
                message: std::io::Error::last_os_error().to_string(),
                gone: true,
                denied: false,
            });
        }
        Ok(watch)
    }

    fn register(&self, pid: u32, flags: u32) -> bool {
        let change = libc::kevent {
            ident: pid as usize,
            filter: libc::EVFILT_PROC,
            flags: libc::EV_ADD | libc::EV_ONESHOT,
            fflags: flags,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        // SAFETY: one change, no events asked for.
        let n = unsafe {
            libc::kevent(
                self.queue.as_raw_fd(),
                &change,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };
        n >= 0
    }

    /// The process's end, if it has come.
    pub(super) fn poll(&self) -> Option<Event> {
        let now = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: a zeroed kevent is valid for the call to fill.
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        // SAFETY: room for one event.
        let n = unsafe {
            libc::kevent(
                self.queue.as_raw_fd(),
                std::ptr::null(),
                0,
                &mut event,
                1,
                &now,
            )
        };
        if n <= 0 {
            return None;
        }
        let status = event.data as i32;
        Some(match event.fflags & libc::NOTE_EXITSTATUS != 0 {
            false => Event::Exited(0),
            true if libc::WIFSIGNALED(status) => Event::Killed(libc::WTERMSIG(status)),
            true => Event::Exited(libc::WEXITSTATUS(status)),
        })
    }
}

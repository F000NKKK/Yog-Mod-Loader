//! macOS: Mach. The task's breakpoint exceptions come to a port of ours;
//! a thread that trapped is suspended before the exception is answered, so
//! it alone stays stopped while the rest run on — non-stop, as on Linux.
//! Breakpoint exceptions that aren't ours are answered "not handled": the
//! process gets them as the `SIGTRAP` it would have had. Mach reports no
//! thread creation or exit, so the thread list is looked at whenever the
//! wait has nothing else to do.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, VecDeque};

use mach2::exception_types::{
    EXC_BREAKPOINT, EXC_MASK_BREAKPOINT, EXCEPTION_DEFAULT, MACH_EXCEPTION_CODES,
    exception_behavior_t, exception_mask_t,
};
use mach2::mach_types::{task_t, thread_act_t};
use mach2::port::{MACH_PORT_NULL, mach_port_t};
use mach2::task::{task_get_exception_ports, task_set_exception_ports};
use mach2::thread_act::{thread_resume, thread_suspend};
use mach2::thread_status::{THREAD_STATE_NONE, thread_state_flavor_t};
use yog_sync::Mutex;

use crate::arch::TRAP_ADVANCE;
use crate::target::{Event, Module, OsError, Regs, Target, Tid};

use events::{ExitWatch, Ports, Received};
use sys::check;

mod events;
mod images;
mod state;
mod sys;

/// The most exception handlers a task can have (one per exception type).
const HANDLERS: usize = 32;

/// Each debugged process's interrupt port: [`interrupt`] posts there.
static INTERRUPTS: Mutex<Vec<(u32, mach_port_t)>> = Mutex::new(Vec::new());

/// Asks the debugger of process `pid` to stop and report
/// [`Event::Interrupted`] from its blocking wait. Callable from any thread.
pub fn interrupt(pid: u32) {
    let port = INTERRUPTS
        .lock()
        .iter()
        .find(|&&(p, _)| p == pid)
        .map(|&(_, port)| port);
    if let Some(port) = port {
        Ports::post_interrupt(port);
    }
}

/// Whether process `pid` has ended: a zombie, or not there at all.
pub fn process_ended(pid: u32) -> bool {
    // SAFETY: a zeroed info struct is valid for the call to fill.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
    // SAFETY: `info` is the flavor's struct, `size` its size.
    let filled = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    filled < size || info.pbi_status == libc::SZOMB
}

/// The task's breakpoint handlers before ours, put back on detaching.
struct Handlers {
    masks: Vec<exception_mask_t>,
    ports: Vec<mach_port_t>,
    behaviors: Vec<exception_behavior_t>,
    flavors: Vec<thread_state_flavor_t>,
}

pub struct Process {
    pid: u32,
    task: task_t,
    ports: Ports,
    exit: ExitWatch,
    /// The process's end, once seen: every later wait reports it.
    end: Option<Event>,
    previous: Handlers,
    /// The main thread: it stands for the process.
    leader: Tid,
    /// Thread ids and their ports (one reference of ours each).
    threads: HashMap<Tid, thread_act_t>,
    /// Events of other threads, kept while waiting on one.
    queue: VecDeque<Event>,
    /// Threads we hold stopped (suspended).
    held: HashSet<Tid>,
    /// Threads we single-stepped: their next breakpoint exception is ours.
    stepping: HashSet<Tid>,
    /// Addresses of our breakpoints (ever): a trap there is ours.
    claimed: HashSet<u64>,
}

/// Points `task`'s breakpoint exceptions at `port`; its handlers before.
fn take_exceptions(task: task_t, port: mach_port_t) -> Result<Handlers, OsError> {
    let mut masks = vec![0; HANDLERS];
    let mut ports = vec![MACH_PORT_NULL; HANDLERS];
    let mut behaviors = vec![0; HANDLERS];
    let mut flavors = vec![0; HANDLERS];
    let mut count = HANDLERS as u32;
    // SAFETY: four arrays of `count` entries for the call to fill, then
    // plain calls.
    unsafe {
        check(
            "reading its exception handlers",
            task_get_exception_ports(
                task,
                EXC_MASK_BREAKPOINT,
                masks.as_mut_ptr(),
                &mut count,
                ports.as_mut_ptr(),
                behaviors.as_mut_ptr(),
                flavors.as_mut_ptr(),
            ),
        )?;
        check(
            "taking its breakpoint exceptions",
            task_set_exception_ports(
                task,
                EXC_MASK_BREAKPOINT,
                port,
                (EXCEPTION_DEFAULT | MACH_EXCEPTION_CODES) as exception_behavior_t,
                THREAD_STATE_NONE,
            ),
        )?;
    }
    let n = count as usize;
    Ok(Handlers {
        masks: masks[..n].to_vec(),
        ports: ports[..n].to_vec(),
        behaviors: behaviors[..n].to_vec(),
        flavors: flavors[..n].to_vec(),
    })
}

impl Handlers {
    fn restore(&self, task: task_t) {
        for i in 0..self.masks.len() {
            // SAFETY: plain calls handing back what the task had.
            unsafe {
                task_set_exception_ports(
                    task,
                    self.masks[i],
                    self.ports[i],
                    self.behaviors[i],
                    self.flavors[i],
                )
            };
        }
    }
}

impl Process {
    fn thread(&self, tid: Tid) -> Result<thread_act_t, OsError> {
        self.threads.get(&tid).copied().ok_or(OsError {
            what: "finding a thread",
            message: format!("thread {tid} isn't there"),
            gone: true,
            denied: false,
        })
    }

    /// Suspends `tid`: it stays stopped while the process runs on.
    fn hold(&mut self, tid: Tid) -> Result<(), OsError> {
        let thread = self.thread(tid)?;
        if !self.held.contains(&tid) {
            // SAFETY: a thread port of ours.
            check("stopping a thread", unsafe { thread_suspend(thread) })?;
            self.held.insert(tid);
        }
        Ok(())
    }

    fn forget(&mut self, tid: Tid) {
        if let Some(thread) = self.threads.remove(&tid) {
            sys::release(thread);
        }
        self.held.remove(&tid);
        self.stepping.remove(&tid);
    }

    /// Looks at the task's threads: newcomers are reported spawned, threads
    /// no longer there gone.
    fn refresh(&mut self) {
        let Ok(names) = sys::threads(self.task) else {
            return;
        };
        let mut seen = HashSet::new();
        for name in names {
            let Ok(tid) = sys::thread_id(name) else {
                sys::release(name);
                continue;
            };
            seen.insert(tid);
            match self.threads.entry(tid) {
                Entry::Occupied(_) => sys::release(name),
                Entry::Vacant(slot) => {
                    slot.insert(name);
                    self.queue.push_back(Event::Spawned {
                        child: tid,
                        parent: None,
                    });
                }
            }
        }
        let gone: Vec<Tid> = self
            .threads
            .keys()
            .filter(|t| !seen.contains(t))
            .copied()
            .collect();
        for tid in gone {
            self.forget(tid);
            self.queue.push_back(Event::Gone(tid));
        }
    }

    /// A breakpoint exception: ours (a step's end, a breakpoint of ours) —
    /// the thread held, a trap — or the process's own, handed back.
    fn exception(&mut self, exception: events::Exception) -> Option<Event> {
        let thread = exception.thread;
        let tid = sys::thread_id(thread).ok();
        if tid.is_some_and(|t| !self.threads.contains_key(&t)) {
            self.refresh();
        }
        let ours = exception.kind == EXC_BREAKPOINT as i32
            && tid.is_some_and(|t| self.is_ours(t, thread))
            && tid.is_some_and(|t| self.hold(t).is_ok());
        exception.reply(ours);
        tid.filter(|_| ours).map(Event::Trap)
    }

    fn is_ours(&mut self, tid: Tid, thread: thread_act_t) -> bool {
        if self.stepping.remove(&tid) {
            let _ = state::set_stepping(thread, false);
            return true;
        }
        state::regs(thread).is_ok_and(|r| self.claimed.contains(&r.pc().wrapping_sub(TRAP_ADVANCE)))
    }

    /// Answers the exceptions still queued — a trap of ours put back on its
    /// breakpoint (its instruction restored by now), so it runs it for real.
    fn drain(&mut self) {
        while let Ok(Received::Exception(exception)) = self.ports.receive() {
            let ours = state::regs(exception.thread)
                .ok()
                .map(|r| r.pc().wrapping_sub(TRAP_ADVANCE))
                .filter(|pc| self.claimed.contains(pc));
            if let Some(pc) = ours {
                let _ = state::set_pc(exception.thread, pc);
            }
            exception.reply(ours.is_some());
        }
    }
}

impl Target for Process {
    fn attach(pid: u32) -> Result<(Self, Vec<Tid>), OsError> {
        let task = sys::task_of(pid)?;
        let ports = Ports::new()?;
        let exit = ExitWatch::new(pid)?;
        let previous = take_exceptions(task, ports.exceptions)?;
        INTERRUPTS.lock().push((pid, ports.interrupts));
        let mut process = Process {
            pid,
            task,
            ports,
            exit,
            end: None,
            previous,
            leader: 0,
            threads: HashMap::new(),
            queue: VecDeque::new(),
            held: HashSet::new(),
            stepping: HashSet::new(),
            claimed: HashSet::new(),
        };
        // The main thread comes first in the task's list.
        let names = sys::threads(task)?;
        let first = names.first().and_then(|&t| sys::thread_id(t).ok());
        names.into_iter().for_each(sys::release);
        process.refresh();
        process.queue.clear();
        process.leader = first.unwrap_or_default();
        let leader = process.leader;
        let mut tids: Vec<Tid> = process.threads.keys().copied().collect();
        tids.sort_by_key(|&t| (t != leader, t));
        Ok((process, tids))
    }

    fn pid(&self) -> u32 {
        self.pid
    }

    fn leader(&self) -> Tid {
        self.leader
    }

    fn thread_name(&self, tid: Tid) -> String {
        self.threads
            .get(&tid)
            .map(|&t| sys::thread_name(t))
            .unwrap_or_default()
    }

    fn stop(&mut self, tid: Tid) -> Result<(), OsError> {
        self.hold(tid)?;
        self.queue.push_back(Event::Stopped(tid));
        Ok(())
    }

    fn resume(&mut self, tid: Tid) -> Result<(), OsError> {
        let thread = self.thread(tid)?;
        if self.held.remove(&tid) {
            // SAFETY: a thread port of ours, suspended by us.
            check("resuming a thread", unsafe { thread_resume(thread) })?;
        }
        Ok(())
    }

    fn step(&mut self, tid: Tid) -> Result<(), OsError> {
        state::set_stepping(self.thread(tid)?, true)?;
        self.stepping.insert(tid);
        self.resume(tid)
    }

    fn wait(&mut self, only: Option<Tid>) -> Result<Event, OsError> {
        loop {
            if let Some(i) = self.queue.iter().position(|e| e.answers(only))
                && let Some(event) = self.queue.remove(i)
            {
                return Ok(event);
            }
            if let Some(end) = self.end {
                return Ok(end);
            }
            match self.ports.receive()? {
                Received::Exception(exception) => {
                    if let Some(event) = self.exception(exception) {
                        self.queue.push_back(event);
                    }
                }
                Received::Interrupt => self.queue.push_back(Event::Interrupted(None)),
                Received::Nothing => match self.exit.poll() {
                    Some(end) => self.end = Some(end),
                    None => self.refresh(),
                },
            }
        }
    }

    fn regs(&self, tid: Tid) -> Result<Regs, OsError> {
        state::regs(self.thread(tid)?)
    }

    fn set_pc(&mut self, tid: Tid, pc: u64) -> Result<(), OsError> {
        state::set_pc(self.thread(tid)?, pc)
    }

    fn read(&self, addr: u64, buf: &mut [u8]) -> bool {
        sys::read(self.task, addr, buf)
    }

    fn write(&mut self, addr: u64, bytes: &[u8]) -> Result<(), OsError> {
        sys::write(self.task, addr, bytes)
    }

    fn claim(&mut self, addr: u64) {
        self.claimed.insert(addr);
    }

    fn modules(&self) -> Vec<Module> {
        images::modules(self.task)
    }

    fn detach(&mut self, stopped: &[Tid]) {
        self.previous.restore(self.task);
        self.drain();
        for tid in std::mem::take(&mut self.stepping) {
            if let Ok(thread) = self.thread(tid) {
                let _ = state::set_stepping(thread, false);
            }
        }
        for &tid in stopped.iter().chain(self.held.clone().iter()) {
            let _ = self.resume(tid);
        }
        for (_, thread) in self.threads.drain() {
            sys::release(thread);
        }
        INTERRUPTS.lock().retain(|&(p, _)| p != self.pid);
        self.ports.close();
        sys::release(self.task);
    }
}

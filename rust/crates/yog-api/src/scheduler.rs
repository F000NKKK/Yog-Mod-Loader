//! Tasks a mod schedules in server ticks: kept by the mod itself, run by
//! its subscription to the server's tick (after the tick's other handlers).

use std::sync::{Arc, Mutex, Once};

use yog_abi::priority;
use yog_core::Server;
use yog_event::server::game::Tick;
use yog_sync::Locked;

use crate::registry::Registry;

/// A scheduled task.
type Task = Arc<dyn Fn(&dyn Server) + Send + Sync>;

/// The tasks waiting for their tick.
struct Tasks<T> {
    /// Ticks left, and the task.
    once: Vec<(u64, T)>,
    /// Period, ticks left, and the task.
    repeating: Vec<(u64, u64, T)>,
}

impl<T: Clone> Tasks<T> {
    const fn new() -> Self {
        Self { once: Vec::new(), repeating: Vec::new() }
    }

    /// One tick: the tasks due now — once tasks whose delay ran out (`d`
    /// ticks later, 0 and 1 meaning the next tick), repeating ones whose
    /// period came round (every `p` ticks, 0 meaning every tick). Once
    /// tasks fired are gone.
    fn due(&mut self) -> Vec<T> {
        let mut out = Vec::new();
        self.once.retain_mut(|(left, task)| {
            if *left <= 1 {
                out.push(task.clone());
                false
            } else {
                *left -= 1;
                true
            }
        });
        for (period, left, task) in &mut self.repeating {
            *left = left.saturating_sub(1);
            if *left == 0 {
                out.push(task.clone());
                *left = (*period).max(1);
            }
        }
        out
    }
}

static TASKS: Mutex<Tasks<Task>> = Mutex::new(Tasks::new());
static TICKING: Once = Once::new();

impl Registry {
    /// Runs `handler` once, `delay_ticks` server ticks from now.
    pub fn schedule_once<F>(&self, delay_ticks: u64, handler: F)
    where
        F: Fn(&dyn Server) + Send + Sync + 'static,
    {
        TASKS.locked().once.push((delay_ticks, Arc::new(handler)));
        self.tick_tasks();
    }

    /// Runs `handler` every `period_ticks` server ticks.
    pub fn schedule_repeating<F>(&self, period_ticks: u64, handler: F)
    where
        F: Fn(&dyn Server) + Send + Sync + 'static,
    {
        TASKS.locked().repeating.push((period_ticks, period_ticks.max(1), Arc::new(handler)));
        self.tick_tasks();
    }

    /// Subscribes the tasks to the server's tick, once.
    fn tick_tasks(&self) {
        TICKING.call_once(|| {
            self.after_at::<Tick, _, _>(priority::LATE, |_, cx| {
                let due = TASKS.locked().due();
                for task in due {
                    crate::registry::guard((), || task(cx.server()));
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduled_tasks_run_when_due() {
        let mut tasks = Tasks::new();
        tasks.once.extend([(0, 'a'), (3, 'b')]);
        tasks.repeating.extend([(2, 2, 'c'), (0, 1, 'd')]);
        let per_tick: Vec<usize> = (0..6).map(|_| tasks.due().len()).collect();
        // tick 1: once(0), every tick; 2: every 2, every tick; 3: once(3), every tick; …
        assert_eq!(per_tick, [2, 2, 2, 2, 1, 2]);
        assert!(tasks.once.is_empty(), "once tasks are gone once run");
    }
}

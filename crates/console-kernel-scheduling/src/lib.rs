//! Which task runs next, on how many cores, and within what budget.
//!
//! The kernel owns the mechanism -- the run queue, the switch, the timer that
//! ends a slice -- and none of the policy. The policy is a `Scheduler`, three
//! questions asked of whatever implements it, which is Linux's sched_ext said
//! as a trait rather than as a BPF program loaded at run time: a handheld
//! wants one answer on battery with the screen dark and another with a game in
//! front of it, and "which scheduler is better" is only settled by running two
//! against the same afternoon. So the answers are pure arithmetic over what
//! they are handed, the clock and the energy counters are read at the edge and
//! handed in, and `replay` runs one recorded workload through any
//! implementation and hands back numbers two of them can be compared by.
//!
//! The first policy is `Lavd`, taken from scx_lavd, the sched_ext scheduler
//! Igalia wrote with Valve for the Steam Deck, which is the nearest machine to
//! this one anybody has tuned a scheduler for. Its argument is that nobody
//! should have to label a task interactive: a task that runs briefly, waits
//! often and wakes others often sits on a chain somebody is looking at, so its
//! latency-criticality is `(log2(waits * wakes) + log2(reverse runtime))^2`,
//! and its virtual deadline is its arrival plus the targeted latency divided
//! by that. Earliest deadline runs. A batch task gets the whole targeted
//! latency, so it is never starved -- a deadline is a moment, and every task
//! queued behind one reaches it -- and an interactive one gets a sliver of it,
//! so it is on a core before the frame it is part of is late. The slice is the
//! targeted latency shared among the runnable tasks, clamped both ways.
//!
//! The second half of lavd is core compaction, and it is the half the battery
//! is spent on. Load is not spread across every core: the cores needed to
//! carry it, with a quarter of headroom, stay awake and run hot, and the rest
//! sleep in their deepest C-state. The energy mode says how hot a core may be
//! run -- Low Power packs harder, Automatic turns compaction off once half the
//! machine is busy, as lavd's balanced mode does, and High Power wakes every
//! core. A job, the group Zircon puts processes in, can be given a power cap,
//! and one that has spent more than its cap times the time elapsed is held off
//! the queue until the rest of the window catches up with it; a game then runs
//! at the pace the battery was promised rather than at the pace it would like.
//!
//! What was left of lavd: criticality inherited along the wake chain, the
//! logical clock and its compete window, the greedy penalty that pushes back
//! a task over its fair share, nice weights, the frequency boost, preemption
//! of a running task, and the energy-model states that pick fast cores on a
//! hybrid part -- the Z1 Extreme's cores are all alike. The statistics a task
//! is scored by are learned by a moving average at the edge and handed in
//! here. Zircon's own scheduler is the other precedent: weighted fair queueing
//! for most threads and earliest deadline first for those given a deadline
//! profile. Lavd is the second half with the deadline derived rather than
//! declared; `RoundRobin` is deliberately neither, so the trait is shown to
//! hold two and the comparison has a floor.

#![no_std]

extern crate alloc;

mod lavd;
mod replay;
mod round_robin;

pub use lavd::Lavd;
pub use replay::{replay, JobBudget, Latency, Outcome, Task, Wake, Workload};
pub use round_robin::RoundRobin;

use core::fmt;
use core::time::Duration;

use console_core_never::Never;

const MILLICORES_PER_CORE: u32 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant {
    pub since_boot: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frequency {
    pub per_second: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Utilization {
    pub millicores: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cores {
    pub count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Power {
    pub milliwatts: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Energy {
    pub microjoules: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Statistics {
    pub average_runtime: Duration,
    pub wakes_others: Frequency,
    pub waits: Frequency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Runnable {
    pub task: TaskId,
    pub job: JobId,
    pub statistics: Statistics,
    pub queued_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spending {
    pub energy: Energy,
    pub over: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Run { task: TaskId, slice: Duration },
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnergyMode {
    LowPower,
    Automatic,
    HighPower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Throttle {
    Running,
    Throttled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingError {
    UnknownTask(TaskId),
}

pub trait Scheduler {
    #[must_use]
    fn pick_next(&self, queue: &[Runnable]) -> Result<Choice, Never>;

    #[must_use]
    fn cores_awake(&self, demand: Utilization, mode: EnergyMode, present: Cores) -> Result<Cores, Never>;

    #[must_use]
    fn throttle(&self, spending: Spending, cap: Power) -> Result<Throttle, Never>;
}

impl Instant {
    #[must_use]
    pub fn after(self, elapsed: Duration) -> Result<Instant, Never> {
        Ok(Instant { since_boot: self.since_boot.saturating_add(elapsed) })
    }
}

impl Energy {
    #[must_use]
    pub fn spent(at: Power, over: Duration) -> Result<Energy, Never> {
        let product = u128::from(at.milliwatts).saturating_mul(over.as_micros()).saturating_div(1000);

        let microjoules = match u64::try_from(product) {
            Ok(microjoules) => microjoules,
            Err(_) => u64::MAX,
        };

        Ok(Energy { microjoules })
    }
}

impl fmt::Display for SchedulingError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchedulingError::UnknownTask(TaskId(id)) => write!(to, "the workload wakes task {id}, which it never declared"),
        }
    }
}

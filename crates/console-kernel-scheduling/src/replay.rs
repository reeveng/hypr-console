use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::{IntoIter, Vec};
use core::iter::Peekable;
use core::mem;
use core::time::Duration;

use console_core_never::Never;

use crate::{
    Choice, Cores, Energy, EnergyMode, Instant, JobId, Power, Runnable, Scheduler, SchedulingError, Spending,
    Statistics, TaskId, Throttle, Utilization, MILLICORES_PER_CORE,
};

const TICK: Duration = Duration::from_micros(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Latency {
    Critical,
    Tolerant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Task {
    pub id: TaskId,
    pub job: JobId,
    pub statistics: Statistics,
    pub latency: Latency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wake {
    pub at: Instant,
    pub task: TaskId,
    pub work: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobBudget {
    pub job: JobId,
    pub cap: Power,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workload {
    pub tasks: Vec<Task>,
    pub wakes: Vec<Wake>,
    pub budgets: Vec<JobBudget>,
    pub cores: Cores,
    pub mode: EnergyMode,
    pub busy_core: Power,
    pub budget_window: Duration,
    pub until: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    pub cores_awake: Duration,
    pub critical_waiting: Duration,
    pub over_budget: Energy,
}

#[derive(Debug, Clone, Copy)]
struct QueueEntry {
    runnable: Runnable,
    latency: Latency,
    left: Duration,
}

#[derive(Debug, Clone, Copy)]
struct OnCore {
    queued: QueueEntry,
    slice: Duration,
}

struct Machine<'w> {
    workload: &'w Workload,
    tasks: BTreeMap<TaskId, &'w Task>,
    now: Instant,
    queue: BTreeMap<TaskId, QueueEntry>,
    cores: BTreeMap<TaskId, OnCore>,
    demand: Utilization,
    spent: BTreeMap<JobId, Energy>,
    window_started: Instant,
    outcome: Outcome,
}

type Arriving = Peekable<IntoIter<Wake>>;

pub fn replay(scheduler: &dyn Scheduler, workload: &Workload) -> Result<Outcome, SchedulingError> {
    let mut wakes = workload.wakes.clone();

    wakes.sort_by_key(|wake| (wake.at, wake.task));

    let mut arriving = wakes.into_iter().peekable();
    let Ok(mut machine) = Machine::new(workload);
    let ticks = workload.until.since_boot.as_micros().saturating_div(TICK.as_micros());

    for _ in 0..ticks {
        machine.step(scheduler, &mut arriving)?;
    }

    let Ok(outcome) = machine.finish();

    Ok(outcome)
}

fn count<T>(of: &BTreeMap<TaskId, T>) -> Result<u32, Never> {
    Ok(match u32::try_from(of.len()) {
        Ok(count) => count,
        Err(_too_large) => u32::MAX,
    })
}

impl<'w> Machine<'w> {
    fn new(workload: &'w Workload) -> Result<Machine<'w>, Never> {
        let start = Instant { since_boot: Duration::ZERO };
        let nothing = Energy { microjoules: 0 };

        Ok(Machine {
            workload,
            tasks: workload.tasks.iter().map(|task| (task.id, task)).collect(),
            now: start,
            queue: BTreeMap::new(),
            cores: BTreeMap::new(),
            demand: Utilization { millicores: 0 },
            spent: BTreeMap::new(),
            window_started: start,
            outcome: Outcome { cores_awake: Duration::ZERO, critical_waiting: Duration::ZERO, over_budget: nothing },
        })
    }

    fn step(&mut self, scheduler: &dyn Scheduler, arriving: &mut Arriving) -> Result<(), SchedulingError> {
        self.arrive(arriving)?;

        let Ok(awake) = self.awake(scheduler);
        let Ok(()) = self.preempt(awake);
        let Ok(()) = self.fill(scheduler, awake);
        let Ok(()) = self.account(awake);
        let Ok(()) = self.run();
        let Ok(()) = self.close_window();

        Ok(())
    }

    fn arrive(&mut self, arriving: &mut Arriving) -> Result<(), SchedulingError> {
        let now = self.now;

        while let Some(wake) = arriving.next_if(|wake| wake.at <= now) {
            let task = match self.tasks.get(&wake.task) {
                Some(task) => *task,
                None => return Err(SchedulingError::UnknownTask(wake.task)),
            };

            match self.cores.get_mut(&wake.task) {
                Some(on) => on.queued.left = on.queued.left.saturating_add(wake.work),
                None => {
                    let runnable = Runnable { task: task.id, job: task.job, statistics: task.statistics, queued_at: wake.at };
                    let queued = self.queue.entry(wake.task).or_insert(QueueEntry { runnable, latency: task.latency, left: Duration::ZERO });

                    queued.left = queued.left.saturating_add(wake.work);
                },
            }
        }

        Ok(())
    }

    fn awake(&mut self, scheduler: &dyn Scheduler) -> Result<Cores, Never> {
        let Ok(waiting) = count(&self.queue);
        let Ok(running) = count(&self.cores);
        let present = self.workload.cores;
        let wanted = waiting.saturating_add(running).min(present.count).saturating_mul(MILLICORES_PER_CORE);
        let smoothed = self.demand.millicores.saturating_mul(3).saturating_add(wanted).saturating_div(4);

        self.demand = Utilization { millicores: smoothed };

        scheduler.cores_awake(self.demand, self.workload.mode, present)
    }

    fn preempt(&mut self, awake: Cores) -> Result<(), Never> {
        let Ok(running) = count(&self.cores);

        for _ in 0..running.saturating_sub(awake.count) {
            match self.cores.pop_last() {
                Some((task, on)) => {
                    let Ok(()) = self.requeue(task, on.queued, self.now);
                },
                None => {},
            }
        }

        Ok(())
    }

    fn fill(&mut self, scheduler: &dyn Scheduler, awake: Cores) -> Result<(), Never> {
        let Ok(held) = self.held(scheduler);
        let Ok(running) = count(&self.cores);

        for _ in running..awake.count {
            let eligible: Vec<Runnable> = self
                .queue
                .values()
                .filter(|queued| !held.contains(&queued.runnable.job))
                .map(|queued| queued.runnable)
                .collect();

            let Ok(choice) = scheduler.pick_next(&eligible);

            let (task, slice) = match choice {
                Choice::Run { task, slice } => (task, slice),
                Choice::Idle => return Ok(()),
            };

            match self.queue.remove(&task) {
                Some(queued) => {
                    let _started = self.cores.insert(task, OnCore { queued, slice });
                },
                None => return Ok(()),
            }
        }

        Ok(())
    }

    fn held(&self, scheduler: &dyn Scheduler) -> Result<BTreeSet<JobId>, Never> {
        let over = self.now.since_boot.saturating_sub(self.window_started.since_boot);

        Ok(self
            .workload
            .budgets
            .iter()
            .filter(|budget| {
                let Ok(energy) = self.spent_by(budget.job);
                let Ok(verdict) = scheduler.throttle(Spending { energy, over }, budget.cap);

                verdict == Throttle::Throttled
            })
            .map(|budget| budget.job)
            .collect())
    }

    fn account(&mut self, awake: Cores) -> Result<(), Never> {
        self.outcome.cores_awake = self.outcome.cores_awake.saturating_add(TICK.saturating_mul(awake.count));

        for queued in self.queue.values() {
            match queued.latency {
                Latency::Critical => self.outcome.critical_waiting = self.outcome.critical_waiting.saturating_add(TICK),
                Latency::Tolerant => {},
            }
        }

        Ok(())
    }

    fn run(&mut self) -> Result<(), Never> {
        let Ok(charge) = Energy::spent(self.workload.busy_core, TICK);
        let Ok(next) = self.now.after(TICK);
        let running = mem::take(&mut self.cores);

        for (task, mut on) in running {
            let Ok(()) = self.charge(on.queued.runnable.job, charge);

            on.queued.left = on.queued.left.saturating_sub(TICK);
            on.slice = on.slice.saturating_sub(TICK);

            match (on.queued.left.is_zero(), on.slice.is_zero()) {
                (true, _) => {},
                (false, true) => {
                    let Ok(()) = self.requeue(task, on.queued, next);
                },
                (false, false) => {
                    let _kept = self.cores.insert(task, on);
                },
            }
        }

        self.now = next;

        Ok(())
    }

    fn requeue(&mut self, task: TaskId, mut queued: QueueEntry, at: Instant) -> Result<(), Never> {
        queued.runnable.queued_at = at;

        let _requeued = self.queue.insert(task, queued);

        Ok(())
    }

    fn charge(&mut self, job: JobId, energy: Energy) -> Result<(), Never> {
        let Ok(before) = self.spent_by(job);
        let after = Energy { microjoules: before.microjoules.saturating_add(energy.microjoules) };
        let _before = self.spent.insert(job, after);

        Ok(())
    }

    fn spent_by(&self, job: JobId) -> Result<Energy, Never> {
        Ok(match self.spent.get(&job) {
            Some(energy) => *energy,
            None => Energy { microjoules: 0 },
        })
    }

    fn close_window(&mut self) -> Result<(), Never> {
        let elapsed = self.now.since_boot.saturating_sub(self.window_started.since_boot);

        match elapsed >= self.workload.budget_window {
            true => self.settle(elapsed),
            false => Ok(()),
        }
    }

    fn settle(&mut self, elapsed: Duration) -> Result<(), Never> {
        let workload = self.workload;

        for budget in &workload.budgets {
            let Ok(allowed) = Energy::spent(budget.cap, elapsed);
            let Ok(spent) = self.spent_by(budget.job);
            let over = spent.microjoules.saturating_sub(allowed.microjoules);
            let total = self.outcome.over_budget.microjoules.saturating_add(over);

            self.outcome.over_budget = Energy { microjoules: total };
        }

        self.spent.clear();
        self.window_started = self.now;

        Ok(())
    }

    fn finish(mut self) -> Result<Outcome, Never> {
        let elapsed = self.now.since_boot.saturating_sub(self.window_started.since_boot);
        let Ok(()) = self.settle(elapsed);

        Ok(self.outcome)
    }
}

use core::time::Duration;

use console_core_never::Never;

use crate::{Choice, Cores, EnergyMode, Power, Runnable, Scheduler, Spending, Throttle, Utilization};

const SLICE: Duration = Duration::from_millis(4);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RoundRobin;

impl Scheduler for RoundRobin {
    fn pick_next(&self, queue: &[Runnable]) -> Result<Choice, Never> {
        let longest_waiting = queue.iter().min_by_key(|runnable| (runnable.queued_at, runnable.task));

        Ok(match longest_waiting {
            Some(runnable) => Choice::Run { task: runnable.task, slice: SLICE },
            None => Choice::Idle,
        })
    }

    fn cores_awake(&self, _demand: Utilization, _mode: EnergyMode, present: Cores) -> Result<Cores, Never> {
        Ok(present)
    }

    fn throttle(&self, _spending: Spending, _cap: Power) -> Result<Throttle, Never> {
        Ok(Throttle::Running)
    }
}

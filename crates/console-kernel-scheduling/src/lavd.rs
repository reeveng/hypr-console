use core::time::Duration;

use console_core_never::Never;

use crate::{Choice, Cores, Energy, EnergyMode, Frequency, Instant, Power, Runnable, Scheduler, Spending, Statistics, Throttle, Utilization, MILLICORES_PER_CORE};

const TARGETED_LATENCY: Duration = Duration::from_millis(15);
const SLICE_MIN: Duration = Duration::from_micros(500);
const SLICE_MAX: Duration = Duration::from_millis(5);
const RUNTIME_MAX: Duration = Duration::from_millis(4);
const FREQUENCY_MAX: u32 = 1000;
const HEADROOM_PERCENT: u32 = 125;
const LOW_POWER_TARGET: Utilization = Utilization { millicores: 800 };
const AUTOMATIC_TARGET: Utilization = Utilization { millicores: 600 };

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Lavd;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Criticality(u32);

fn log2(of: u64) -> Result<u32, Never> {
    Ok(match of.checked_ilog2() {
        Some(log) => log,
        None => 0,
    })
}

fn factor(of: Frequency) -> Result<u64, Never> {
    Ok(u64::from(of.per_second.min(FREQUENCY_MAX)).saturating_add(1))
}

fn reverse_runtime(of: Duration) -> Result<u64, Never> {
    let shorter = RUNTIME_MAX.saturating_sub(of).as_micros();
    let steps = shorter.saturating_div(SLICE_MIN.as_micros());

    Ok(match u64::try_from(steps) {
        Ok(steps) => steps.saturating_add(1),
        Err(_) => 1,
    })
}

fn criticality(of: Statistics) -> Result<Criticality, Never> {
    let Ok(waits) = factor(of.waits);
    let Ok(wakes) = factor(of.wakes_others);
    let Ok(chain) = log2(waits.saturating_mul(wakes));
    let Ok(runtime) = reverse_runtime(of.average_runtime);
    let Ok(brevity) = log2(runtime);
    let sum = chain.saturating_add(brevity);

    Ok(Criticality(sum.saturating_mul(sum)))
}

fn deadline(of: &Runnable) -> Result<Instant, Never> {
    let Ok(Criticality(critical)) = criticality(of.statistics);

    let window = match TARGETED_LATENCY.checked_div(critical.saturating_add(1)) {
        Some(window) => window,
        None => TARGETED_LATENCY,
    };

    of.queued_at.after(window)
}

fn slice(among: &[Runnable]) -> Result<Duration, Never> {
    let runnable = match u32::try_from(among.len()) {
        Ok(runnable) => runnable.max(1),
        Err(_) => u32::MAX,
    };

    let shared = match TARGETED_LATENCY.checked_div(runnable) {
        Some(shared) => shared,
        None => SLICE_MIN,
    };

    Ok(shared.clamp(SLICE_MIN, SLICE_MAX))
}

impl Scheduler for Lavd {
    fn pick_next(&self, queue: &[Runnable]) -> Result<Choice, Never> {
        let earliest = queue.iter().min_by_key(|runnable| {
            let Ok(due) = deadline(runnable);

            (due, runnable.task)
        });

        let Ok(slice) = slice(queue);

        Ok(match earliest {
            Some(runnable) => Choice::Run { task: runnable.task, slice },
            None => Choice::Idle,
        })
    }

    fn cores_awake(&self, demand: Utilization, mode: EnergyMode, present: Cores) -> Result<Cores, Never> {
        let required = demand.millicores.saturating_mul(HEADROOM_PERCENT).saturating_div(100);
        let whole = present.count.saturating_mul(MILLICORES_PER_CORE);

        let target = match (mode, required.saturating_mul(2) >= whole) {
            (EnergyMode::HighPower, _) | (EnergyMode::Automatic, true) => return Ok(present),
            (EnergyMode::Automatic, false) => AUTOMATIC_TARGET,
            (EnergyMode::LowPower, _) => LOW_POWER_TARGET,
        };

        let count = required.div_ceil(target.millicores);

        Ok(Cores { count: count.max(1).min(present.count) })
    }

    fn throttle(&self, spending: Spending, cap: Power) -> Result<Throttle, Never> {
        let Ok(allowed) = Energy::spent(cap, spending.over);

        Ok(match spending.energy > allowed {
            true => Throttle::Throttled,
            false => Throttle::Running,
        })
    }
}

use core::time::Duration;

use console_kernel_scheduling::{
    replay, Cores, EnergyMode, Frequency, Instant, JobBudget, JobId, Latency, Lavd, Power, RoundRobin,
    SchedulingError, Statistics, Task, TaskId, Wake, Workload,
};

const COMPOSITOR: TaskId = TaskId(1);
const INDEXER: TaskId = TaskId(2);
const DESKTOP: JobId = JobId(1);
const GAME: JobId = JobId(2);
const SECOND: Duration = Duration::from_secs(1);

fn task(id: TaskId, job: JobId, statistics: Statistics, latency: Latency) -> Result<Task, SchedulingError> {
    Ok(Task { id, job, statistics, latency })
}

fn every(period: Duration, work: Duration, task: TaskId) -> Result<Vec<Wake>, SchedulingError> {
    let mut wakes = Vec::new();
    let mut at = Duration::ZERO;

    while at < SECOND {
        wakes.push(Wake { at: Instant { since_boot: at }, task, work });
        at = at.saturating_add(period);
    }

    Ok(wakes)
}

fn light_interactive() -> Result<Workload, SchedulingError> {
    let frames = Statistics {
        average_runtime: Duration::from_micros(300),
        wakes_others: Frequency { per_second: 120 },
        waits: Frequency { per_second: 120 },
    };
    let indexing = Statistics {
        average_runtime: Duration::from_millis(2),
        wakes_others: Frequency { per_second: 0 },
        waits: Frequency { per_second: 10 },
    };
    let mut wakes = every(Duration::from_micros(8333), Duration::from_micros(300), COMPOSITOR)?;

    wakes.extend(every(Duration::from_millis(100), Duration::from_millis(2), INDEXER)?);

    Ok(Workload {
        tasks: vec![task(COMPOSITOR, DESKTOP, frames, Latency::Critical)?, task(INDEXER, DESKTOP, indexing, Latency::Tolerant)?],
        wakes,
        budgets: Vec::new(),
        cores: Cores { count: 8 },
        mode: EnergyMode::Automatic,
        busy_core: Power { milliwatts: 1500 },
        budget_window: SECOND,
        until: Instant { since_boot: SECOND },
    })
}

fn game_under_a_cap() -> Result<Workload, SchedulingError> {
    let render = Statistics {
        average_runtime: Duration::from_millis(6),
        wakes_others: Frequency { per_second: 60 },
        waits: Frequency { per_second: 60 },
    };
    let threads = [TaskId(10), TaskId(11), TaskId(12), TaskId(13)];
    let mut tasks = vec![];
    let mut wakes = vec![];

    for thread in threads {
        tasks.push(task(thread, GAME, render, Latency::Tolerant)?);
        wakes.push(Wake { at: Instant { since_boot: Duration::ZERO }, task: thread, work: SECOND });
    }

    Ok(Workload {
        tasks,
        wakes,
        budgets: vec![JobBudget { job: GAME, cap: Power { milliwatts: 4500 } }],
        cores: Cores { count: 8 },
        mode: EnergyMode::HighPower,
        busy_core: Power { milliwatts: 3000 },
        budget_window: Duration::from_millis(250),
        until: Instant { since_boot: SECOND },
    })
}

#[test]
fn lavd_keeps_fewer_cores_awake_than_round_robin_for_a_light_interactive_load() -> Result<(), SchedulingError> {
    let workload = light_interactive()?;

    let lavd = replay(&Lavd, &workload)?;
    let round_robin = replay(&RoundRobin, &workload)?;

    assert!(lavd.cores_awake.saturating_mul(4) < round_robin.cores_awake, "{lavd:?} against {round_robin:?}");
    assert!(lavd.critical_waiting <= Duration::from_millis(1), "{lavd:?}");

    Ok(())
}

#[test]
fn on_one_busy_core_a_frame_waits_less_under_lavd_than_under_round_robin() -> Result<(), SchedulingError> {
    let mut workload = light_interactive()?;
    let batch = Statistics {
        average_runtime: Duration::from_millis(40),
        wakes_others: Frequency { per_second: 0 },
        waits: Frequency { per_second: 1 },
    };

    workload.cores = Cores { count: 1 };

    for id in [20, 21, 22] {
        workload.tasks.push(task(TaskId(id), DESKTOP, batch, Latency::Tolerant)?);
        workload.wakes.push(Wake { at: Instant { since_boot: Duration::ZERO }, task: TaskId(id), work: SECOND });
    }

    let lavd = replay(&Lavd, &workload)?;
    let round_robin = replay(&RoundRobin, &workload)?;

    assert!(lavd.critical_waiting < round_robin.critical_waiting, "{lavd:?} against {round_robin:?}");

    Ok(())
}

#[test]
fn a_game_under_a_cap_keeps_to_it_under_lavd_and_not_under_round_robin() -> Result<(), SchedulingError> {
    let workload = game_under_a_cap()?;

    let lavd = replay(&Lavd, &workload)?;
    let round_robin = replay(&RoundRobin, &workload)?;

    assert!(lavd.over_budget.microjoules <= 45_000, "{lavd:?}");
    assert!(round_robin.over_budget.microjoules >= 7_000_000, "{round_robin:?}");

    Ok(())
}

#[test]
fn the_same_workload_replays_to_the_same_numbers() -> Result<(), SchedulingError> {
    let workload = light_interactive()?;

    let first = replay(&Lavd, &workload)?;
    let second = replay(&Lavd, &workload)?;

    assert_eq!(first, second);

    Ok(())
}

#[test]
fn a_wake_for_a_task_nobody_declared_is_refused() -> Result<(), SchedulingError> {
    let mut workload = light_interactive()?;

    workload.wakes.push(Wake { at: Instant { since_boot: Duration::ZERO }, task: TaskId(99), work: SECOND });

    assert_eq!(replay(&Lavd, &workload), Err(SchedulingError::UnknownTask(TaskId(99))));

    Ok(())
}

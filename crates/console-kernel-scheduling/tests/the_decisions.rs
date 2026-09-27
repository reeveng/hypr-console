use core::time::Duration;

use console_kernel_scheduling::{
    Choice, Cores, Energy, EnergyMode, Frequency, Instant, JobId, Lavd, Power, RoundRobin, Runnable, Scheduler,
    SchedulingError, Spending, Statistics, TaskId, Throttle, Utilization,
};

const INTERACTIVE: Statistics = Statistics {
    average_runtime: Duration::from_micros(200),
    wakes_others: Frequency { per_second: 240 },
    waits: Frequency { per_second: 240 },
};

const BATCH: Statistics = Statistics {
    average_runtime: Duration::from_millis(40),
    wakes_others: Frequency { per_second: 0 },
    waits: Frequency { per_second: 1 },
};

const LEGION_GO: Cores = Cores { count: 8 };

fn queued(task: TaskId, statistics: Statistics, at: Duration) -> Result<Runnable, SchedulingError> {
    Ok(Runnable {
        task,
        job: JobId(task.0),
        statistics,
        queued_at: Instant { since_boot: at },
    })
}

fn task_of(choice: Choice) -> Result<TaskId, SchedulingError> {
    match choice {
        Choice::Run { task, .. } => Ok(task),
        Choice::Idle => Ok(TaskId(0)),
    }
}

#[test]
fn an_interactive_task_reaches_the_core_before_a_batch_task_queued_ahead_of_it() -> Result<(), SchedulingError> {
    let batch = queued(TaskId(1), BATCH, Duration::from_micros(0))?;
    let interactive = queued(TaskId(2), INTERACTIVE, Duration::from_micros(1000))?;
    let queue = [batch, interactive];

    let Ok(lavd) = Lavd.pick_next(&queue);
    let Ok(round_robin) = RoundRobin.pick_next(&queue);

    let picked_by_lavd = task_of(lavd)?;
    assert_eq!(picked_by_lavd, TaskId(2));
    let picked_by_round_robin = task_of(round_robin)?;
    assert_eq!(picked_by_round_robin, TaskId(1));

    Ok(())
}

#[test]
fn a_batch_task_is_not_starved_once_its_deadline_comes() -> Result<(), SchedulingError> {
    let batch = queued(TaskId(1), BATCH, Duration::from_micros(0))?;
    let interactive = queued(TaskId(2), INTERACTIVE, Duration::from_micros(20_000))?;
    let queue = [batch, interactive];

    let Ok(choice) = Lavd.pick_next(&queue);

    let chosen = task_of(choice)?;
    assert_eq!(chosen, TaskId(1));

    Ok(())
}

#[test]
fn an_empty_queue_leaves_the_core_idle() -> Result<(), SchedulingError> {
    let Ok(choice) = Lavd.pick_next(&[]);

    assert_eq!(choice, Choice::Idle);

    Ok(())
}

#[test]
fn light_load_keeps_fewer_cores_awake_in_low_power_than_in_high_power() -> Result<(), SchedulingError> {
    let light = Utilization { millicores: 1500 };

    let Ok(low) = Lavd.cores_awake(light, EnergyMode::LowPower, LEGION_GO);
    let Ok(automatic) = Lavd.cores_awake(light, EnergyMode::Automatic, LEGION_GO);
    let Ok(high) = Lavd.cores_awake(light, EnergyMode::HighPower, LEGION_GO);

    assert_eq!(low, Cores { count: 3 });
    assert_eq!(automatic, Cores { count: 4 });
    assert_eq!(high, LEGION_GO);

    Ok(())
}

#[test]
fn a_busy_machine_wakes_every_core_in_automatic_and_an_idle_one_keeps_one() -> Result<(), SchedulingError> {
    let Ok(busy) = Lavd.cores_awake(Utilization { millicores: 3500 }, EnergyMode::Automatic, LEGION_GO);
    let Ok(idle) = Lavd.cores_awake(Utilization { millicores: 0 }, EnergyMode::LowPower, LEGION_GO);

    assert_eq!(busy, LEGION_GO);
    assert_eq!(idle, Cores { count: 1 });

    Ok(())
}

#[test]
fn a_job_over_its_budget_is_throttled_and_one_within_it_is_not() -> Result<(), SchedulingError> {
    let cap = Power { milliwatts: 5000 };
    let second = Duration::from_secs(1);
    let over = Spending { energy: Energy { microjoules: 7_000_000 }, over: second };
    let within = Spending { energy: Energy { microjoules: 3_000_000 }, over: second };

    let Ok(game) = Lavd.throttle(over, cap);
    let Ok(browser) = Lavd.throttle(within, cap);

    assert_eq!(game, Throttle::Throttled);
    assert_eq!(browser, Throttle::Running);

    Ok(())
}

#[test]
fn the_same_queue_is_answered_the_same_way_twice() -> Result<(), SchedulingError> {
    let task_three = queued(TaskId(3), INTERACTIVE, Duration::from_micros(500))?;
    let task_one = queued(TaskId(1), BATCH, Duration::from_micros(0))?;
    let task_two = queued(TaskId(2), INTERACTIVE, Duration::from_micros(500))?;
    let queue = [task_three, task_one, task_two];

    let Ok(first) = Lavd.pick_next(&queue);
    let Ok(second) = Lavd.pick_next(&queue);

    assert_eq!(first, second);
    let chosen = task_of(first)?;
    assert_eq!(chosen, TaskId(2));

    Ok(())
}

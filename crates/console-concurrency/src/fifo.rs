//! Picture jobs own their inputs and can run after the decoder submits them.
//! Workers claim jobs in FIFO order so a later picture waiting on a reference
//! always has an older worker making progress. Both decoders allow two queued
//! pictures beyond their worker count. Once the sender is dropped, workers
//! finish queued jobs and exit.
//! An uncaught job panic aborts the process because dependents may already be
//! waiting for a result that the failed job can no longer publish.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, mpsc};

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_parallel_execution::Workers;

use crate::HostWorkers;

type Task = Box<dyn FnOnce() + Send>;

const QUEUED_PICTURES: u32 = 2;

pub struct FifoPool {
  jobs: mpsc::SyncSender<Task>,
}

impl FifoPool {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "persistent workers must claim jobs from one FIFO receiver; the decoder owns its only sender and closing it ends every worker"
    )
  )]
  pub fn for_threads(name: &str, width: u32) -> Result<Option<Self>, Never> {
    match width {
      0 | 1 => return Ok(None),
      _ => {}
    }

    let Ok(capacity) = index(QUEUED_PICTURES);
    let (jobs, receive) = mpsc::sync_channel(capacity);
    let queue = Arc::new(Mutex::new(receive));

    for number in 0..width {
      let worker_queue = Arc::clone(&queue);
      let started = HostWorkers.start(format!("{name}-{number}"), move || {
        match serve(&worker_queue) {
          Ok(()) => {}
          Err(Endless) => drop(worker_queue),
        }
      });

      match started {
        Ok(worker) => drop(worker),
        Err(_could_not_start_pool) => return Ok(None),
      }
    }

    Ok(Some(Self { jobs }))
  }

  pub fn submit(&self, job: impl FnOnce() + Send + 'static) -> Result<(), Never> {
    match self.jobs.send(Box::new(job)) {
      Ok(()) => {}
      Err(unsent) => unsent.0(),
    }

    Ok(())
  }
}

fn serve(queue: &Mutex<mpsc::Receiver<Task>>) -> Result<(), Endless> {
  iterate(queue, |pending| {
    #[cfg_attr(
      dylint_lib = "explicit057_no_unbounded_wait",
      expect(
        explicit057_no_unbounded_wait,
        reason = "the decoder owns the only sender; releasing it wakes the idle workers, and the receiver guard is released before any job runs"
      )
    )]
    let next = {
      let receiver = match pending.lock() {
        Ok(receiver) => receiver,
        Err(poisoned) => poisoned.into_inner(),
      };

      receiver.recv()
    };

    let job = match next {
      Ok(job) => job,
      Err(_pool_closed) => return Ok(Step::Halt(())),
    };

    match catch_unwind(AssertUnwindSafe(job)) {
      Ok(()) => {}
      #[cfg_attr(
        dylint_lib = "explicit042_no_leaving_early",
        expect(
          explicit042_no_leaving_early,
          reason = "this preserves the uncaught-panic contract of detached picture jobs: after codec recovery itself panics, readers may already be blocked on unpublished reference rows"
        )
      )]
      Err(_job_failed_to_publish) => std::process::abort(),
    }

    Ok(Step::Again(pending))
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::Duration;

  #[test]
  fn older_jobs_can_run_concurrently_and_queued_jobs_survive_drop()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(started_pool) = FifoPool::for_threads("fifo-test", 2);
    let pool = match started_pool {
      Some(pool) => pool,
      None => {
        return Err(Box::<dyn std::error::Error>::from(
          "could not start the two FIFO workers",
        ));
      }
    };
    let (started, running) = mpsc::sync_channel(2);
    let (done, finished) = mpsc::sync_channel(2);
    let deadline = Duration::from_secs(5);
    let mut releases = Vec::new();

    for _ in 0..2 {
      let (release, wait) = mpsc::sync_channel::<()>(0);
      releases.push(release);
      let arrived = started.clone();
      let Ok(()) = pool.submit(move || {
        let _test_may_have_ended = arrived.send(());
        let _released_by_dropping_the_sender = wait.recv_timeout(deadline);
      });
    }

    for _ in 0..2 {
      running.recv_timeout(deadline)?;
    }

    for index in 0_u32..2 {
      let completed = done.clone();
      let Ok(()) = pool.submit(move || {
        let _test_may_have_ended = completed.send(index);
      });
    }

    drop(pool);
    drop(releases.pop());

    for index in 0_u32..2 {
      let completed = finished.recv_timeout(deadline)?;

      assert_eq!(completed, index);
    }

    Ok(())
  }
}

//! A worker that reads the latest state when woken. One queued wake is enough
//! while it waits or works, and shutdown drains the final wake before joining.

use std::io;
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};

use console_core_never::Never;
use console_core_parallel_execution::{Failed, Worker, Workers};

use crate::HostWorkers;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerStopped;

pub struct Latest {
  wake: SyncSender<()>,
  worker: Worker,
}

impl Latest {
  #[cfg_attr(
    dylint_lib = "explicit055_no_unbounded_loop",
    expect(
      explicit055_no_unbounded_loop,
      reason = "the owner closes this receiver by dropping its sole sender before joining the worker"
    )
  )]
  pub fn start(
    name: String,
    mut work: impl FnMut() -> Result<(), Never> + Send + 'static,
  ) -> io::Result<Self> {
    let (wake, notices) = sync_channel(1);
    let worker = HostWorkers.start(name, move || {
      for () in notices {
        let Ok(()) = work();
      }
    })?;

    Ok(Self { wake, worker })
  }

  pub fn schedule(&self) -> Result<(), WorkerStopped> {
    match self.wake.try_send(()) {
      Ok(()) | Err(TrySendError::Full(())) => Ok(()),
      Err(TrySendError::Disconnected(())) => Err(WorkerStopped),
    }
  }

  pub fn close(self) -> Result<(), Failed> {
    drop(self.wake);

    self.worker.join()
  }
}

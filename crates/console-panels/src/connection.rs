//! Keep one unused Wayland connection ready for the next opening.

use std::io;
use std::sync::mpsc::{Receiver, TryRecvError, TrySendError, sync_channel};

use console_concurrency::{Latest, WorkerStopped};
use console_core_never::Never;
use console_panel::surface::Connected;

pub struct Spare {
  ready: Receiver<Connected>,
  worker: Latest,
}

impl Spare {
  pub fn start() -> io::Result<Self> {
    let (publish, ready) = sync_channel(1);
    let worker = Latest::start("panel Wayland connection".to_owned(), move || {
      match Connected::open() {
        Ok(connected) => match publish.try_send(connected) {
          Ok(()) => {}
          Err(TrySendError::Full(extra) | TrySendError::Disconnected(extra)) => drop(extra),
        },
        Err(fault) => console_core_logging::debug!("preparing a panel connection: {fault}"),
      }

      Ok(())
    })?;
    let spare = Self { ready, worker };
    let Ok(()) = spare.replenish();

    Ok(spare)
  }

  pub fn take(&self) -> Result<Option<Connected>, Never> {
    Ok(match self.ready.try_recv() {
      Ok(ready) => Some(ready),
      Err(TryRecvError::Empty) => None,
      Err(TryRecvError::Disconnected) => {
        console_core_logging::trace!("the panel connection worker has stopped");

        None
      }
    })
  }

  pub fn replenish(&self) -> Result<(), Never> {
    match self.worker.schedule() {
      Ok(()) => {}
      Err(WorkerStopped) => {
        console_core_logging::trace!("no worker for the spare panel connection")
      }
    }

    Ok(())
  }
}

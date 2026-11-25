//! One decoder works while the displayed wallpaper continues. A newer request
//! replaces the queued job or invalidates the in-flight result. The wake socket
//! carries readiness and the mailbox owns the decoded pixels.

use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixDatagram;
use std::sync::{Arc, Mutex, MutexGuard};

use console_concurrency::{Latest, WorkerStopped};
use console_core_never::Never;

use crate::picture::Decode;
use crate::play::{Decoded, LoadFault, Ticket};

struct Job {
  ticket: Ticket,
  input: Decode,
}

enum Mailbox {
  Idle,
  Queued(Job),
  Running(Ticket),
  Ready(Decoded),
}

impl Mailbox {
  fn begin(&mut self) -> Result<Option<Job>, Never> {
    Ok(match std::mem::replace(self, Self::Idle) {
      Self::Queued(job) => {
        *self = Self::Running(job.ticket);
        Some(job)
      }
      before @ (Self::Idle | Self::Running(_) | Self::Ready(_)) => {
        *self = before;
        None
      }
    })
  }

  fn receive(&mut self) -> Result<Option<Decoded>, Never> {
    Ok(match std::mem::replace(self, Self::Idle) {
      Self::Ready(decoded) => Some(decoded),
      before @ (Self::Idle | Self::Queued(_) | Self::Running(_)) => {
        *self = before;
        None
      }
    })
  }

  fn running(&self) -> Result<Option<Ticket>, Never> {
    Ok(match self {
      Self::Running(ticket) => Some(*ticket),
      Self::Idle | Self::Queued(_) | Self::Ready(_) => None,
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the display and its sole decode worker exchange ownership of jobs and completed pixels through this small mailbox"
  )
)]
pub struct Decoder {
  worker: Latest,
  mailbox: Arc<Mutex<Mailbox>>,
  waking: UnixDatagram,
}

impl Decoder {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "one display and one decoder own the shared handoff until the daemon exits"
    )
  )]
  pub fn start() -> io::Result<Self> {
    let (wake, waking) = UnixDatagram::pair()?;
    wake.set_nonblocking(true)?;
    waking.set_nonblocking(true)?;
    let mailbox = Arc::new(Mutex::new(Mailbox::Idle));
    let decoding = Arc::clone(&mailbox);
    let worker = Latest::start("wallpaper decoder".to_owned(), move || {
      work(&decoding, &wake)
    })?;

    Ok(Self {
      worker,
      mailbox,
      waking,
    })
  }

  pub fn schedule(&self, ticket: Ticket, input: Decode) -> Result<(), LoadFault> {
    let Ok(()) = self.replace(Mailbox::Queued(Job { ticket, input }));

    match self.worker.schedule() {
      Ok(()) => Ok(()),
      Err(WorkerStopped) => {
        let Ok(()) = self.cancel();

        Err(LoadFault::WorkerStopped)
      }
    }
  }

  pub fn cancel(&self) -> Result<(), Never> {
    self.replace(Mailbox::Idle)
  }

  fn replace(&self, next: Mailbox) -> Result<(), Never> {
    let removed = {
      let Ok(mut held) = hold(&self.mailbox);

      std::mem::replace(&mut *held, next)
    };
    drop(removed);

    Ok(())
  }

  pub fn descriptor(&self) -> Result<BorrowedFd<'_>, Never> {
    Ok(self.waking.as_fd())
  }

  pub fn receive(&self) -> io::Result<Option<Decoded>> {
    match self.waking.recv(&mut [0; 1]) {
      Ok(_) => {}
      Err(fault) => match matches!(
        fault.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
      )
      .then_some(())
      {
        Some(()) => {}
        None => return Err(fault),
      },
    }

    let Ok(mut held) = hold(&self.mailbox);
    let Ok(decoded) = held.receive();

    Ok(decoded)
  }
}

fn work(mailbox: &Mutex<Mailbox>, wake: &UnixDatagram) -> Result<(), Never> {
  let next = {
    let Ok(mut held) = hold(mailbox);
    let Ok(job) = held.begin();

    job
  };
  let job = match next {
    Some(job) => job,
    None => return Ok(()),
  };
  let result = job.input.prepare().map_err(LoadFault::Decode);

  {
    let Ok(mut held) = hold(mailbox);
    let Ok(running) = held.running();

    match (running == Some(job.ticket)).then_some(()) {
      Some(()) => {
        *held = Mailbox::Ready(Decoded {
          ticket: job.ticket,
          result,
        })
      }
      None => return Ok(()),
    }
  }

  notify(wake)
}

fn notify(wake: &UnixDatagram) -> Result<(), Never> {
  let sent = wake.send(&[1]).map(|_| ());
  let delivered = match sent {
    Err(fault) => match (fault.kind() == io::ErrorKind::Interrupted).then_some(()) {
      Some(()) => wake.send(&[1]).map(|_| ()),
      None => Err(fault),
    },
    Ok(()) => Ok(()),
  };

  match delivered {
    Ok(()) => {}
    Err(fault) => match (fault.kind() == io::ErrorKind::WouldBlock).then_some(()) {
      Some(()) => {}
      None => console_core_logging::warn!("could not wake the wallpaper display: {fault}"),
    },
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the lock only covers moves of the mailbox variants; file reads, decoding and releasing displaced pixel memory happen after unlocking"
  )
)]
fn hold(mailbox: &Mutex<Mailbox>) -> Result<MutexGuard<'_, Mailbox>, Never> {
  Ok(match mailbox.lock() {
    Ok(held) => held,
    Err(poisoned) => poisoned.into_inner(),
  })
}

use std::collections::BTreeSet;
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::mpsc::{Receiver, RecvError};

use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_waiting::latch;
use rustix::io::Errno;

use super::Woke;

pub(super) struct Source {
  from: Woke,
  reading: OwnedFd,
}

impl AsFd for Source {
  fn as_fd(&self) -> BorrowedFd<'_> {
    self.reading.as_fd()
  }
}

pub(super) fn forwarded(from: Woke, input: Receiver<()>) -> io::Result<Source> {
  let pipe = latch::pipe()?;
  let worker = std::thread::Builder::new().spawn(move || {
    let Ok(()) = relay(input, pipe.saying);
  })?;
  let Ok(()) = console_program_lifetime::threads::let_go(worker);

  Ok(Source {
    from,
    reading: pipe.waiting,
  })
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the isolated subscription thread waits for a source to send or disconnect without waking the drawing loop or polling idle sources"
  )
)]
fn relay(input: Receiver<()>, output: File) -> Result<(), Never> {
  let settled = iterate((input, output), |(input_2, output_2)| {
    match input_2.recv() {
      Ok(()) => {}
      Err(RecvError) => return Ok(Step::Halt(())),
    }

    match pulse(&output_2) {
      Ok(()) => Ok(Step::Again((input_2, output_2))),
      Err(Errno::PIPE) => Ok(Step::Halt(())),
      Err(fault) => {
        eprintln!("console-bar: could not signal a changed reading: {fault}");

        Ok(Step::Halt(()))
      }
    }
  });

  match settled {
    Ok(()) => Ok(()),
    Err(_endless) => Ok(()),
  }
}

fn pulse(output: &File) -> rustix::io::Result<()> {
  for _ in 0..16 {
    match rustix::io::write(output, &[1]) {
      Ok(1) | Err(Errno::AGAIN) => return Ok(()),
      Ok(_short) => return Err(Errno::IO),
      Err(Errno::INTR) => continue,
      Err(fault) => return Err(fault),
    }
  }

  Err(Errno::INTR)
}

enum Reading {
  Different,
  Idle,
  Ended,
}

fn reading(one: &Source) -> Result<Reading, Never> {
  let mut incoming = [0; 4096];
  let mut pending = Reading::Idle;

  for _ in 0..16 {
    match rustix::io::read(&one.reading, &mut incoming) {
      Ok(0) => match pending {
        Reading::Different => return Ok(Reading::Different),
        Reading::Idle | Reading::Ended => return Ok(Reading::Ended),
      },
      Ok(_bytes) => pending = Reading::Different,
      Err(Errno::AGAIN) => return Ok(pending),
      Err(Errno::INTR) => continue,
      Err(fault) => {
        eprintln!("console-bar: {:?} wake source failed: {fault}", one.from);

        return Ok(Reading::Ended);
      }
    }
  }

  Ok(pending)
}

pub(super) fn take_woken(sources: &mut Vec<Source>) -> Result<BTreeSet<Woke>, Never> {
  let mut seen = BTreeSet::new();

  let mut ended = BTreeSet::new();

  for source in sources.iter() {
    let Ok(observed) = reading(source);

    match observed {
      Reading::Different => {
        seen.insert(source.from);
      }
      Reading::Idle => {}
      Reading::Ended => {
        ended.insert(source.from);
      }
    }
  }

  sources.retain(|source| !ended.contains(&source.from));

  Ok(seen)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_full_wake_pipe_keeps_the_pending_signal_and_accepts_the_next_one()
  -> Result<(), Box<dyn std::error::Error>> {
    let pipe = latch::pipe()?;
    let _capacity = rustix::pipe::fcntl_setpipe_size(&pipe.saying, 4096)?;
    let _filled = rustix::io::write(&pipe.saying, &[1; 4096])?;
    let mut sources = vec![Source {
      from: Woke::Sound,
      reading: pipe.waiting,
    }];

    pulse(&pipe.saying)?;
    let Ok(full) = take_woken(&mut sources);
    assert_eq!(full, BTreeSet::from([Woke::Sound]));
    pulse(&pipe.saying)?;
    let Ok(next) = take_woken(&mut sources);
    assert_eq!(next, BTreeSet::from([Woke::Sound]));

    Ok(())
  }

  #[test]
  fn repeated_wakes_coalesce_without_losing_another_source()
  -> Result<(), Box<dyn std::error::Error>> {
    let sound = latch::pipe()?;
    let network = latch::pipe()?;
    let mut sources = vec![
      Source {
        from: Woke::Sound,
        reading: sound.waiting,
      },
      Source {
        from: Woke::Network,
        reading: network.waiting,
      },
    ];
    let _sound = rustix::io::write(&sound.saying, &[1; 64])?;
    let _network = rustix::io::write(&network.saying, &[1])?;

    let Ok(seen) = take_woken(&mut sources);
    assert_eq!(seen, BTreeSet::from([Woke::Sound, Woke::Network]));

    drop(sound.saying);
    let Ok(quiet) = take_woken(&mut sources);
    assert!(quiet.is_empty());
    assert_eq!(sources.len(), 1);
    let _once_more = rustix::io::write(&network.saying, &[1])?;
    let Ok(later) = take_woken(&mut sources);
    assert_eq!(later, BTreeSet::from([Woke::Network]));

    Ok(())
  }
}

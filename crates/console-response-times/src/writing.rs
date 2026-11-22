//! The line, put on a queue and written by a thread of its own.
//!
//! A stopwatch that costs what it measures measures itself. The line is short
//! and the write is one `write` to a descriptor opened `O_APPEND`, but the rest
//! of it is not: the first line a process writes has to make the directory,
//! open the file and ask how long it already is, and every line after it pays
//! for a `write` on the thread that has just finished drawing a window and is
//! about to draw the next one. On a handheld that is the frame someone sees.
//!
//! So nothing on the timed thread touches the disk. It renders the line, hands
//! it to a queue and goes back to what it was doing; one thread per process,
//! started when the first line is written and never stopped, holds the file
//! open for the life of the process and does the writing.
//!
//! ## When the queue is full
//!
//! The handover is `try_send`, so a thread being timed never waits on the one
//! writing -- and a queue that is full means the disk is behind by four
//! thousand lines, which has never happened and would be worth knowing about.
//! The line is then written here, on the thread that made it, rather than
//! dropped: a stopwatch that quietly loses the slow openings is a stopwatch
//! that says the machine is fast.
//!
//! ## When a process is about to go
//!
//! A queued line is in this process and nowhere else, so a program that writes
//! one and exits at once can exit before the thread has written it. Anything
//! whose whole run is one wait -- going to Game Mode, coming back, the desktop
//! starting -- calls `settled` before it returns, which waits for the queue to
//! empty and no longer. A panel does not need to: it writes its opening and
//! then stays up for as long as someone is looking at it.
//!
//! ## When the file is long
//!
//! It is kept. `CAP` is ten gigabytes and the device has the room: what these
//! lines are for is the question *is this getting slower*, which cannot be
//! asked of a file that keeps a week. The rotation that is left is a last stop
//! against a program stuck in a loop, not a retention policy.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::measuring::{self, Measuring};
use crate::where_;

pub const CAP: u64 = 10 << 30;

const QUEUE: u32 = 4096;

enum Message {
  Line(String),
  Flush(SyncSender<()>),
}

pub fn line(said: &str) -> Result<(), Never> {
  let mut whole = String::with_capacity(said.len().saturating_add(1));
  whole.push_str(said);
  whole.push('\n');

  let Ok(held) = writer();

  let say = match held {
    Some(say) => say,
    None => {
      let Ok(()) = by_hand(&whole);

      return Ok(());
    }
  };

  match say.try_send(Message::Line(whole.clone())) {
    Ok(()) => {}
    Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
      let Ok(()) = by_hand(&whole);
    }
  }

  Ok(())
}

pub fn flush() -> Result<(), Never> {
  let Ok(held) = writer();

  let say = match held {
    Some(say) => say,
    None => return Ok(()),
  };

  let (told, back) = sync_channel(0);

  match say.send(Message::Flush(told)) {
    Ok(()) => {}
    Err(_writer_already_exited) => return Ok(()),
  }

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "flush is the explicit exit barrier for all preceding journal writes and its private acknowledgment disconnects if the writer exits"
    )
  )]
  let receipt = back.recv();

  match receipt {
    Ok(()) => Ok(()),
    Err(_writer_exited_before_acknowledging) => Ok(()),
  }
}

fn writer() -> Result<Option<&'static SyncSender<Message>>, Never> {
  #[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
      explicit044_no_ambient_value,
      reason = "one thread per process holds the file open for the life of it, which is what this crate's head argues for: the queue is what every timed thread hands a line to, and a sender held by any one of them would be a sender the next one cannot find"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit068_no_global_mutable_state",
    expect(
      explicit068_no_global_mutable_state,
      reason = "one immutable sender points to the single process writer; only that thread owns or modifies the journal file"
    )
  )]
  static WRITER: OnceLock<Option<SyncSender<Message>>> = OnceLock::new();

  let held = WRITER.get_or_init(|| {
    let Ok(started) = start();

    started
  });

  Ok(held.as_ref())
}

fn start() -> Result<Option<SyncSender<Message>>, Never> {
  let Ok(queue) = index(QUEUE);
  let (say, heard) = sync_channel(queue);

  let Ok(at) = where_();

  let at_3 = match at {
    Some(at_2) => at_2,
    None => return Ok(None),
  };

  let started = std::thread::Builder::new()
    .name("wait-times".to_string())
    .spawn(move || keep(&heard, &at_3, measuring::current));

  match started {
    Ok(_) => Ok(Some(say)),
    Err(fault) => {
      eprintln!("console-response-times: no thread to write with: {fault}");

      Ok(None)
    }
  }
}

fn keep(
  heard: &Receiver<Message>,
  at: &Path,
  asked: fn() -> Result<Measuring, Never>,
) -> Result<(), Never> {
  let mut store: Option<Store> = None;

  #[cfg_attr(
    dylint_lib = "explicit055_no_unbounded_loop",
    expect(
      explicit055_no_unbounded_loop,
      reason = "the dedicated background writer accepts events for the lifetime of the process and stops when its senders disconnect"
    )
  )]
  'messages: for told in heard {
    match told {
      Message::Line(said) => {
        let Ok(chosen) = asked();

        match chosen {
          Measuring::On => {
            let Ok(held) = append(store, at, &said);

            store = held;
          }
          Measuring::Off => store = None,
        }
      }
      Message::Flush(told_2) => match told_2.send(()) {
        Ok(()) => {}
        Err(_flusher_disconnected) => continue 'messages,
      },
    }
  }

  Ok(())
}

struct Store {
  file: File,
  long: u64,
}

fn append(store: Option<Store>, at: &Path, said: &str) -> Result<Option<Store>, Never> {
  let mut store_3 = match store {
    Some(store_2) => store_2,
    None => {
      let Ok(held) = opened(at);

      match held {
        Some(held_2) => held_2,
        None => return Ok(None),
      }
    }
  };

  match store_3.file.write_all(said.as_bytes()) {
    Ok(()) => {}
    Err(fault) => {
      eprintln!("console-response-times: {}: {fault}", at.display());

      return Ok(None);
    }
  }

  let Ok(long) = fitted(said.len());

  store_3.long = store_3.long.saturating_add(long);

  match ::core::primitive::bool::then_some(store_3.long >= CAP, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(()) = set_aside(at);

      Ok(None)
    }
    ::core::option::Option::None => Ok(Some(store_3)),
  }
}

fn opened(at: &Path) -> Result<Option<Store>, Never> {
  match at.parent() {
    Some(above) => match std::fs::create_dir_all(above) {
      Ok(()) => {}
      Err(fault) => {
        eprintln!("console-response-times: {}: {fault}", above.display());

        return Ok(None);
      }
    },
    None => {}
  }

  #[cfg_attr(
    dylint_lib = "explicit040_no_torn_write",
    allow(
      explicit040_no_torn_write,
      reason = "a log appended to a line at a time, opened once and held for the life of the process: a rename over it would take away every line the other processes are still appending to"
    )
  )]
  let file = match OpenOptions::new().create(true).append(true).open(at) {
    Ok(file) => file,
    Err(fault) => {
      eprintln!("console-response-times: {}: {fault}", at.display());

      return Ok(None);
    }
  };
  let long = match file.metadata() {
    Ok(about) => about.len(),
    Err(fault) => {
      eprintln!(
        "console-response-times: {}: how long it already is would not be said: {fault}. The \
                 ten gigabytes are counted from here instead",
        at.display()
      );

      0
    }
  };

  Ok(Some(Store { file, long }))
}

fn set_aside(at: &Path) -> Result<(), Never> {
  match std::fs::rename(at, at.with_extension("jsonl.old")) {
    Ok(()) => {}
    Err(fault) => eprintln!(
      "console-response-times: cannot rotate {}: {fault}",
      at.display()
    ),
  }

  Ok(())
}

fn by_hand(said: &str) -> Result<(), Never> {
  let Ok(chosen) = measuring::current();

  match chosen {
    Measuring::On => {}
    Measuring::Off => return Ok(()),
  }

  let Ok(at) = where_();

  let at_3 = match at {
    Some(at_2) => at_2,
    None => return Ok(()),
  };

  let Ok(held) = opened(&at_3);

  let mut store = match held {
    Some(store) => store,
    None => return Ok(()),
  };

  match store.file.write_all(said.as_bytes()) {
    Ok(()) => Ok(()),
    Err(fault) => {
      eprintln!("console-response-times: {}: {fault}", at_3.display());

      Ok(())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;
  use std::io::{self, ErrorKind};
  use std::path::PathBuf;
  use std::thread::JoinHandle;
  use std::time::Duration;

  const WAIT_FOR_WRITER: Duration = Duration::from_secs(5);

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the tests close their only sender after the writer acknowledges the queue, so the receive loop exits and the join collects its result"
    )
  )]
  fn joined(writer: JoinHandle<Result<(), Never>>) -> io::Result<()> {
    let result = writer
      .join()
      .map_err(|_writer_panicked| io::Error::other("the journal writer panicked"))?;
    let Ok(()) = result;

    Ok(())
  }

  fn somewhere(named: &str) -> Result<PathBuf, Box<dyn Error>> {
    let folder = console_core_temporary_directories::fresh(&format!("waited-{named}"))?;

    Ok(folder.join("waited.jsonl"))
  }

  fn on() -> Result<Measuring, Never> {
    Ok(Measuring::On)
  }

  fn off() -> Result<Measuring, Never> {
    Ok(Measuring::Off)
  }

  #[test]
  fn the_file_is_opened_once_and_held_for_every_line_after_the_first()
  -> Result<(), Box<dyn Error>> {
    let at = somewhere("held")?;
    let Ok(store) = append(None, &at, "one\n");

    assert!(store.is_some(), "the first line did not open the file");

    let Ok(after_second_line) = append(store, &at, "two\n");
    let written = std::fs::read_to_string(&at)?;

    assert!(
      after_second_line.is_some(),
      "the second line let go of the file"
    );
    assert_eq!(written, "one\ntwo\n");

    std::fs::remove_file(&at)?;

    Ok(())
  }

  #[test]
  fn a_line_written_by_hand_lands_where_the_thread_would_have_put_it()
  -> Result<(), Box<dyn Error>> {
    let at = somewhere("by-hand")?;
    let Ok(store) = append(None, &at, "written on the thread that timed it\n");
    let written = std::fs::read_to_string(&at)?;

    assert!(store.is_some());
    assert!(written.ends_with('\n'));

    std::fs::remove_file(&at)?;

    Ok(())
  }

  #[test]
  fn the_thread_writes_what_it_was_given_and_answers_once_the_queue_is_empty()
  -> Result<(), Box<dyn Error>> {
    let at = somewhere("queue")?;
    let (say, heard) = sync_channel(8);
    let there = at.clone();
    let thread = std::thread::spawn(move || keep(&heard, &there, on));
    let (told, back) = sync_channel(0);

    say
      .send(Message::Line("one\n".to_string()))
      .map_err(|_| "the queue had no room in it")?;
    say
      .send(Message::Flush(told))
      .map_err(|_| "the queue had no room in it")?;
    back
      .recv_timeout(WAIT_FOR_WRITER)
      .map_err(|_| "the thread never said it had caught up")?;

    let written = std::fs::read_to_string(&at)?;

    assert_eq!(written, "one\n");

    drop(say);

    joined(thread)?;
    std::fs::remove_file(&at)?;

    Ok(())
  }

  #[test]
  fn with_measuring_off_the_thread_writes_nothing_and_still_answers() -> Result<(), Box<dyn Error>>
  {
    let at = somewhere("off")?;
    let (say, heard) = sync_channel(8);
    let there = at.clone();
    let thread = std::thread::spawn(move || keep(&heard, &there, off));
    let (told, back) = sync_channel(0);

    say
      .send(Message::Line("one\n".to_string()))
      .map_err(|_| "the queue had no room in it")?;
    say
      .send(Message::Flush(told))
      .map_err(|_| "the queue had no room in it")?;
    back
      .recv_timeout(WAIT_FOR_WRITER)
      .map_err(|_| "the thread never said it had caught up")?;

    let absent = match std::fs::metadata(&at) {
      Ok(_unexpected_journal) => false,
      Err(fault) => fault.kind() == ErrorKind::NotFound,
    };
    assert!(absent, "a line was written with measuring off");

    drop(say);

    joined(thread)?;

    Ok(())
  }

  #[test]
  fn waiting_for_a_queue_nothing_was_put_on_returns() {
    let Ok(()) = flush();
    let Ok(()) = flush();
  }
}

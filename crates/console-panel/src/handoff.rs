//! Asking the program that holds the panels to put one up.
//!
//! Three days of the device's own timing file said the same thing every day:
//! the two largest stretches of an opening are `exec` and the toolkit coming
//! up, and everything a card does after that is a fraction of either. Neither
//! is work about the panel. They are the cost of starting from nothing, paid
//! again every time someone presses a button, on a machine that spent the
//! moment before asleep.
//!
//! So one program keeps the toolkit up and draws whichever panel is asked for,
//! and this is how it is asked. What is on the other side of the socket has
//! already opened the display, parsed the stylesheet and read the icon theme,
//! and none of that happens twice.
//!
//! **The program that asks is still the program that holds the screen.** The
//! one-picker lock is a `flock` on a file held open for as long as a process
//! lives, and it is released by the kernel however that process ends -- which
//! is what lets a picker that was killed outright leave nothing behind. A
//! host that outlived every panel would be a host that never let the lock go.
//! So the panel's own program stays: it takes the lock, asks for the surface,
//! and waits with nothing to draw until the surface is gone. Every road in --
//! the bar's `on-click`, a `.desktop` file, the daemon's table, a key bound in
//! the compositor -- reaches the same program under the same name, and what
//! `hyprctl` reports as being on the screen is the same namespace as before.
//!
//! **The host could hold the lock, and the lock is not what keeps the program
//! that asks.** A `flock` the host took for one panel would be let go on every
//! road out: a panel closed is a `close` the host answers, a drawing that fails
//! is the pipe the host already watches for the surface ending, and a host that
//! is killed is a process ending, which the kernel answers for a host exactly as
//! it does for a panel. What the file holds beside the lock is a pid, and the
//! pid is the half everything else uses: a picker coming asks the one there to
//! go with a SIGTERM and takes it off the screen with a SIGKILL when it will
//! not, and the paddle's `console-put-away` is that SIGTERM and nothing more.
//! Held by the host, every one of those lands on the process holding every
//! panel -- a panel put away is the host told to stop, and one slow to go is the
//! host killed. Teaching the host that a SIGTERM means "put away what is up" is
//! a host that no longer stops the way the service manager stops it, and the
//! bar and the daemon, which only start a program today, would each have to be
//! told whether their press opened something or closed it, because a door that
//! only opens and a second press that closes are the holder's to decide. All
//! of that would buy the asking program's own start, which measured against
//! `socat` knocking on the same socket is the loader and a few reads -- small
//! beside the card being built and drawn, which no road in skips.
//!
//! **A panel with no host is merely slower.** This is a daemon that can be
//! down, and nothing here may be written as though it cannot be: a socket that
//! refuses is a panel that draws in its own process, exactly as it did before
//! any of this, and says so on the journal rather than on the screen.
//!
//! **What the lock file says is said by the one holding it.** `picker::mark_drawn`
//! and `picker::gone` write into the lock this process opened, and the host
//! has no such lock -- called there they would be two no-ops, and the door name
//! they write is what `alone` reads to decide that a door which only opens has
//! been asked twice. So the host says which of the two has happened and the
//! stand-in writes it, which is the same order it happened in when a panel was
//! one process.
//!
//! Acceptance and drawing are separate replies. Acceptance is sent before the
//! card is built; Drawn follows the compositor's presentation acknowledgement.
//! A failed drawing says Failed, and the stand-in draws locally. The request's
//! completion file, when requested, records those replies as they arrive.
//!
//! Lines, and no version number, for `console_events::wire`'s reasons: both
//! ends are built by the same `console apply` from the same commit, and someone
//! holding `socat` against the socket should be able to read what went past.
//! What is escaped here is not what is escaped there -- an event group is one
//! token and a space is what ends it, and these are a program's arguments,
//! which are allowed to hold anything at all except a nul.

use std::io::{BufRead, BufReader, Write};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Duration;

use console_core_iteration::Step;
use console_core_never::Never;

pub const SOCKET: &str = "panels.sock";

pub const OPEN: &str = "open";

pub const CLOSE: &str = "close";

pub use crate::completion::{ACCEPTED, DRAWN, FAILED, GONE};

const NOTHING: &str = "-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
  pub who: String,
  pub from: String,
  pub pressed: Option<String>,
  pub exec: Duration,
  pub tells: Option<String>,
  pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawnBy {
  ByTheHost,
  Here,
}

pub fn where_() -> Result<Option<PathBuf>, Never> {
  let ours = console_core_places::application_runtime()?;

  Ok(ours.map(|ours_2| ours_2.join(SOCKET)))
}

pub fn serialize(asked: &Request) -> Result<String, Never> {
  let mut said = String::from(OPEN);

  for word in [&asked.who, &asked.from] {
    let Ok(word_2) = escaped(word);

    said.push(' ');
    said.push_str(&word_2);
  }

  let stamp = match asked.pressed.clone() {
    Some(stamp) => stamp,
    None => NOTHING.to_string(),
  };
  let Ok(stamp_2) = escaped(&stamp);

  said.push(' ');
  said.push_str(&stamp_2);
  said.push(' ');
  said.push_str(&asked.exec.as_nanos().to_string());

  let tells = match asked.tells.clone() {
    Some(tells) => tells,
    None => NOTHING.to_string(),
  };
  let Ok(tells_2) = escaped(&tells);

  said.push(' ');
  said.push_str(&tells_2);

  for word in &asked.arguments {
    let Ok(word_3) = escaped(word);

    said.push(' ');
    said.push_str(&word_3);
  }

  Ok(said)
}

pub fn read(line: &str) -> Result<Option<Request>, Never> {
  let mut words = line.split(' ');

  match words.next() {
    Some(OPEN) => {}
    Some(_) | None => return Ok(None),
  }

  let who = match words.next() {
    Some(who) => who,
    None => return Ok(None),
  };

  let from = match words.next() {
    Some(from) => from,
    None => return Ok(None),
  };

  let pressed = match words.next() {
    Some(pressed) => pressed,
    None => return Ok(None),
  };

  let exec = match words.next() {
    Some(exec) => exec,
    None => return Ok(None),
  };

  let tells = match words.next() {
    Some(tells) => tells,
    None => return Ok(None),
  };

  let Ok(exec_2) = took(exec);
  let Ok(who_2) = plain(who);
  let Ok(from_2) = plain(from);
  let Ok(stamp) = plain(pressed);
  let Ok(tells_2) = plain(tells);

  let arguments: Vec<String> = words
    .map(|word| {
      let Ok(word_2) = plain(word);

      word_2
    })
    .collect();

  match ::core::primitive::bool::then_some(who_2.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(None),
    ::core::option::Option::None => {}
  }

  Ok(Some(Request {
    who: who_2,
    from: from_2,
    pressed: match ::core::primitive::bool::then_some(stamp.is_empty(), ()) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => Some(stamp),
    },
    exec: exec_2,
    tells: match ::core::primitive::bool::then_some(tells_2.is_empty(), ()) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => Some(tells_2),
    },
    arguments,
  }))
}

fn took(said: &str) -> Result<Duration, Never> {
  Ok(match said.trim().parse::<u64>() {
    Ok(nanos) => Duration::from_nanos(nanos),
    Err(_not_a_number) => Duration::ZERO,
  })
}

fn escaped(said: &str) -> Result<String, Never> {
  let mut written = String::with_capacity(said.len());

  for letter in said.chars() {
    match letter {
      '\\' => written.push_str("\\\\"),
      ' ' => written.push_str("\\s"),
      '\n' => written.push_str("\\n"),
      other => written.push(other),
    }
  }

  match ::core::primitive::bool::then_some(written.is_empty(), ()) {
    ::core::option::Option::Some(()) => Ok(NOTHING.to_string()),
    ::core::option::Option::None => Ok(written),
  }
}

fn plain(said: &str) -> Result<String, Never> {
  match ::core::primitive::bool::then_some(said == NOTHING, ()) {
    ::core::option::Option::Some(()) => return Ok(String::new()),
    ::core::option::Option::None => {}
  }

  let written = console_core_iteration::iterate(
    (String::with_capacity(said.len()), said.chars()),
    |(mut written, mut letters)| {
      Ok(match letters.next() {
        Some('\\') => {
          match letters.next() {
            Some('\\') => written.push('\\'),
            Some('s') => written.push(' '),
            Some('n') => written.push('\n'),
            Some(other) => written.push(other),
            None => {}
          }

          Step::Again((written, letters))
        }
        Some(other) => {
          written.push(other);

          Step::Again((written, letters))
        }
        None => Step::Halt(written),
      })
    },
  );

  Ok(match written {
    Ok(written_2) => written_2,
    Err(_endless) => String::new(),
  })
}

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "a signal handler is handed nothing and may allocate nothing: the descriptor it writes `close` down has to be a number it can read with one load"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "the POSIX signal callback has no captured state and receives the current borrowed handoff descriptor through this atomic"
  )
)]
static TELLING: AtomicI32 = AtomicI32::new(-1);

#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "serializes a lock-free signal write and lets the socket owner await its completion before closing the descriptor"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  expect(
    explicit044_no_ambient_value,
    reason = "a POSIX callback must signal completion without a captured value while the process owner still holds the socket"
  )
)]
static WRITING: AtomicBool = AtomicBool::new(false);

pub fn stood_in(who: &str, arguments: &[String]) -> Result<DrawnBy, Never> {
  let Ok(where_) = where_();

  let Ok(drawn) = match where_ {
    Some(at) => stood_in_at(&at, who, arguments),
    None => {
      console_core_logging::warn!("{who}: XDG_RUNTIME_DIR names nothing, so there is no host to ask");

      Ok(DrawnBy::Here)
    }
  };

  match drawn {
    DrawnBy::Here => {
      let Ok(()) = crate::picker::taken_over();
    }
    DrawnBy::ByTheHost => {}
  }

  Ok(drawn)
}

pub fn stood_in_at(at: &Path, who: &str, arguments: &[String]) -> Result<DrawnBy, Never> {
  let Ok(completion) = crate::completion::where_to();

  request(at, who, arguments, |progress| {
    match crate::completion::record(completion.as_deref(), progress) {
      Ok(()) => {}
      Err(fault) => console_core_logging::warn!("{who}: the completion could not be recorded: {fault}"),
    }

    Ok(())
  })
}

pub fn request(
  at: &Path,
  who: &str,
  arguments: &[String],
  report: impl FnMut(crate::completion::Progress) -> Result<(), Never>,
) -> Result<DrawnBy, Never> {
  let asking = match UnixStream::connect(at) {
    Ok(asking) => asking,
    Err(fault) => {
      console_core_logging::warn!(
        "{who}: {}: {fault}, so this panel draws itself",
        at.display()
      );

      return Ok(DrawnBy::Here);
    }
  };

  let Ok(asked) = asking_for(who, arguments);
  let Ok(said) = serialize(&asked);

  let mut writing = match asking.try_clone() {
    Ok(writing) => writing,
    Err(fault) => {
      console_core_logging::warn!("{who}: {fault}, so this panel draws itself");

      return Ok(DrawnBy::Here);
    }
  };

  #[cfg_attr(
    dylint_lib = "explicit090_no_raw_descriptor",
    expect(
      explicit090_no_raw_descriptor,
      reason = "the signal handler can only receive the number; the socket owner remains alive until publication is withdrawn and the in-flight write finishes"
    )
  )]
  TELLING.store(writing.as_raw_fd(), Ordering::SeqCst);

  let Ok(()) = answers_being_asked_to_stop();

  match writeln!(writing, "{said}") {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::warn!("{who}: {fault}, so this panel draws itself");

      let Ok(()) = no_longer_answering();

      return Ok(DrawnBy::Here);
    }
  }

  let Ok(drawn) = read_reply(who, asking, report);
  let Ok(()) = no_longer_answering();

  Ok(drawn)
}

fn read_reply(
  who: &str,
  asking: UnixStream,
  mut report: impl FnMut(crate::completion::Progress) -> Result<(), Never>,
) -> Result<DrawnBy, Never> {
  let reading = BufReader::new(asking);
  let mut drawn = DrawnBy::Here;
  let mut accepted = false;

  for line in reading.lines() {
    let line_3 = match line {
      Ok(line_2) => line_2,
      Err(_the_read_failed) => break,
    };

    let Ok(progress) = crate::completion::Progress::read(line_3.trim());
    let progress_3 = match progress {
      Some(progress_2) => progress_2,
      None => {
        console_core_logging::warn!(
          "{who}: the host said {line:?}, which is nothing this knows",
          line = line_3
        );

        continue;
      }
    };

    match ::core::primitive::bool::then_some(progress_3 == crate::completion::Progress::Visible && !accepted, ()) {
      ::core::option::Option::Some(()) => {
        console_core_logging::warn!("{who}: the host claimed a drawing without accepting its request");

        return Ok(DrawnBy::Here);
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = report(progress_3);

    match progress_3 {
      crate::completion::Progress::Accepted => accepted = true,
      crate::completion::Progress::Visible => {
        let Ok(()) = crate::picker::mark_drawn();

        drawn = DrawnBy::ByTheHost;
      }
      crate::completion::Progress::Gone => {
        let Ok(()) = crate::picker::gone();

        return Ok(match ::core::primitive::bool::then_some(accepted, ()) {
          ::core::option::Option::Some(()) => DrawnBy::ByTheHost,
          ::core::option::Option::None => DrawnBy::Here,
        });
      }
      crate::completion::Progress::Failed => return Ok(DrawnBy::Here),
    }
  }

  Ok(match drawn {
    DrawnBy::ByTheHost => DrawnBy::ByTheHost,
    DrawnBy::Here => {
      console_core_logging::warn!("{who}: the host went away before it drew anything, so this panel does");

      DrawnBy::Here
    }
  })
}

fn asking_for(who: &str, arguments: &[String]) -> Result<Request, Never> {
  let Ok(pressed) = console_response_times::press_said();
  let Ok(from) = console_response_times::from_said();
  let Ok(exec) = console_response_times::since_exec();
  let Ok(tells) = crate::description::where_to();

  Ok(Request {
    who: who.to_string(),
    from,
    pressed,
    exec: match exec {
      Some(exec_2) => exec_2,
      None => std::time::Duration::ZERO,
    },
    tells,
    arguments: arguments.to_vec(),
  })
}

fn answers_being_asked_to_stop() -> Result<(), Never> {
  #[cfg_attr(
    dylint_lib = "explicit095_no_unsafe_in_a_handler",
    expect(
      explicit095_no_unsafe_in_a_handler,
      reason = "BorrowedFd::borrow_raw creates a stack-only wrapper; the writer release waits until the nonblocking send finishes"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the glibc signal registration requires an externally implemented handler; this handler acquires only atomics and sends one nonblocking message"
    )
  )]
  let answering = {
    // SAFETY: on_signal uses atomics and one nonblocking send on the process-owned socket.
    unsafe { console_signals::install_handler(&console_signals::STOPPING, on_signal) }
  };

  match answering {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("console-panel: {fault}"),
  }

  Ok(())
}

fn no_longer_answering() -> Result<(), Never> {
  TELLING.store(-1, Ordering::SeqCst);

  #[cfg_attr(
    dylint_lib = "explicit053_no_while",
    expect(
      explicit053_no_while,
      reason = "the only writer clears this atomic after a single nonblocking syscall; the socket owner must finish any in-flight raw descriptor borrow before closing the descriptor"
    )
  )]
  while WRITING.load(Ordering::SeqCst) {
    core::hint::spin_loop();
  }

  for which in console_signals::STOPPING {
    match console_signals::restore_default(which) {
      Ok(()) => {}
      Err(fault) => console_core_logging::warn!("console-panel: {fault}"),
    }
  }

  Ok(())
}

extern "C" fn on_signal(_number: core::ffi::c_int) {
  match WRITING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst) {
    Ok(_) => {}
    Err(_another_signal_is_sending) => return,
  }

  let fd = TELLING.load(Ordering::SeqCst);

  match ::core::primitive::bool::then_some(fd >= 0, ()) {
    ::core::option::Option::Some(()) => {
      #[cfg_attr(
        dylint_lib = "explicit058_contain_unsafe",
        expect(explicit058_contain_unsafe, reason = "the POSIX signal handler borrows the descriptor already owned by request; withdrawal waits for WRITING before returning ownership")
      )]
      // SAFETY: request keeps the socket open until the current WRITING operation clears its flag.
      let held = unsafe { BorrowedFd::borrow_raw(fd) };
      let flags = rustix::net::SendFlags::DONTWAIT | rustix::net::SendFlags::NOSIGNAL;
      let _a_gone_host_no_longer_needs_the_close = rustix::net::send(held, b"close\n", flags);
    }
    ::core::option::Option::None => {}
  }

  WRITING.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
  use super::*;

  fn sample_request(arguments: &[&str]) -> Result<Request, Never> {
    Ok(Request {
      who: "launcher".to_string(),
      from: "bar".to_string(),
      pressed: Some("12345".to_string()),
      exec: Duration::from_millis(3),
      tells: None,
      arguments: arguments.iter().map(|word| (*word).to_string()).collect(),
    })
  }

  #[test]
  fn what_was_asked_for_is_what_arrives() {
    let Ok(asking) = sample_request(&["--place", "2.1.3"]);
    let Ok(said) = serialize(&asking);

    assert_eq!(read(&said), Ok(Some(asking)));
  }

  #[test]
  fn where_the_asker_wants_the_panel_to_say_what_it_drew_reaches_the_host() {
    let Ok(general) = sample_request(&["General"]);
    let asking = Request {
      tells: Some("/run/user/1000/told here.jsonl".to_string()),
      ..general
    };
    let Ok(said) = serialize(&asking);

    assert_eq!(read(&said), Ok(Some(asking)));
  }

  #[test]
  fn an_argument_with_a_space_in_it_is_still_one_argument() {
    let Ok(asking) = sample_request(&["Game Mode"]);
    let Ok(said) = serialize(&asking);
    let Ok(back) = read(&said);

    assert_eq!(
      back.map(|back_2| back_2.arguments),
      Some(vec!["Game Mode".to_string()])
    );
  }

  #[test]
  fn a_path_holding_the_marks_that_spell_the_line_survives_it() {
    let awkward = "/home/someone/a folder\\with a mark/and a\nline";
    let Ok(asking) = sample_request(&[awkward]);
    let Ok(said) = serialize(&asking);

    assert!(
      !said.contains('\n'),
      "a request is one line or it is two requests"
    );
    let Ok(back) = read(&said);

    assert_eq!(
      back.map(|back_2| back_2.arguments),
      Some(vec![awkward.to_string()])
    );
  }

  #[test]
  fn an_argument_that_says_nothing_is_still_an_argument() {
    let Ok(asking) = sample_request(&["", "Sound"]);
    let Ok(said) = serialize(&asking);
    let Ok(back) = read(&said);

    assert_eq!(
      back.map(|back_2| back_2.arguments),
      Some(vec![String::new(), "Sound".to_string()]),
      "an empty word dropped would shift every argument after it"
    );
  }

  #[test]
  fn a_panel_no_one_pressed_says_so_rather_than_saying_nought() {
    let Ok(bare) = sample_request(&[]);
    let asking = Request {
      pressed: None,
      from: String::new(),
      ..bare
    };
    let Ok(said) = serialize(&asking);

    assert_eq!(
      read(&said),
      Ok(Some(asking)),
      "a stamp of zero is a wait that ended long ago"
    );
  }

  #[test]
  fn a_line_that_is_not_a_request_is_not_read_as_one() {
    assert_eq!(read(""), Ok(None));
    assert_eq!(read("shut launcher"), Ok(None));
    assert_eq!(read("open"), Ok(None));
    assert_eq!(read("open launcher"), Ok(None));
    assert_eq!(read("open launcher bar 12345"), Ok(None));
    assert_eq!(
      read("open - bar 12345 0"),
      Ok(None),
      "a panel with no name is no panel"
    );
  }

  #[test]
  fn the_socket_is_beside_the_pools() {
    let Ok(where_) = where_();

    match where_ {
      Some(at) => {
        assert!(at.ends_with("console/panels.sock"), "{}", at.display());
      }
      None => {}
    }
  }
}

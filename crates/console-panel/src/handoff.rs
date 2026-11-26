//! The two ways to ask the resident panel host for an opening.
//!
//! A stand-in process takes the picker lease, sends a request and holds a
//! connection until the surface closes. Acceptance and compositor feedback
//! are separate replies. A failed handoff lets the stand-in draw locally.
//!
//! A resident input source sends a one-shot request on the resident socket.
//! The host takes the picker lease and owns the surface until dismissal. The
//! same lease marker tells conventional pickers and console-put-away to ask
//! the host to close its surface through the control socket.
//!
//! Requests and replies are newline-delimited. Arguments preserve spaces,
//! backslashes, and newlines through the wire escaping below.

use std::io::{self, BufReader, Write};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::{Duration, Instant};

use console_core_bounded_reads::{Line, LineEnding, LinePart};
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::fixed_index;
use console_operating_system_linux::unix_stream::{Within, connect, receive_line};

pub const SOCKET: &str = "panels.sock";

pub const RESIDENT_SOCKET: &str = "panels-resident.sock";

pub const OPEN: &str = "open";

pub const CLOSE: &str = "close";

pub const REQUEST_BYTES: u32 = 128 * 1024;
const REPLY_BYTES: u32 = 256;
const FRAME_TIME: Duration = Duration::from_secs(1);
const ACK_TIME: Duration = Duration::from_secs(2);

const RESIDENT: &str = "resident ";
const DISMISS: &str = "dismiss";

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

pub enum HostRequest {
  StandIn(Request),
  Resident(Request),
  Dismiss,
}

pub fn read_host(line: &str) -> Result<Option<HostRequest>, Never> {
  match line.strip_prefix(RESIDENT) {
    Some(said) => {
      let Ok(request) = read(said);

      Ok(request.map(HostRequest::Resident))
    }
    None => match (line == DISMISS).then_some(()) {
      Some(()) => Ok(Some(HostRequest::Dismiss)),
      None => {
        let Ok(request) = read(line);

        Ok(request.map(HostRequest::StandIn))
      }
    },
  }
}

pub fn open_resident((who, from): (&str, &str), arguments: &[String]) -> std::io::Result<()> {
  let Ok(pressed) = console_response_times::press_stamp();
  let Ok(tells) = crate::description::where_to();
  let Ok(said) = serialize(&Request {
    who: who.to_owned(),
    from: from.to_owned(),
    pressed: pressed.map(|(_, stamp)| stamp),
    exec: Duration::ZERO,
    tells,
    arguments: arguments.to_vec(),
  });

  send_to_host(&format!("{RESIDENT}{said}\n"))
}

pub(crate) fn dismiss_resident() -> std::io::Result<()> {
  send_to_host(concat!("dismiss", "\n"))
}

fn send_to_host(packet: &str) -> std::io::Result<()> {
  use rustix::net::{
    AddressFamily, SendFlags, SocketAddrUnix, SocketFlags, SocketType, connect, send, socket_with,
  };

  check_request(packet)?;
  let Ok(path) = resident_where();
  let at = path.ok_or(std::io::ErrorKind::NotFound)?;
  let socket = socket_with(
    AddressFamily::UNIX,
    SocketType::STREAM,
    SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
    None,
  )?;
  let address = SocketAddrUnix::new(at)?;
  connect(&socket, &address)?;
  let written = send(&socket, packet.as_bytes(), SendFlags::NOSIGNAL).map(|bytes| {
    let Ok(count) = console_core_number_conversion::fitted::<_, u64>(bytes);

    count
  })?;
  let Ok(length) = console_core_number_conversion::fitted::<_, u64>(packet.len());

  match (written == length).then_some(()) {
    Some(()) => Ok(()),
    None => Err(std::io::Error::from(std::io::ErrorKind::WriteZero)),
  }
}

fn check_request(frame: &str) -> io::Result<()> {
  let Ok(length) = fixed_index(frame.len());

  match (length <= u64::from(REQUEST_BYTES)).then_some(()) {
    Some(()) => Ok(()),
    None => Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      "panel request exceeds its byte limit",
    )),
  }
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

pub fn resident_where() -> Result<Option<PathBuf>, Never> {
  let ours = console_core_places::application_runtime()?;

  Ok(ours.map(|directory| directory.join(RESIDENT_SOCKET)))
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
      console_core_logging::warn!(
        "{who}: XDG_RUNTIME_DIR names nothing, so there is no host to ask"
      );

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
      Err(fault) => {
        console_core_logging::warn!("{who}: the completion could not be recorded: {fault}")
      }
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
  let asking = match connect(at) {
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

  let writing = match asking.try_clone() {
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

  let frame = format!("{said}\n");
  let sent = check_request(&frame)
    .and_then(|()| Within::new(&writing, FRAME_TIME))
    .and_then(|mut stream| stream.write_all(frame.as_bytes()));

  match sent {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::warn!("{who}: {fault}, so this panel draws itself");

      let Ok(()) = no_longer_answering();

      return Ok(DrawnBy::Here);
    }
  }

  #[cfg_attr(
    all(not(test), dylint_lib = "explicit039_no_reading_the_clock"),
    expect(
      explicit039_no_reading_the_clock,
      reason = "sending the request is the socket boundary that starts the acknowledgement timeout"
    )
  )]
  let deadline = Instant::now().checked_add(ACK_TIME);
  let Ok(drawn) = read_reply(who, asking, deadline, report);
  let Ok(()) = no_longer_answering();

  Ok(drawn)
}

fn read_reply(
  who: &str,
  asking: UnixStream,
  mut deadline: Option<Instant>,
  mut report: impl FnMut(crate::completion::Progress) -> Result<(), Never>,
) -> Result<DrawnBy, Never> {
  let mut reading = BufReader::new(asking);
  let Ok(mut frame) = Line::new(REPLY_BYTES, LineEnding::Required);
  let mut drawn = DrawnBy::Here;
  let mut accepted = false;

  #[cfg_attr(
    dylint_lib = "explicit054_no_loop",
    expect(
      explicit054_no_loop,
      reason = "a panel may remain open as long as its owner needs it; EOF and completion end the handoff"
    )
  )]
  loop {
    match receive_line(&mut reading, &mut frame, deadline, FRAME_TIME) {
      Ok(LinePart::Complete) => {}
      Ok(LinePart::Partial | LinePart::End) => break,
      Err(fault) => {
        console_core_logging::debug!("{who}: reading panel progress: {fault}");

        break;
      }
    }

    let Ok(bytes) = frame.bytes();
    let line = match std::str::from_utf8(bytes) {
      Ok(line) => line,
      Err(_invalid_utf8) => break,
    };

    let Ok(progress) = crate::completion::Progress::read(line.trim());
    let progress_3 = match progress {
      Some(progress_2) => progress_2,
      None => {
        console_core_logging::warn!("{who}: the host said {line:?}, which is nothing this knows");

        continue;
      }
    };

    match ::core::primitive::bool::then_some(
      progress_3 == crate::completion::Progress::Visible && !accepted,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        console_core_logging::warn!(
          "{who}: the host claimed a drawing without accepting its request"
        );

        return Ok(DrawnBy::Here);
      }
      ::core::option::Option::None => {}
    }

    let Ok(()) = report(progress_3);

    match progress_3 {
      crate::completion::Progress::Accepted => {
        accepted = true;
        deadline = None;
      }
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
      console_core_logging::warn!(
        "{who}: the host went away before it drew anything, so this panel does"
      );

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
        expect(
          explicit058_contain_unsafe,
          reason = "the POSIX signal handler borrows the descriptor already owned by request; withdrawal waits for WRITING before returning ownership"
        )
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

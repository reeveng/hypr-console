//! What the greeter and the login window say to each other, a line each.
//!
//! greetd's greeters speak JSON over a socket because greetd has to serve
//! greeters it has never met. This one has met its greeter: a line is a word
//! and what follows it. A pattern is letters
//! and nothing else, so a line that says `login` with anything else after it
//! is not a login, and nothing a module says can end a line early because a
//! newline in it is written as a space.

use std::io::{self, BufReader, Read};
use std::os::fd::{AsFd, BorrowedFd};

use console_core_bounded_reads::{Line, LineEnding, LinePart, ReadError};
use console_core_never::Never;
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};

pub const REQUEST_BYTES: u32 = 32;
pub const REPLY_BYTES: u32 = 8_192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Buffered {
  Ready,
  Empty,
}

pub struct Reader<R> {
  input: BufReader<R>,
  frame: Line,
}

impl<R: Read + AsFd> Reader<R> {
  pub fn new(pipe: R, limit: u32) -> io::Result<Self> {
    let flags = fcntl_getfl(&pipe)?;
    fcntl_setfl(&pipe, flags | OFlags::NONBLOCK)?;
    let Ok(frame) = Line::new(limit, LineEnding::Required);

    Ok(Self {
      input: BufReader::new(pipe),
      frame,
    })
  }

  pub fn buffered(&self) -> Result<Buffered, Never> {
    Ok(match self.input.buffer().is_empty().then_some(()) {
      Some(()) => Buffered::Empty,
      None => Buffered::Ready,
    })
  }

  pub fn read(&mut self) -> io::Result<Option<String>> {
    match self.frame.read_part(&mut self.input) {
      Ok(LinePart::Complete) => {
        let Ok(bytes) = self.frame.bytes();
        let line = std::str::from_utf8(bytes).map_err(io::Error::other)?;
        let text = line.to_owned();
        let Ok(()) = self.frame.clear();

        Ok(Some(text))
      }
      Ok(LinePart::Partial) => Ok(None),
      Ok(LinePart::End) => Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
      Err(ReadError::Io(fault)) => match (fault.kind() == io::ErrorKind::WouldBlock
        || fault.kind() == io::ErrorKind::Interrupted)
        .then_some(())
      {
        Some(()) => Ok(None),
        None => Err(fault),
      },
      Err(fault @ (ReadError::TooLong { .. } | ReadError::NotUtf8(_))) => {
        Err(io::Error::from(fault))
      }
    }
  }
}

impl<R: Read + AsFd> AsFd for Reader<R> {
  fn as_fd(&self) -> BorrowedFd<'_> {
    self.input.get_ref().as_fd()
  }
}

#[derive(Clone, PartialEq, Eq)]
pub enum FromGreeter {
  Login(String),
  Fingerprint,
  Terminal,
}

impl std::fmt::Debug for FromGreeter {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      FromGreeter::Login(_) => write!(to, "Login(..)"),
      FromGreeter::Fingerprint => write!(to, "Fingerprint"),
      FromGreeter::Terminal => write!(to, "Terminal"),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToGreeter {
  Failed(String),
  FingerprintAvailable,
  FingerprintFailed,
  Message(String),
  Welcome,
}

impl From<crate::fingerprint::Event> for ToGreeter {
  fn from(event: crate::fingerprint::Event) -> Self {
    match event {
      crate::fingerprint::Event::Available => Self::FingerprintAvailable,
      crate::fingerprint::Event::Accepted => Self::Welcome,
      crate::fingerprint::Event::Refused => Self::FingerprintFailed,
    }
  }
}

const LOGIN: &str = "login";
const FINGERPRINT: &str = "fingerprint";
const FINGERPRINT_AVAILABLE: &str = "fingerprint-available";
const FINGERPRINT_FAILED: &str = "fingerprint-failed";
const TERMINAL: &str = "terminal";
const FAILED: &str = "failed";
const MESSAGE: &str = "message";
const WELCOME: &str = "welcome";

pub fn from_greeter(line: &str) -> Result<Option<FromGreeter>, Never> {
  let line_2 = line.trim_end_matches('\n');
  let (word, rest) = match line_2.split_once(' ') {
    Some((word, rest)) => (word, Some(rest)),
    None => (line_2, None),
  };

  Ok(match (word, rest) {
    (LOGIN, Some(secret)) => {
      let letters =
        !secret.is_empty() && secret.chars().all(|letter| letter.is_ascii_alphanumeric());

      match ::core::primitive::bool::then_some(letters, ()) {
        ::core::option::Option::Some(()) => Some(FromGreeter::Login(secret.to_string())),
        ::core::option::Option::None => None,
      }
    }
    (FINGERPRINT, None) => Some(FromGreeter::Fingerprint),
    (TERMINAL, None) => Some(FromGreeter::Terminal),
    _ => None,
  })
}

pub fn to_greeter(line: &str) -> Result<Option<ToGreeter>, Never> {
  let line_2 = line.trim_end_matches('\n');
  let (word, rest) = match line_2.split_once(' ') {
    Some((word, rest)) => (word, rest),
    None => (line_2, ""),
  };

  Ok(match (word, rest) {
    (FAILED, reason) => Some(ToGreeter::Failed(reason.to_string())),
    (FINGERPRINT_AVAILABLE, "") => Some(ToGreeter::FingerprintAvailable),
    (FINGERPRINT_FAILED, "") => Some(ToGreeter::FingerprintFailed),
    (MESSAGE, text) => Some(ToGreeter::Message(text.to_string())),
    (WELCOME, _) => Some(ToGreeter::Welcome),
    (_, _) => None,
  })
}

pub fn line_from_greeter(message: &FromGreeter) -> Result<String, Never> {
  Ok(match message {
    FromGreeter::Login(secret) => format!("{LOGIN} {secret}\n"),
    FromGreeter::Fingerprint => format!("{FINGERPRINT}\n"),
    FromGreeter::Terminal => format!("{TERMINAL}\n"),
  })
}

pub fn line_to_greeter(message: &ToGreeter) -> Result<String, Never> {
  let flat = |text: &str| text.replace('\n', " ");

  Ok(match message {
    ToGreeter::Failed(why) => format!("{FAILED} {}\n", flat(why)),
    ToGreeter::FingerprintAvailable => format!("{FINGERPRINT_AVAILABLE}\n"),
    ToGreeter::FingerprintFailed => format!("{FINGERPRINT_FAILED}\n"),
    ToGreeter::Message(text) => format!("{MESSAGE} {}\n", flat(text)),
    ToGreeter::Welcome => format!("{WELCOME}\n"),
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Write;

  #[test]
  fn a_partial_greeter_request_survives_an_empty_poll_turn() -> io::Result<()> {
    let (from_greeter, to_window) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;
    let mut reader = Reader::new(std::fs::File::from(from_greeter), REQUEST_BYTES)?;
    let mut writer = std::fs::File::from(to_window);

    writer.write_all(b"ter")?;
    let first_part = reader.read()?;
    let wait = reader.read()?;
    assert_eq!(first_part, None);
    assert_eq!(wait, None);

    writer.write_all(b"minal\nfingerprint\n")?;
    let first = reader.read()?;
    let Ok(buffered) = reader.buffered();
    let second = reader.read()?;
    assert_eq!(first.as_deref(), Some("terminal"));
    assert_eq!(buffered, Buffered::Ready);
    assert_eq!(second.as_deref(), Some("fingerprint"));

    Ok(())
  }

  #[test]
  fn every_line_reads_back_as_what_was_written() {
    let from = [
      FromGreeter::Login("tat".to_string()),
      FromGreeter::Fingerprint,
      FromGreeter::Terminal,
    ];
    let to = [
      ToGreeter::Failed("that is not the pattern".to_string()),
      ToGreeter::FingerprintAvailable,
      ToGreeter::FingerprintFailed,
      ToGreeter::Message("hello".to_string()),
      ToGreeter::Welcome,
    ];

    for message in from {
      let Ok(line) = line_from_greeter(&message);
      let Ok(read) = from_greeter(&line);

      assert_eq!(read, Some(message));
    }

    for message in to {
      let Ok(line) = line_to_greeter(&message);
      let Ok(read) = to_greeter(&line);

      assert_eq!(read, Some(message));
    }
  }

  #[test]
  fn a_login_that_is_not_letters_is_not_a_login() {
    for line in [
      "login",
      "login ",
      "login tat tat",
      "login t;t",
      "login tat\ttat",
    ] {
      let Ok(read) = from_greeter(line);

      assert_eq!(read, None, "{line:?}");
    }
  }

  #[test]
  fn a_module_cannot_end_a_line_early() {
    let Ok(line) = line_to_greeter(&ToGreeter::Message("one\nwelcome".to_string()));

    assert_eq!(line.matches('\n').count(), 1);
  }

  #[test]
  fn a_login_never_prints_its_pattern() {
    assert_eq!(
      format!("{:?}", FromGreeter::Login("tat".to_string())),
      "Login(..)"
    );
  }
}

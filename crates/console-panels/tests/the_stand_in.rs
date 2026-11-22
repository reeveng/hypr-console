//! What a panel does when it has asked someone else to draw it.
//!
//! The socket is between processes and this is not two processes, which is a
//! fair thing to hold against a test. What it is here to hold shut is the half
//! that has nothing to do with drawing: a request that reaches the host says
//! what was asked for, a host that answers means the panel does not draw, and
//! every way of the host not answering means the panel does. That last one is
//! the whole of `a panel with no host is merely slower`, and it is the line
//! most likely to be got wrong, because it is the line no one sees working.
//!
//! The screen is where the rest of it is proved. `console-check` opens the menu
//! against a nested desktop and reads back what it drew, and that check does not
//! know or care which side of this socket drew it -- which is the point.

use std::error::Error;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::Duration;

use console_core_never::Never;

use console_panel::handoff::{self, DrawnBy, Request};
use console_waiting::clock::LiveClock;
use console_waiting::{Schedule, until_some_handed_on_result};

const PATIENCE: Duration = Duration::from_secs(5);

const DRAWN_AND_GONE: &[u8] = b"accepted\ndrawn\ngone\n";

fn somewhere(named: &str) -> Result<PathBuf, console_core_temporary_directories::Unmade> {
  let at = console_core_temporary_directories::fresh(&format!("panels-{named}"))?;

  Ok(at.join("host.sock"))
}

fn arguments(words: &[&str]) -> Result<Vec<String>, Never> {
  Ok(words.iter().map(|word| (*word).to_string()).collect())
}

struct Host {
  at: PathBuf,
  heard: Receiver<io::Result<Request>>,
}

impl Host {
  fn request(&self) -> Result<Request, Box<dyn Error>> {
    let reported = self.heard.recv_timeout(PATIENCE)?;
    let request = reported?;

    Ok(request)
  }
}

impl Drop for Host {
  fn drop(&mut self) {
    match std::fs::remove_file(&self.at) {
      Ok(()) => {}
      Err(fault) => match bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
        Some(()) => {}
        None => console_core_logging::warn!("removing the test panel socket: {fault}"),
      },
    }
  }
}

fn a_host(named: &str, answer: &'static [u8]) -> Result<Host, Box<dyn Error>> {
  let at = somewhere(named)?;
  let listening = UnixListener::bind(&at)?;
  listening.set_nonblocking(true)?;
  let (say, heard) = sync_channel(1);

  let _host_reports_the_exchange = std::thread::spawn(move || {
    let served = answer_one(listening, answer);
    say.send(served)
  });

  Ok(Host { at, heard })
}

fn answer_one(mut listening: UnixListener, answer: &[u8]) -> io::Result<Request> {
  let Ok(mut clock) = LiveClock::started();
  let Ok(schedule) = Schedule::of(PATIENCE);
  let accepted = until_some_handed_on_result(&mut clock, schedule, &mut listening, |door| {
    match door.accept() {
      Ok((stream, _address)) => Ok(Some(stream)),
      Err(fault) => match bool::then_some(
        [io::ErrorKind::WouldBlock, io::ErrorKind::Interrupted].contains(&fault.kind()),
        (),
      ) {
        Some(()) => Ok(None),
        None => Err(fault),
      },
    }
  })?;
  let mut stream = accepted.ok_or(io::ErrorKind::TimedOut)?;
  stream.set_read_timeout(Some(PATIENCE))?;
  stream.set_write_timeout(Some(PATIENCE))?;
  let reader = stream.try_clone()?;
  let mut reading = BufReader::new(reader);
  let mut line = String::new();
  reading.read_line(&mut line)?;

  let Ok(asked) = handoff::read(line.trim_end());
  let request = asked.ok_or(io::ErrorKind::InvalidData)?;
  stream.write_all(answer)?;

  Ok(request)
}

#[test]
fn an_old_hosts_drawn_reply_cannot_claim_presentation_completion() -> Result<(), Box<dyn Error>> {
  let host = a_host("no-acceptance", b"drawn\ngone\n")?;
  let Ok(words) = arguments(&[]);

  assert_eq!(
    handoff::stood_in_at(&host.at, "launcher", &words),
    Ok(DrawnBy::Here)
  );

  let _request_was_answered = host.request()?;

  Ok(())
}

#[test]
fn cancellation_before_presentation_does_not_reopen_the_panel_locally()
-> Result<(), Box<dyn Error>> {
  let host = a_host("cancelled", b"accepted\ngone\n")?;
  let Ok(words) = arguments(&[]);

  assert_eq!(
    handoff::stood_in_at(&host.at, "launcher", &words),
    Ok(DrawnBy::ByTheHost)
  );

  let _request_was_answered = host.request()?;

  Ok(())
}

#[test]
fn acceptance_does_not_mean_the_host_finished_drawing() -> Result<(), Box<dyn Error>> {
  let host = a_host("accepted-only", b"accepted\n")?;
  let Ok(words) = arguments(&[]);

  assert_eq!(
    handoff::stood_in_at(&host.at, "launcher", &words),
    Ok(DrawnBy::Here)
  );

  let _request_was_answered = host.request()?;

  Ok(())
}

#[test]
fn a_failed_drawing_is_left_to_the_requester() -> Result<(), Box<dyn Error>> {
  let host = a_host("failed", b"accepted\nfailed\n")?;
  let Ok(words) = arguments(&[]);

  assert_eq!(
    handoff::stood_in_at(&host.at, "launcher", &words),
    Ok(DrawnBy::Here)
  );

  let _request_was_answered = host.request()?;

  Ok(())
}

#[test]
fn a_panel_with_no_host_draws_itself() -> Result<(), Box<dyn Error>> {
  let at = somewhere("no one-is-listening")?;
  let Ok(words) = arguments(&[]);

  assert_eq!(
    handoff::stood_in_at(&at, "launcher", &words),
    Ok(DrawnBy::Here),
    "a host that is down has to be a panel that is slower, not a button that does nothing"
  );

  Ok(())
}

#[test]
fn a_host_that_draws_it_is_a_panel_that_does_not() -> Result<(), Box<dyn Error>> {
  let host = a_host("draws", DRAWN_AND_GONE)?;
  let Ok(words) = arguments(&["--keep"]);

  assert_eq!(
    handoff::stood_in_at(&host.at, "launcher", &words),
    Ok(DrawnBy::ByTheHost),
    "drawn twice is two menus over each other"
  );

  let _request_was_answered = host.request()?;

  Ok(())
}

#[test]
fn a_requester_keeps_the_first_drawing_when_the_host_hangs_up() -> Result<(), Box<dyn Error>> {
  let host = a_host("gone-after-drawn", b"accepted\ndrawn\n")?;
  let Ok(words) = arguments(&[]);

  assert_eq!(
    handoff::stood_in_at(&host.at, "launcher", &words),
    Ok(DrawnBy::ByTheHost)
  );
  let _request_was_answered = host.request()?;

  Ok(())
}

#[test]
fn what_was_typed_is_what_the_host_is_asked_for() -> Result<(), Box<dyn Error>> {
  let host = a_host("arguments", DRAWN_AND_GONE)?;
  let Ok(words) = arguments(&["Game Mode"]);

  let ::core::result::Result::Ok(_) = handoff::stood_in_at(&host.at, "settings-panel", &words);
  let asked = host.request()?;

  assert_eq!(asked.who, "settings-panel");

  let Ok(words_2) = arguments(&["Game Mode"]);

  assert_eq!(
    asked.arguments, words_2,
    "the tab a bar icon asks for is one word"
  );

  Ok(())
}

#[test]
fn a_host_that_goes_away_before_it_draws_leaves_the_panel_to_draw() -> Result<(), Box<dyn Error>>
{
  let host = a_host("hangs-up", b"")?;
  let Ok(words) = arguments(&[]);

  assert_eq!(
    handoff::stood_in_at(&host.at, "launcher", &words),
    Ok(DrawnBy::Here),
    "a host that died between accepting and drawing is a press that answered nothing"
  );

  let _request_was_answered = host.request()?;

  Ok(())
}

//! A requester can observe acceptance while rendering is still pending.
//!
//! The host advances only when the test permits it. Acceptance, presentation
//! and closing are therefore separate observable states without a sleep
//! deciding when the next step is safe to run.

use std::error::Error;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use console_panel::completion;
use console_panel::handoff::{self, DrawnBy};
use console_waiting::clock::LiveClock;
use console_waiting::{Schedule, until_some_handed_on_result};

const PATIENCE: Duration = Duration::from_secs(5);

fn observed(at: &Path, wanted: &str) -> Result<(), Box<dyn Error>> {
  let Ok(mut clock) = LiveClock::started();
  let Ok(schedule) = Schedule::of(PATIENCE);
  let found = until_some_handed_on_result(
    &mut clock,
    schedule,
    &mut (at, wanted),
    |(at_2, wanted_2)| {
      let said = match std::fs::read_to_string(at_2) {
        Ok(said) => said,
        Err(fault) => {
          match ::core::primitive::bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
            ::core::option::Option::Some(()) => return Ok(None),
            ::core::option::Option::None => return Err(fault),
          }
        }
      };

      Ok(
        match ::core::primitive::bool::then_some(said.lines().any(|line| line == *wanted_2), ()) {
          ::core::option::Option::Some(()) => Some(()),
          ::core::option::Option::None => None,
        },
      )
    },
  )?;

  match found {
    Some(()) => Ok(()),
    None => Err(Box::new(io::Error::new(
      io::ErrorKind::TimedOut,
      "the requester did not report its milestone",
    ))),
  }
}

#[test]
fn acceptance_and_finished_drawing_are_separate_observable_milestones()
-> Result<(), Box<dyn Error>> {
  let here = console_core_temporary_directories::fresh("panel-completion")?;
  let at = here.join("panels.sock");
  let completion = here.join("request.progress");

  let listening = UnixListener::bind(&at)?;
  let (advance, permitted) = mpsc::sync_channel(1);
  let (host_done, host_result) = mpsc::sync_channel(1);
  let _host_reports_on_completion = std::thread::spawn(move || {
    let served = (|| -> Result<(), Box<dyn Error + Send + Sync>> {
      let (mut stream, _) = listening.accept()?;
      stream.set_read_timeout(Some(PATIENCE))?;
      let mut request = String::new();

      let copy = stream.try_clone()?;
      let mut reading = BufReader::new(copy);

      reading.read_line(&mut request)?;
      stream.write_all(b"accepted\n")?;
      permitted.recv_timeout(PATIENCE)?;
      stream.write_all(b"drawn\n")?;
      permitted.recv_timeout(PATIENCE)?;
      stream.write_all(b"gone\n")?;

      Ok(())
    })();
    host_done.send(served)
  });
  let observed_at = completion.clone();
  let (client_done, client_result) = mpsc::sync_channel(1);
  let _client_reports_on_completion = std::thread::spawn(move || {
    let held = handoff::request(&at, "launcher", &[], |progress| {
      let recorded = completion::record(Some(&observed_at), progress);

      match recorded {
        Ok(()) => {}
        Err(fault) => eprintln!("the completion could not be recorded: {fault}"),
      }

      Ok(())
    });
    client_done.send(held)
  });

  observed(&completion, "accepted")?;

  let accepted = std::fs::read_to_string(&completion)?;

  assert_eq!(accepted, "accepted\n");

  advance.send(())?;
  observed(&completion, "drawn")?;

  let drawn = std::fs::read_to_string(&completion)?;

  assert_eq!(drawn, "accepted\ndrawn\n");

  advance.send(())?;

  let held = client_result.recv_timeout(PATIENCE)?;

  assert_eq!(held, Ok(DrawnBy::ByTheHost));
  let gone = std::fs::read_to_string(&completion)?;

  assert_eq!(gone, "accepted\ndrawn\ngone\n");

  let served = host_result.recv_timeout(PATIENCE)?;

  served.map_err(|fault| io::Error::other(fault.to_string()))?;

  Ok(())
}

//! One program, holding every panel.
//!
//! Three days of the device's own timing file said the same thing every day:
//! the two largest stretches of any opening are `exec` and the toolkit coming
//! up. Neither is work about the panel. A card is built and filled and drawn in
//! a fraction of either, so no amount of work on how a card is built moves the
//! number a thumb waits -- and the thing that does is not starting a process at
//! all.
//!
//! So this one stays up. It opens the display once, parses the stylesheet once,
//! reads the icon theme once, and draws whichever panel it is asked for. What
//! reaches it is a request on a socket from the panel's own program, which is
//! still exec'd and still holds the screen -- see `console_panel::handoff` for
//! why that half did not go away, and why it was not taken over here.
//!
//! **A closed panel costs nothing.** This is the whole of what a resident
//! process owes a handheld. With nothing on the screen this process is asleep
//! in one `poll` over the door, the panel it is holding and the signal that
//! asks it to stop, so the cost of being warm is the memory and not the
//! battery -- but only for as long as nothing is left running underneath. What a panel holds while it is up is a `pactl subscribe` or a
//! `busctl monitor` per watch, a thread per actor, and a window; today those
//! cost nothing because the process dies. Here they are released by name, in
//! `shut`, and `console-events`' own head is the argument for why that matters:
//! twenty-five orphaned subscriptions were once found alive on this device, the
//! oldest four hours old.
//!
//! **Nothing is kept between openings.** The window is destroyed and built
//! again rather than hidden and shown, and the panel's state is built by the
//! same call its `main` used to make. That is deliberate and it costs a few
//! milliseconds of the cheapest stretch there is: a surface that survived would
//! be a surface holding a reading no one refreshed, which is the one hazard
//! the `programs.md` note in Orbit names as the real one in a program that
//! holds state. An opening here runs the same code an exec ran, less the
//! toolkit coming up.
//!
//! **One at a time, and never none between two.** A request that arrives while
//! something is up is drawn first and closes what was up after its first
//! frame, so the screen goes from one panel to the next without an empty
//! moment in between. It used to close first, and every listener on the
//! compositor's layers -- the bar lighting its icons most of all -- read that
//! moment as nothing being up, so a tap on the clock with settings open lit
//! the calendar, put it out and lit it again. The two are up together for one
//! frame, which is shorter than anything the picker's lock is there to stop:
//! it is about a hand driving a picker it cannot see, and nothing can be
//! pressed into the one going in the frame it takes to go.
//!
//! Closing normally waits for drawing to stop before running the panel's
//! finalizer. If the surface outlives its close deadline, its finalizer stays
//! uncalled. A requester that already saw Drawn then reaches the end of the
//! socket with its drawing still owned by the host.

use std::io::{self, BufRead, BufReader, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::ExitCode;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use rustix::event::{PollFd, PollFlags, poll};

use console_core_arguments::{Command, Operands};
use console_core_internal_programs::InternalProgram;
use console_core_iteration::Step;
use console_core_never::Never;
use console_panel::card::{Card, Finalizer, refusal};
use console_panel::completion::Progress;
use console_panel::handoff::{self, CLOSE, Request};
use console_panel::left_open::LeftOpen;
use console_panel::picker;
use console_panel::surface;
use console_panel::tasks::{self, InProgress};
use console_waiting::{Outcome, Ready, Schedule, until};

const ACCEPTS_PER_TURN: u8 = 16;

const CLOSE_PATIENCE: Duration = Duration::from_secs(2);

const CLOSE_CHECK: Duration = Duration::from_millis(1);

pub struct Panel {
  pub who: &'static str,
  pub command: Command,
  pub card: fn(&[String]) -> Result<Card, Never>,
  pub program: InternalProgram,
}

pub const COMMAND: Command = Command {
  name: "console-panels",
  about: "the program that holds every panel; with nothing after it, the host, and with a panel named, that panel drawn here and then gone",
  flags: &[],
  operands: Operands::Verbatim("PANEL"),
};

pub const PANELS: &[Panel] = &[
  Panel {
    who: console_launcher::WHO,
    command: console_launcher::COMMAND,
    card: console_launcher::card,
    program: InternalProgram::Launcher,
  },
  Panel {
    who: console_settings::WHO,
    command: console_settings::COMMAND,
    card: console_settings::card,
    program: InternalProgram::SettingsPanel,
  },
  Panel {
    who: console_application_music::WHO,
    command: console_application_music::COMMAND,
    card: console_application_music::card,
    program: InternalProgram::MusicPanel,
  },
  Panel {
    who: console_notifications::WHO,
    command: console_notifications::COMMAND,
    card: console_notifications::card,
    program: InternalProgram::NotificationsPanel,
  },
  Panel {
    who: console_application_clipboard::WHO,
    command: console_application_clipboard::COMMAND,
    card: console_application_clipboard::card,
    program: InternalProgram::Clipboard,
  },
  Panel {
    who: console_application_task_manager::WHO,
    command: console_application_task_manager::COMMAND,
    card: console_application_task_manager::card,
    program: InternalProgram::TaskManager,
  },
  Panel {
    who: console_input_mapping::WHO,
    command: console_input_mapping::COMMAND,
    card: console_input_mapping::card,
    program: InternalProgram::MappingPanel,
  },
  Panel {
    who: console_application_calculator::WHO,
    command: console_application_calculator::COMMAND,
    card: console_application_calculator::card,
    program: InternalProgram::Calculator,
  },
  Panel {
    who: console_calendar::WHO,
    command: console_calendar::COMMAND,
    card: console_calendar::card,
    program: InternalProgram::CalendarPanel,
  },
  Panel {
    who: console_forecast::WHO,
    command: console_forecast::COMMAND,
    card: console_forecast::card,
    program: InternalProgram::ForecastPanel,
  },
];

pub fn one(who: &str) -> Result<Option<&'static Panel>, Never> {
  Ok(PANELS.iter().find(|panel| panel.who == who))
}

struct Up {
  who: String,
  shut: surface::Close,
  thread: thread::JoinHandle<Result<(), Never>>,
  writer: UnixStream,
  reader: BufReader<UnixStream>,
  while_it_is_up: OwnedFd,
  done: Option<Finalizer>,
}

struct PanelResources {
  tasks: InProgress,
  fonts: surface::FontCatalog,
}

pub fn serve() -> Result<(), Never> {
  let Ok(where_) = handoff::where_();

  let at = match where_ {
    Some(at) => at,
    None => {
      console_core_logging::warn!("XDG_RUNTIME_DIR names nothing, so there is nowhere to listen");

      return Ok(());
    }
  };

  match no_one_is_there(&at) {
    Ok(Free::Occupied) => {
      console_core_logging::info!("{} is answering already", at.display());

      return Ok(());
    }
    Ok(Free::Yes) => {}
    Err(fault) => {
      console_core_logging::warn!("{}: {fault}", at.display());

      return Ok(());
    }
  }

  let listening = match UnixListener::bind(&at) {
    Ok(listening) => listening,
    Err(fault) => {
      console_core_logging::warn!("{}: {fault}", at.display());

      return Ok(());
    }
  };

  match listening.set_nonblocking(true) {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::warn!("the panel listener could not be made nonblocking: {fault}");

      return clean_socket(&at);
    }
  }

  let Ok(stopping) = console_panel::asked::signal_pipe();

  let kept = match tasks::kept() {
    Ok(kept) => kept,
    Err(fault) => {
      console_core_logging::warn!("nothing to keep what a press starts in: {fault}");

      return clean_socket(&at);
    }
  };
  let Ok(fonts) = surface::prepare_fonts();
  let resources = PanelResources {
    tasks: kept.actor.clone(),
    fonts,
  };

  console_core_logging::info!("holding the panels, listening at {}", at.display());

  let Ok(()) = put_back();

  let held_until_stopped = console_core_iteration::iterate(None, |mut up: Option<Up>| {
    let held = up.as_ref().map(|up_2| Descriptors {
      panel: up_2.reader.get_ref().as_fd(),
      gone: up_2.while_it_is_up.as_fd(),
      pending: up_2.reader.buffer(),
    });
    let Ok(gone) = wait_for_request(&listening, stopping.as_ref(), held);

    match gone {
      PanelRequest::ToStop => {
        let Ok(()) = nothing_is_up(&mut up);

        return Ok(Step::Halt(()));
      }
      PanelRequest::ForAPanel => {
        let Ok(()) = accept_pending(&mut up, &listening, &resources);
      }
      PanelRequest::ByThePanel => {
        let Ok(said) = a_word(&mut up);

        match said {
          Word::Close | Word::Closed => {
            let Ok(()) = nothing_is_up(&mut up);
          }
          Word::None => {}
        }
      }
      PanelRequest::PanelGone => {
        let Ok(()) = nothing_is_up(&mut up);
        let Ok(()) = console_panel::left_open::put_away();
      }
      PanelRequest::None => {}
    }

    Ok(Step::Again(up))
  });

  match held_until_stopped {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("the panel listener ended unexpectedly: {fault}"),
  }

  let Ok(()) = clean_socket(&at);

  kept.scope.close()
}

fn put_back() -> Result<(), Never> {
  let Ok(left) = console_panel::left_open::load();

  let left_3 = match left {
    Some(left_2) => left_2,
    None => return Ok(()),
  };

  let Ok(known) = one(&left_3.who);

  match known {
    Some(known_2) => console_panel::left_open::started(known_2.program, left_3.arguments),
    None => {
      console_core_logging::warn!("{:?} was left open and nothing here draws it", left_3.who);

      Ok(())
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelRequest {
  ForAPanel,
  ByThePanel,
  PanelGone,
  ToStop,
  None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Watching {
  Door,
  Stopping,
  Panel,
  Gone,
}

struct Descriptors<'a> {
  panel: BorrowedFd<'a>,
  gone: BorrowedFd<'a>,
  pending: &'a [u8],
}

fn wait_for_request(
  listening: &UnixListener,
  stopping: Option<&OwnedFd>,
  up: Option<Descriptors<'_>>,
) -> Result<PanelRequest, Never> {
  match ::core::primitive::bool::then_some(
    up.as_ref()
      .is_some_and(|up_2| up_2.pending.contains(&b'\n')),
    (),
  ) {
    ::core::option::Option::Some(()) => return Ok(PanelRequest::ByThePanel),
    ::core::option::Option::None => {}
  }

  let mut which: Vec<Watching> = vec![Watching::Door];
  let mut watch: Vec<PollFd<'_>> =
    vec![PollFd::from_borrowed_fd(listening.as_fd(), PollFlags::IN)];

  match stopping {
    Some(stopping_2) => {
      which.push(Watching::Stopping);
      watch.push(PollFd::from_borrowed_fd(stopping_2.as_fd(), PollFlags::IN));
    }
    None => {}
  }

  match up {
    Some(up_3) => {
      which.push(Watching::Panel);
      watch.push(PollFd::from_borrowed_fd(up_3.panel, PollFlags::IN));
      which.push(Watching::Gone);
      watch.push(PollFd::from_borrowed_fd(up_3.gone, PollFlags::IN));
    }
    None => {}
  }

  match poll(&mut watch, None) {
    Ok(_) => {}
    Err(rustix::io::Errno::INTR) => return Ok(PanelRequest::None),
    Err(fault) => {
      console_core_logging::warn!("waiting: {fault}");

      return Ok(PanelRequest::ToStop);
    }
  }

  let said = watch
    .iter()
    .zip(which)
    .find(|(fd, _)| !fd.revents().is_empty())
    .map(|(_, which_2)| which_2);

  Ok(match said {
    Some(Watching::Door) => PanelRequest::ForAPanel,
    Some(Watching::Stopping) => PanelRequest::ToStop,
    Some(Watching::Panel) => PanelRequest::ByThePanel,
    Some(Watching::Gone) => PanelRequest::PanelGone,
    None => PanelRequest::None,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Free {
  Yes,
  Occupied,
}

fn no_one_is_there(at: &Path) -> io::Result<Free> {
  match UnixStream::connect(at) {
    Ok(_answered) => Ok(Free::Occupied),
    Err(fault) => match (
      fault.kind() == io::ErrorKind::NotFound,
      fault.kind() == io::ErrorKind::ConnectionRefused,
    ) {
      (true, _) => Ok(Free::Yes),
      (false, true) => {
        remove_socket(at)?;

        Ok(Free::Yes)
      }
      (false, false) => Err(fault),
    },
  }
}

fn remove_socket(at: &Path) -> io::Result<()> {
  let about = match std::fs::symlink_metadata(at) {
    Ok(about) => about,
    Err(fault) => return missing_socket_is_gone(fault),
  };

  match bool::then_some(about.file_type().is_socket(), ()) {
    Some(()) => {}
    None => {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "expected a panel socket",
      ));
    }
  }

  std::fs::remove_file(at).or_else(missing_socket_is_gone)
}

fn missing_socket_is_gone(fault: io::Error) -> io::Result<()> {
  match bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
    Some(()) => Ok(()),
    None => Err(fault),
  }
}

fn clean_socket(at: &Path) -> Result<(), Never> {
  match remove_socket(at) {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("removing {}: {fault}", at.display()),
  }

  Ok(())
}

fn accept_pending(
  up: &mut Option<Up>,
  listening: &UnixListener,
  resources: &PanelResources,
) -> Result<(), Never> {
  for _ in 0..ACCEPTS_PER_TURN {
    let asking = match listening.accept() {
      Ok((asking, _address)) => asking,
      Err(fault) => match (
        fault.kind() == io::ErrorKind::WouldBlock,
        fault.kind() == io::ErrorKind::Interrupted,
      ) {
        (true, _) => break,
        (false, true) => continue,
        (false, false) => {
          console_core_logging::warn!("accepting a panel request: {fault}");

          break;
        }
      },
    };
    let Ok(()) = asked_of(up, asking, resources);
  }

  Ok(())
}

fn asked_of(
  up: &mut Option<Up>,
  asking: UnixStream,
  resources: &PanelResources,
) -> Result<(), Never> {
  let heard = match asking.try_clone() {
    Ok(heard) => heard,
    Err(fault) => {
      console_core_logging::warn!("cloning a panel request socket: {fault}");

      return Ok(());
    }
  };

  let mut reader = BufReader::new(heard);
  let mut line = String::new();

  match reader.read_line(&mut line) {
    Ok(0) => return Ok(()),
    Err(_the_read_failed) => return Ok(()),
    Ok(_) => {}
  }

  let Ok(asked) = handoff::read(line.trim_end());

  let asked_3 = match asked {
    Some(asked_2) => asked_2,
    None => {
      console_core_logging::warn!("{:?} is not a request", line.trim_end());

      return Ok(());
    }
  };

  put_up(up, asked_3, asking, reader, resources)
}

fn put_up(
  up: &mut Option<Up>,
  asked: Request,
  writer: UnixStream,
  reader: BufReader<UnixStream>,
  resources: &PanelResources,
) -> Result<(), Never> {
  let Ok(known) = one(&asked.who);

  let known_3 = match known {
    Some(known_2) => known_2,
    None => {
      console_core_logging::warn!("nothing here draws {:?}", asked.who);

      let Ok(()) = say(&writer, Progress::Failed);

      return Ok(());
    }
  };

  let Ok(()) = say(&writer, Progress::Accepted);

  let Ok(()) = console_panel::opening::start(
    &asked.who,
    asked.pressed.as_deref(),
    console_panel::opening::Came(&asked.from),
    asked.exec,
  );

  let Ok(()) = console_panel::left_open::opened(&LeftOpen {
    who: asked.who.clone(),
    arguments: asked.arguments.clone(),
  });
  let Ok(card) = (known_3.card)(&asked.arguments);
  let Card {
    build,
    column,
    start,
    done,
  } = card;

  let (while_it_is_up, told_when_it_is_not) = match up_and_gone() {
    Ok(ends) => ends,
    Err(fault) => {
      console_core_logging::warn!("nothing to hear a panel close on: {fault}");

      let Ok(()) = say(&writer, Progress::Failed);

      return Ok(());
    }
  };

  let Ok((first_frame, drawn)) = surface::drawing_notifications();
  let Ok(fonts) = resources.fonts.for_draw_thread();

  let Ok((shut, surface_thread)) = surface::run(
    &asked.who,
    (build, resources.tasks.clone()),
    column,
    start.as_deref(),
    surface::HostedDrawing {
      fonts,
      while_it_is_up: told_when_it_is_not,
      first_frame,
    },
    asked.tells.clone(),
  );

  let Ok(clock) = console_waiting::clock::LiveClock::started();
  let presented = wait_for_drawing(&drawn, &clock, picker::COMING);
  let incoming = Up {
    who: asked.who.clone(),
    shut,
    thread: surface_thread,
    writer,
    reader,
    while_it_is_up,
    done: Some(done),
  };

  match presented {
    Ok(()) => {
      let Ok(()) = nothing_is_up(up);

      *up = Some(incoming);

      match up.as_ref() {
        Some(up_2) => {
          let Ok(()) = say(&up_2.writer, Progress::Visible);
        }
        None => {}
      }
    }
    Err(fault) => {
      console_core_logging::warn!("{} did not present its first frame: {fault:?}", asked.who);
      let Ok(()) = finish(incoming, Finish::Unpresented(fault));
    }
  }

  Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrawingError {
  Discarded,
  Unavailable,
  Cancelled,
  Timeout,
  Disconnected,
}

fn drawing_ended(fault: DrawingError) -> Result<Progress, Never> {
  Ok(match fault {
    DrawingError::Cancelled => Progress::Gone,
    DrawingError::Discarded
    | DrawingError::Unavailable
    | DrawingError::Timeout
    | DrawingError::Disconnected => Progress::Failed,
  })
}

fn first_drawing(
  received: Result<surface::Drawing, RecvTimeoutError>,
) -> Result<(), DrawingError> {
  match received {
    Ok(surface::Drawing::Presented) => Ok(()),
    Ok(surface::Drawing::Discarded) => Err(DrawingError::Discarded),
    Ok(surface::Drawing::Unavailable) => Err(DrawingError::Unavailable),
    Ok(surface::Drawing::Cancelled) => Err(DrawingError::Cancelled),
    Err(RecvTimeoutError::Timeout) => Err(DrawingError::Timeout),
    Err(RecvTimeoutError::Disconnected) => Err(DrawingError::Disconnected),
  }
}

fn wait_for_drawing(
  drawn: &mpsc::Receiver<surface::Drawing>,
  clock: &impl console_waiting::clock::Clock,
  patience: std::time::Duration,
) -> Result<(), DrawingError> {
  let Ok(began) = clock.elapsed();
  let ended = console_core_iteration::iterate((), |()| {
    let Ok(now) = clock.elapsed();
    let remaining = patience.saturating_sub(now.saturating_sub(began));

    match ::core::primitive::bool::then_some(remaining.is_zero(), ()) {
      ::core::option::Option::Some(()) => return Ok(Step::Halt(Err(DrawingError::Timeout))),
      ::core::option::Option::None => {}
    }

    Ok(match first_drawing(drawn.recv_timeout(remaining)) {
      Err(DrawingError::Discarded) => Step::Again(()),
      Ok(()) => Step::Halt(Ok(())),
      Err(fault) => Step::Halt(Err(fault)),
    })
  });

  match ended {
    Ok(finished) => finished,
    Err(_endless) => Err(DrawingError::Timeout),
  }
}

fn up_and_gone() -> rustix::io::Result<(OwnedFd, OwnedFd)> {
  let (told_when_it_is_not, while_it_is_up) =
    rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;

  Ok((while_it_is_up, told_when_it_is_not))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Word {
  Close,
  Closed,
  None,
}

fn a_word(up: &mut Option<Up>) -> Result<Word, Never> {
  let mut line = String::new();

  let up_3 = match up {
    Some(up_2) => up_2,
    None => return Ok(Word::None),
  };

  Ok(match up_3.reader.read_line(&mut line) {
    Ok(0) => Word::Closed,
    Err(_the_read_failed) => Word::Closed,
    Ok(_) => match ::core::primitive::bool::then_some(line.trim() == CLOSE, ()) {
      ::core::option::Option::Some(()) => Word::Close,
      ::core::option::Option::None => Word::None,
    },
  })
}

fn nothing_is_up(up: &mut Option<Up>) -> Result<(), Never> {
  let up_3 = match up.take() {
    Some(up_2) => up_2,
    None => return Ok(()),
  };

  finish(up_3, Finish::Presented)
}

enum Finish {
  Presented,
  Unpresented(DrawingError),
}

fn finish(up: Up, finished: Finish) -> Result<(), Never> {
  let Ok(mut closing) = console_response_times::Waiting::here(console_response_times::Wait {
    who: &up.who,
    what: "closing",
  });
  let Ok(()) = up.shut.shut();
  let Up {
    who,
    shut: _,
    thread,
    writer,
    reader: _,
    while_it_is_up: _,
    done,
  } = up;
  let Ok(patience) = Schedule::asking_every(CLOSE_PATIENCE, CLOSE_CHECK);
  let Ok(stopped) = until(patience, || {
    Ok(match bool::then_some(thread.is_finished(), ()) {
      Some(()) => Ready::Yes,
      None => Ready::NotYet,
    })
  });

  match stopped {
    Outcome::Happened => {}
    Outcome::RanOut => {
      console_core_logging::warn!("the {who} surface did not close before its deadline");

      return match finished {
        Finish::Presented => Ok(()),
        Finish::Unpresented(_) => say(&writer, Progress::Failed),
      };
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the bounded wait immediately above verified that the surface worker has finished"
    )
  )]
  let joined = thread.join();

  match joined {
    Ok(Ok(())) => {}
    Err(_worker_panicked) => console_core_logging::warn!("the {who} surface worker panicked"),
  }

  let Ok(()) = closing.mark("drawing");

  match done {
    Some(done_2) => {
      let Ok(()) = done_2();
    }
    None => {}
  }

  let Ok(()) = closing.mark("finishing");

  let Ok(()) = closing.finish();

  match finished {
    Finish::Presented => say(&writer, Progress::Gone),
    Finish::Unpresented(fault) => {
      let Ok(progress) = drawing_ended(fault);
      say(&writer, progress)
    }
  }
}

fn say(writer: &UnixStream, progress: Progress) -> Result<(), Never> {
  let mut writing = writer;
  let Ok(word) = progress.word();

  match writeln!(writing, "{word}") {
    Ok(()) => {}
    Err(_the_panel_that_asked_has_gone) => return Ok(()),
  }

  Ok(())
}

fn drawn_by_hand(who: &str, arguments: &[String]) -> Result<ExitCode, Never> {
  let Ok(known) = one(who);

  let known_3 = match known {
    Some(known_2) => known_2,
    None => {
      console_core_logging::warn!("nothing here draws {who:?}");

      return Ok(ExitCode::FAILURE);
    }
  };

  let Ok(refusing) = refusal(&known_3.command, arguments);

  match refusing {
    Some(code) => return Ok(code),
    None => {}
  }

  let Ok(card) = (known_3.card)(arguments);
  let Ok(()) = surface::drawn_here(who, card);

  Ok(ExitCode::SUCCESS)
}

pub fn asked_for(arguments: &[String]) -> Result<ExitCode, Never> {
  let Ok(refusing) = refusal(&COMMAND, arguments);

  match refusing {
    Some(code) => return Ok(code),
    None => {}
  }

  match arguments.split_first() {
    None => {
      let Ok(()) = serve();

      Ok(ExitCode::SUCCESS)
    }
    Some((who, rest)) => drawn_by_hand(who, rest),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;

  #[test]
  fn startup_retires_a_stale_socket_and_preserves_other_paths() -> Result<(), Box<dyn Error>> {
    let fresh = console_core_temporary_directories::fresh("panel-socket-startup")?;
    let socket = fresh.join("panels.sock");
    let stale = UnixListener::bind(&socket)?;
    drop(stale);

    let recovered = no_one_is_there(&socket)?;
    assert_eq!(recovered, Free::Yes);

    let listening = UnixListener::bind(&socket)?;
    let occupied = no_one_is_there(&socket)?;
    assert_eq!(occupied, Free::Occupied);
    drop(listening);
    remove_socket(&socket)?;

    use rustix::fs::{Mode, OFlags, open};
    let _other = open(
      &socket,
      OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC,
      Mode::RUSR | Mode::WUSR,
    )?;
    let refused = no_one_is_there(&socket);
    let metadata = socket.symlink_metadata()?;

    assert!(matches!(refused, Err(_not_a_socket)));
    assert!(metadata.is_file());

    Ok(())
  }

  #[test]
  fn only_a_presented_frame_completes_drawing() {
    assert_eq!(first_drawing(Ok(surface::Drawing::Presented)), Ok(()));
    assert_eq!(
      first_drawing(Ok(surface::Drawing::Discarded)),
      Err(DrawingError::Discarded)
    );
    assert_eq!(
      first_drawing(Ok(surface::Drawing::Unavailable)),
      Err(DrawingError::Unavailable)
    );
    assert_eq!(
      first_drawing(Ok(surface::Drawing::Cancelled)),
      Err(DrawingError::Cancelled)
    );
    assert_eq!(
      first_drawing(Err(RecvTimeoutError::Timeout)),
      Err(DrawingError::Timeout)
    );
    assert_eq!(
      first_drawing(Err(RecvTimeoutError::Disconnected)),
      Err(DrawingError::Disconnected)
    );
    assert_eq!(drawing_ended(DrawingError::Cancelled), Ok(Progress::Gone));
    assert_eq!(
      drawing_ended(DrawingError::Unavailable),
      Ok(Progress::Failed)
    );
    assert_eq!(drawing_ended(DrawingError::Timeout), Ok(Progress::Failed));
  }

  #[test]
  fn a_superseded_frame_waits_for_the_replacement_to_be_presented() -> Result<(), Box<dyn Error>>
  {
    let (report, drawing) = mpsc::sync_channel(2);

    report.send(surface::Drawing::Discarded)?;
    report.send(surface::Drawing::Presented)?;

    let clock = console_waiting::clock::TestClock::default();

    assert_eq!(wait_for_drawing(&drawing, &clock, picker::COMING), Ok(()));

    Ok(())
  }

  #[test]
  fn an_unpresented_frame_cannot_finish_after_its_deadline() {
    let (_report, drawing) = mpsc::sync_channel(1);
    let clock = console_waiting::clock::TestClock::default();

    assert_eq!(
      wait_for_drawing(&drawing, &clock, std::time::Duration::ZERO),
      Err(DrawingError::Timeout)
    );
  }

  #[test]
  fn a_close_read_ahead_of_its_turn_is_handled_before_another_open() -> Result<(), Box<dyn Error>>
  {
    let fresh = console_core_temporary_directories::fresh("panels-buffered-close")?;
    let at = fresh.join("panels.sock");
    let listening = UnixListener::bind(&at)?;
    let _another_open = UnixStream::connect(&at)?;
    let (asked_by, mut asker) = UnixStream::pair()?;

    asker.write_all(b"open launcher\nclose\n")?;

    let mut reader = BufReader::new(asked_by);
    let mut opening = String::new();

    reader.read_line(&mut opening)?;

    let (while_it_is_up, _still_up) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;
    let heard = wait_for_request(
      &listening,
      None,
      Some(Descriptors {
        panel: reader.get_ref().as_fd(),
        gone: while_it_is_up.as_fd(),
        pending: reader.buffer(),
      }),
    );

    assert_eq!(heard, Ok(PanelRequest::ByThePanel));

    Ok(())
  }

  #[test]
  fn a_panel_that_put_itself_away_is_gone_without_a_word_from_whoever_asked_for_it()
  -> Result<(), Box<dyn Error>> {
    let fresh = console_core_temporary_directories::fresh("panels-gone")?;
    let at = fresh.join("panels.sock");
    let listening = UnixListener::bind(&at)?;
    let (asked_by, _still_waiting) = UnixStream::pair()?;
    let (while_it_is_up, told_when_it_is_not) =
      rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;

    drop(told_when_it_is_not);

    let heard = wait_for_request(
      &listening,
      None,
      Some(Descriptors {
        panel: asked_by.as_fd(),
        gone: while_it_is_up.as_fd(),
        pending: &[],
      }),
    );
    remove_socket(&at)?;

    assert_eq!(
      heard,
      Ok(PanelRequest::PanelGone),
      "B put the panel away and the program that asked for it is still waiting to hear so; \
             reading a word from it instead is a host stuck until the next press, which then \
             closes a menu that is not there rather than opening one"
    );

    Ok(())
  }

  #[test]
  fn every_panel_is_named_once() {
    let mut named: Vec<&str> = PANELS.iter().map(|panel| panel.who).collect();

    named.sort_unstable();
    let mut once = named.clone();
    once.dedup();

    assert_eq!(once, named, "two entries answering to one name: {named:?}");
  }

  #[test]
  fn a_panel_is_found_by_the_name_someone_types() {
    let Ok(found) = one("launcher");

    assert!(
      found.is_some(),
      "the menu is on a button, a paddle, a key and the bar"
    );

    let Ok(nothing) = one("console-panels");

    assert!(
      nothing.is_none(),
      "the host is not a panel it can be asked to draw"
    );
  }

  #[test]
  fn a_panel_left_open_is_put_back_by_starting_the_program_that_asks_for_it() {
    for panel in PANELS {
      assert_eq!(
        panel.program.name(),
        Ok(panel.who),
        "a restart would start the wrong program to put {} back",
        panel.who
      );
    }
  }

  #[test]
  fn every_name_here_is_the_program_a_road_in_already_names() {
    for panel in PANELS {
      assert!(!panel.who.is_empty());
      assert!(
        !panel.who.contains('/'),
        "{}: a namespace is a name, not a path",
        panel.who
      );
    }
  }

  #[test]
  fn a_panel_hears_it_is_put_away_while_a_program_it_started_is_still_running()
  -> Result<(), Box<dyn Error>> {
    let (while_it_is_up, told_when_it_is_not) = up_and_gone()?;
    let Ok(mut staying) = console_core_external_programs::Program::Sh.command();
    let mut started = staying.stdin(std::process::Stdio::piped()).spawn()?;

    drop(while_it_is_up);
    rustix::io::ioctl_fionbio(&told_when_it_is_not, true)?;

    let mut said = [0_u8; 1];
    let heard = match rustix::io::read(&told_when_it_is_not, &mut said) {
      Ok(0) => "put away",
      Ok(_a_byte) => "told something",
      Err(_still_open) => "nothing yet",
    };

    drop(started.stdin.take());
    let status = started.wait()?;

    assert!(
      status.success(),
      "the shell did not finish on EOF: {status}"
    );

    assert_eq!(
      heard, "put away",
      "an app the panel started kept the pipe open, so the panel never hears it was put away"
    );

    Ok(())
  }
}

//! The resident host for Console panels.
//!
//! A controller press can send the host a request directly. The host holds
//! the picker lease for that opening. Stand-in launchers also use the host
//! and keep their existing close and completion protocol.
//!
//! Each opening rebuilds its live page and action handlers. The host retains
//! up to three first-frame buffers. An identical card at the same geometry
//! can attach its prepared buffer while the page continues with live inputs.
//! A closed panel has no surface, active subscriptions, or render loop.
//!
//! When switching panels, the incoming panel draws before the outgoing panel
//! closes. Closing waits for the drawing thread before calling its finalizer.
//! A surface that outlives the close deadline keeps its finalizer uncalled.

use std::collections::VecDeque;
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
use console_panel::card::{Card, Door, Finalizer, refusal};
use console_panel::completion::Progress;
use console_panel::handoff::{self, CLOSE, HostRequest, Request};
use console_panel::left_open::{self, Journal, LeftOpen};
use console_panel::picker::{self, Again, ResidentLease};
use console_panel::surface;
use console_panel::tasks::{self, InProgress};
use console_waiting::{Outcome, Ready, Schedule, until};

mod connection;

const ACCEPTS_PER_TURN: u8 = 16;

const PREPARED_PANELS: u32 = 3;

const CLOSE_PATIENCE: Duration = Duration::from_secs(2);

const CLOSE_CHECK: Duration = Duration::from_millis(1);

pub struct Panel {
  pub who: &'static str,
  pub command: Command,
  pub card: fn(&[String]) -> Result<Card, Never>,
  pub door: fn(&[String]) -> Result<Door, Never>,
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
    door: console_launcher::door,
    program: InternalProgram::Launcher,
  },
  Panel {
    who: console_settings::WHO,
    command: console_settings::COMMAND,
    card: console_settings::card,
    door: console_settings::door,
    program: InternalProgram::SettingsPanel,
  },
  Panel {
    who: console_application_music::WHO,
    command: console_application_music::COMMAND,
    card: console_application_music::card,
    door: console_application_music::door,
    program: InternalProgram::MusicPanel,
  },
  Panel {
    who: console_notifications::WHO,
    command: console_notifications::COMMAND,
    card: console_notifications::card,
    door: console_notifications::door,
    program: InternalProgram::NotificationsPanel,
  },
  Panel {
    who: console_application_clipboard::WHO,
    command: console_application_clipboard::COMMAND,
    card: console_application_clipboard::card,
    door: console_application_clipboard::door,
    program: InternalProgram::Clipboard,
  },
  Panel {
    who: console_application_task_manager::WHO,
    command: console_application_task_manager::COMMAND,
    card: console_application_task_manager::card,
    door: console_application_task_manager::door,
    program: InternalProgram::TaskManager,
  },
  Panel {
    who: console_input_mapping::WHO,
    command: console_input_mapping::COMMAND,
    card: console_input_mapping::card,
    door: console_input_mapping::door,
    program: InternalProgram::MappingPanel,
  },
  Panel {
    who: console_application_calculator::WHO,
    command: console_application_calculator::COMMAND,
    card: console_application_calculator::card,
    door: console_application_calculator::door,
    program: InternalProgram::Calculator,
  },
  Panel {
    who: console_calendar::WHO,
    command: console_calendar::COMMAND,
    card: console_calendar::card,
    door: console_calendar::door,
    program: InternalProgram::CalendarPanel,
  },
  Panel {
    who: console_forecast::WHO,
    command: console_forecast::COMMAND,
    card: console_forecast::card,
    door: console_forecast::door,
    program: InternalProgram::ForecastPanel,
  },
  Panel {
    who: console_device::update_menu::WHO,
    command: console_device::update_menu::COMMAND,
    card: console_device::update_menu::card,
    door: console_device::update_menu::door,
    program: InternalProgram::Update,
  },
];

pub fn one(who: &str) -> Result<Option<&'static Panel>, Never> {
  Ok(PANELS.iter().find(|panel| panel.who == who))
}

struct Up {
  who: String,
  shut: surface::Close,
  thread: thread::JoinHandle<Result<(), Never>>,
  owner: Owner,
  while_it_is_up: OwnedFd,
  done: Option<Finalizer>,
}

enum Owner {
  StandIn {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
  },
  Resident {
    lease: ResidentLease,
    door: Door,
  },
}

impl Owner {
  fn reader(&self) -> Result<Option<&BufReader<UnixStream>>, Never> {
    Ok(match self {
      Self::StandIn { reader, .. } => Some(reader),
      Self::Resident { .. } => None,
    })
  }

  fn report(&mut self, progress: Progress) -> Result<(), Never> {
    match self {
      Self::StandIn { writer, .. } => say(writer, progress),
      Self::Resident { lease, door } => match progress {
        Progress::Visible => {
          match lease.visible(&door.name) {
            Ok(()) => {}
            Err(fault) => console_core_logging::warn!("recording the resident picker: {fault}"),
          }

          Ok(())
        }
        Progress::Accepted | Progress::Gone | Progress::Failed => Ok(()),
      },
    }
  }
}

struct PanelResources {
  tasks: InProgress,
  fonts: surface::FontCatalog,
  journal: Option<Journal>,
  prepared: VecDeque<(&'static str, surface::PreparedCard)>,
  connection: Option<connection::Spare>,
}

impl PanelResources {
  fn take_prepared(&mut self, who: &str) -> Result<Option<surface::PreparedCard>, Never> {
    match self.prepared.iter().position(|(known, _)| *known == who) {
      Some(at) => Ok(self.prepared.remove(at).map(|(_, frame)| frame)),
      None => Ok(None),
    }
  }

  fn keep_prepared(
    &mut self,
    who: &'static str,
    frame: surface::PreparedCard,
  ) -> Result<(), Never> {
    let occupied = match u32::try_from(self.prepared.len()) {
      Ok(count) => count,
      Err(_outside_the_cache_limit) => u32::MAX,
    };

    match (occupied >= PREPARED_PANELS).then_some(()) {
      Some(()) => {
        let _ = self.prepared.pop_front();
      }
      None => {}
    }

    self.prepared.push_back((who, frame));

    Ok(())
  }

  fn remember(&self, open: Option<&LeftOpen>) -> Result<(), Never> {
    match self.journal.as_ref() {
      Some(journal) => journal.remember(open),
      None => left_open::remember_directly(open, || {}),
    }
  }
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
  let record_worker = match Journal::start() {
    Ok(journal) => Some(journal),
    Err(fault) => {
      console_core_logging::warn!(
        "console-panels: could not start the panel record worker: {fault}"
      );
      None
    }
  };
  let mut resources = PanelResources {
    tasks: kept.actor.clone(),
    fonts,
    journal: record_worker,
    prepared: VecDeque::new(),
    connection: match connection::Spare::start() {
      Ok(spare) => Some(spare),
      Err(fault) => {
        console_core_logging::warn!("could not start the spare panel connection: {fault}");

        None
      }
    },
  };
  let Ok(resident) = listen_for_resident();

  console_core_logging::info!("holding the panels, listening at {}", at.display());

  let Ok(()) = put_back();

  let held_until_stopped =
    console_core_iteration::iterate((None::<Up>, &mut resources), |(mut up, held_resources)| {
      let held = up.as_ref().map(|up_2| {
        let Ok(reader) = up_2.owner.reader();

        Descriptors {
          panel: reader.map(|connection| connection.get_ref().as_fd()),
          gone: up_2.while_it_is_up.as_fd(),
          pending: reader.map_or(&[], BufReader::buffer),
        }
      });
      let Ok(gone) = wait_for_request(
        (&listening, resident.as_ref().map(|(_, socket)| socket)),
        stopping.as_ref(),
        held,
      );

      match gone {
        PanelRequest::ToStop => {
          let Ok(()) = nothing_is_up(&mut up);

          return Ok(Step::Halt(()));
        }
        PanelRequest::ForAPanel => {
          let Ok(()) = accept_pending(&mut up, &listening, held_resources);
        }
        PanelRequest::FromResident => match resident.as_ref() {
          Some((_, socket)) => {
            let Ok(()) = accept_pending(&mut up, socket, held_resources);
          }
          None => {}
        },
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
          let Ok(()) = held_resources.remember(None);
        }
        PanelRequest::None => {}
      }

      Ok(Step::Again((up, held_resources)))
    });

  match held_until_stopped {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("the panel listener ended unexpectedly: {fault}"),
  }

  let Ok(()) = clean_socket(&at);

  match resident {
    Some((path, _socket)) => {
      let Ok(()) = clean_socket(&path);
    }
    None => {}
  }

  match resources.journal {
    Some(journal) => {
      let Ok(()) = journal.close();
    }
    None => {}
  }

  kept.scope.close()
}

fn listen_for_resident() -> Result<Option<(std::path::PathBuf, UnixListener)>, Never> {
  let Ok(address) = handoff::resident_where();
  let at = match address {
    Some(at) => at,
    None => return Ok(None),
  };
  let listening = no_one_is_there(&at).and_then(|free| match free {
    Free::Yes => UnixListener::bind(&at),
    Free::Occupied => Err(io::Error::from(io::ErrorKind::AddrInUse)),
  });
  let listener = match listening {
    Ok(listener) => listener,
    Err(fault) => {
      console_core_logging::warn!("listening for resident panel requests: {fault}");

      return Ok(None);
    }
  };

  Ok(match listener.set_nonblocking(true) {
    Ok(()) => Some((at, listener)),
    Err(fault) => {
      console_core_logging::warn!("making the resident panel socket nonblocking: {fault}");
      let Ok(()) = clean_socket(&at);

      None
    }
  })
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
  FromResident,
  ByThePanel,
  PanelGone,
  ToStop,
  None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Watching {
  Door,
  Resident,
  Stopping,
  Panel,
  Gone,
}

struct Descriptors<'a> {
  panel: Option<BorrowedFd<'a>>,
  gone: BorrowedFd<'a>,
  pending: &'a [u8],
}

fn wait_for_request(
  (listening, resident): (&UnixListener, Option<&UnixListener>),
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

  match resident {
    Some(socket) => {
      which.push(Watching::Resident);
      watch.push(PollFd::from_borrowed_fd(socket.as_fd(), PollFlags::IN));
    }
    None => {}
  }

  match stopping {
    Some(stopping_2) => {
      which.push(Watching::Stopping);
      watch.push(PollFd::from_borrowed_fd(stopping_2.as_fd(), PollFlags::IN));
    }
    None => {}
  }

  match up {
    Some(up_3) => {
      match up_3.panel {
        Some(panel) => {
          which.push(Watching::Panel);
          watch.push(PollFd::from_borrowed_fd(panel, PollFlags::IN));
        }
        None => {}
      }

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
    Some(Watching::Resident) => PanelRequest::FromResident,
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
  resources: &mut PanelResources,
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
  resources: &mut PanelResources,
) -> Result<(), Never> {
  let mut reader = BufReader::new(asking);
  let mut line = String::new();

  match reader.read_line(&mut line) {
    Ok(0) => return Ok(()),
    Err(_the_read_failed) => return Ok(()),
    Ok(_) => {}
  }

  let request = match line.strip_suffix('\n') {
    Some(packet) => packet.trim_end(),
    None => return Ok(()),
  };
  let Ok(asked) = handoff::read_host(request);

  match asked {
    Some(HostRequest::StandIn(client)) => stand_in(up, client, reader, resources),
    Some(HostRequest::Resident(sent)) => resident(up, sent, resources),
    Some(HostRequest::Dismiss) => dismiss(up, resources),
    None => {
      console_core_logging::warn!("{:?} is not a request", line.trim_end());

      Ok(())
    }
  }
}

fn stand_in(
  up: &mut Option<Up>,
  asked: Request,
  reader: BufReader<UnixStream>,
  resources: &mut PanelResources,
) -> Result<(), Never> {
  let writer = match reader.get_ref().try_clone() {
    Ok(writer) => writer,
    Err(fault) => {
      console_core_logging::warn!("cloning a panel request socket: {fault}");

      return Ok(());
    }
  };
  let Ok(known) = one(&asked.who);

  match known {
    Some(panel) => put_up(
      up,
      asked,
      panel,
      Owner::StandIn { writer, reader },
      resources,
    ),
    None => {
      console_core_logging::warn!("nothing here draws {:?}", asked.who);

      say(&writer, Progress::Failed)
    }
  }
}

fn resident(
  up: &mut Option<Up>,
  asked: Request,
  resources: &mut PanelResources,
) -> Result<(), Never> {
  let Ok(known) = one(&asked.who);
  let panel = match known {
    Some(panel) => panel,
    None => return Ok(()),
  };
  let Ok(refused) = refusal(&panel.command, &asked.arguments);

  match refused {
    Some(_) => return Ok(()),
    None => {}
  }

  let Ok(door) = (panel.door)(&asked.arguments);
  let current = up.as_ref().map(|opened| &opened.owner);
  let offered = match current {
    Some(Owner::Resident { lease, door: open }) => match (open.name == door.name, door.again) {
      (true, Again::Keeps) => return Ok(()),
      (true, Again::Closes) => return dismiss(up, resources),
      (false, _) => lease.duplicate().map(Some),
    },
    Some(Owner::StandIn { .. }) => Ok(None),
    None => ResidentLease::acquire(),
  };
  let lease = match offered {
    Ok(Some(lease)) => lease,
    Ok(None) => return left_open::started(panel.program, asked.arguments),
    Err(fault) => {
      console_core_logging::warn!("the resident picker could not take the screen: {fault}");

      return left_open::started(panel.program, asked.arguments);
    }
  };

  put_up(up, asked, panel, Owner::Resident { lease, door }, resources)
}

fn dismiss(up: &mut Option<Up>, resources: &PanelResources) -> Result<(), Never> {
  match up.as_ref().map(|opened| &opened.owner) {
    Some(Owner::Resident { .. }) => {
      let Ok(()) = nothing_is_up(up);

      resources.remember(None)
    }
    Some(Owner::StandIn { .. }) | None => Ok(()),
  }
}

fn put_up(
  up: &mut Option<Up>,
  asked: Request,
  known: &Panel,
  mut owner: Owner,
  resources: &mut PanelResources,
) -> Result<(), Never> {
  let Ok(()) = owner.report(Progress::Accepted);

  let Ok(()) = console_panel::opening::start(
    &asked.who,
    asked.pressed.as_deref(),
    console_panel::opening::Came(&asked.from),
    asked.exec,
  );

  let Ok(card) = (known.card)(&asked.arguments);
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

      return owner.report(Progress::Failed);
    }
  };

  let Ok((first_frame, drawn)) = surface::drawing_notifications();
  let Ok(fonts) = resources.fonts.for_draw_thread();
  let Ok(previous) = resources.take_prepared(known.who);
  let connected = match resources.connection.as_ref() {
    Some(spare) => {
      let Ok(held) = spare.take();
      held
    }
    None => None,
  };
  let (remember, prepared) = mpsc::sync_channel(1);
  let Ok((shut, surface_thread)) = surface::run(
    &asked.who,
    (build, resources.tasks.clone()),
    column,
    start.as_deref(),
    surface::HostedDrawing {
      fonts,
      while_it_is_up: told_when_it_is_not,
      first_frame,
      preparation: surface::Preparing {
        previous,
        remember,
        connected,
      },
    },
    asked.tells,
  );

  let Ok(clock) = console_waiting::clock::LiveClock::started();
  let presented = wait_for_drawing(&drawn, &clock, picker::COMING);
  let mut incoming = Up {
    who: asked.who.clone(),
    shut,
    thread: surface_thread,
    owner,
    while_it_is_up,
    done: Some(done),
  };

  let Ok(()) = match presented {
    Ok(()) => {
      match prepared.try_recv() {
        Ok(frame) => {
          let Ok(()) = resources.keep_prepared(known.who, frame);
        }
        Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => {
          console_core_logging::trace!("{} did not retain its first frame", known.who);
        }
      }

      let Ok(()) = nothing_is_up(up);
      let Ok(()) = incoming.owner.report(Progress::Visible);
      *up = Some(incoming);

      let Ok(()) = resources.remember(Some(&LeftOpen {
        who: asked.who,
        arguments: asked.arguments,
      }));

      Ok(())
    }
    Err(fault) => {
      console_core_logging::warn!("{} did not present its first frame: {fault:?}", asked.who);

      let recover = matches!(&incoming.owner, Owner::Resident { .. }).then_some(known.program);
      let Ok(()) = finish(incoming, Finish::Unpresented(fault));

      match recover {
        Some(program) => left_open::started(program, asked.arguments),
        None => Ok(()),
      }
    }
  };

  match resources.connection.as_ref() {
    Some(spare) => spare.replenish(),
    None => Ok(()),
  }
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

  let reader = match &mut up_3.owner {
    Owner::StandIn { reader, .. } => reader,
    Owner::Resident { .. } => return Ok(Word::None),
  };

  Ok(match reader.read_line(&mut line) {
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
    mut owner,
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
        Finish::Unpresented(_) => owner.report(Progress::Failed),
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
    Finish::Presented => owner.report(Progress::Gone),
    Finish::Unpresented(fault) => {
      let Ok(progress) = drawing_ended(fault);
      owner.report(progress)
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
      (&listening, None),
      None,
      Some(Descriptors {
        panel: Some(reader.get_ref().as_fd()),
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
      (&listening, None),
      None,
      Some(Descriptors {
        panel: Some(asked_by.as_fd()),
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
      let Ok(program) = panel.program.name();
      let leaf = match panel.who.strip_prefix("console-") {
        Some(name) => name,
        None => panel.who,
      };

      assert!(
        program == panel.who || program.strip_prefix("console-application-") == Some(leaf),
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

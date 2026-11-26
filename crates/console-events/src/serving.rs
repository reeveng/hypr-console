//! The pool over a socket: who is connected, what they asked for, and what to
//! do when a source says something.  Split from the program so that it can be
//! started on a socket of a test's own, with sources of a test's own. What that
//! buys is the one thing the unit tests in `pool` cannot ask: that a program
//! which asks for an event group over a real socket is told the last word on it
//! *before* anything changes, which is the whole reason the pool remembers
//! anything.  Where the words come from is handed in, the same way
//! `console_input_controller::turning` is handed a machine. There is no other
//! way to ask what the pool does about a compositor that says something,
//! because the only compositor is the one this is running under.
//!
//! ## One thread, asleep in `poll`
//!
//! This used to be a thread per program to read it, a thread per program to
//! write to it, a thread per event group to carry a source's words across, and
//! one for the door: two threads a subscriber, most of them asleep in a
//! blocking call, and every word crossing a channel twice before it reached a
//! socket. The pool was always decided on one thread. Now it is heard and told
//! on that thread as well, which waits in one `poll` over the door, every
//! connection, and a pipe that a source's word wakes it through.
//!
//! What sources say still arrives on a sync_channel, because a source is a thread of
//! its own that this does not own and `Holding` hands it a `SyncSender`. One thread
//! carries every event group's words across and writes a byte down the pipe,
//! which is the one place a channel meets `poll`.
//!
//! ## One program that stops reading is not everyone's silence
//!
//! Everyone is told from this one loop, and a blocking write into a socket
//! whose reader has gone to sleep waits once the kernel's buffer for it is
//! full. That is not the slow program's problem, it is everyone's: the loop
//! that would have told the others is inside that write. On a desktop it reads
//! as the volume freezing on the bar because a panel behind a picker stopped
//! reading, which is a fault in the last place anyone would look for it.
//!
//! So no socket here blocks. A write takes what the kernel will take and the
//! rest waits in that program's outbox until `poll` says there is room, and
//! the slowest subscriber on the machine costs everyone else nothing.
//!
//! What is bounded is the bytes a program has been sent and not read, rather
//! than a count of words: a burst of short lines is nothing to hold and a
//! program that has stopped reading is megabytes within seconds, and the second
//! is the one worth ending. Bounding the count instead let a burst end healthy
//! subscriptions -- a hundred thousand words handed over at once dropped every
//! program on the machine and had them all reconnecting a second later, which
//! measured as a fortieth of the throughput and half the words lost.
//!
//! **A full outbox ends the connection rather than dropping the word.** These
//! words are what is true now, so a program that missed some of them holds a
//! picture that is wrong and has no way to find out -- dropping a line is
//! silent and permanent. Being let go is the recoverable one, and every piece
//! of that is already here: `console-core-reconnect` brings the program back,
//! `Client` asks for its event groups again, the pool replays the last word on
//! each, and `Heard::GotIn` tells it there was a gap. It comes back knowing
//! what is true instead of carrying on with what it missed.
//!
//! **A burst is written in pieces rather than a line at a time.** Words that
//! are already waiting are gathered as they are read, spelled once each, and
//! appended to every outbox that wants them; the outboxes are written when
//! nothing else is waiting or when [`BATCH`] has gathered, whichever comes
//! first. One word alone is still one write, so nothing about a quiet desktop
//! changes; a thousand words in a burst is one write per program instead of a
//! thousand. [`BATCH`] exists because a burst gathered whole is a burst that
//! starts arriving only when it has finished, so it bounds the wait as well as
//! the memory.
//!
//! ## A door that cannot open is not a door that is quiet
//!
//! `accept` takes the descriptor it is going to need before it does anything
//! else, so a process with none left is refused with `EMFILE` before the queue
//! is even looked at, and nothing about asking again changes that answer. Under
//! `poll` that is worse than it was under a blocking `accept`: the door stays
//! readable for as long as someone is queued at it, so a refusal is a `poll`
//! that returns at once, for ever, in the one thread everything else needs.
//!
//! So a door that refuses is left out of the next [`BREATH`] of waiting. It is
//! long enough that a door which cannot open costs nothing and short enough
//! that a program waiting to be let in does not notice, and the wait is the
//! whole of it: descriptors come back, the connection that was queued is
//! taken, and the pool carries on. Saying so once rather than every turn is the
//! other half, because a fault that repeats faster than it can be read is a
//! fault no one reads.

use std::collections::{BTreeMap, VecDeque};
use std::fs::File;
use std::io::{ErrorKind, IoSlice, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::time::Duration;

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, fixed_index, index};
use console_program_contract::{Change, EventGroup};
use console_program_lifetime::threads;
use console_waiting::latch::{self, Latch};
use rustix::event::{Nsecs, PollFd, PollFlags, Secs, Timespec, poll};

use crate::Unserved;
use crate::ipc::{self, Fault, Reply, Request, Trace};
use crate::pool::{Pool, Who};
use crate::sources::Subscribed;
use crate::wire::{self, Message};
use crate::workspace::{self, Published};

const BREATH: Duration = Duration::from_millis(200);

pub const OUTBOX: u64 = 4 << 20;

pub const BATCH: u64 = 64 << 10;

pub const REQUEST_BYTES: u64 = 16 << 10;

const READS_PER_TURN: u32 = 4;
const WRITE_BYTES: u64 = 64 << 10;
const ACCEPTS_PER_TURN: u32 = 32;
const PUBLICATIONS_PER_TURN: u32 = 64;
const IPC_FAULT_RESERVE: u64 = 128;

struct Client {
  connection: UnixStream,
  heard: Vec<u8>,
  outbox: VecDeque<u8>,
  workspace: WorkspaceWatch,
}

pub type Holding = fn(&EventGroup, SyncSender<Change>) -> Result<Subscribed, Never>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Door {
  Open,
  Resting,
  Retrying,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rejected {
  NotYet,
  Interrupted,
  Gone,
}

fn refused(fault: &std::io::Error) -> Result<Rejected, Never> {
  let kind = fault.kind();

  Ok(
    match (
      kind == ErrorKind::WouldBlock,
      kind == ErrorKind::Interrupted,
    ) {
      (true, true) | (true, false) => Rejected::NotYet,
      (false, true) => Rejected::Interrupted,
      (false, false) => Rejected::Gone,
    },
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Watched {
  Latch,
  Door,
  Client(Who),
}

struct Serving<'source> {
  pool: Pool,
  clients: BTreeMap<Who, Client>,
  held: Vec<EventGroup>,
  sources: SyncSender<Change>,
  holding: &'source dyn Fn(&EventGroup, SyncSender<Change>) -> Result<Subscribed, Never>,
  carried: u64,
  publishing: Publication,
  workspace: WorkspaceDesk,
  trace: Trace,
}

enum WorkspaceWatch {
  Off,
  Current,
  Pending,
}

struct WorkspaceDesk {
  source: Option<workspace::Subscription>,
  latest: Published,
  line: String,
  wake: File,
}

#[derive(Clone, Copy)]
enum Publication {
  Idle,
  Queued,
}

pub fn serve(
  socket: &Path,
  holding: impl Fn(&EventGroup, SyncSender<Change>) -> Result<Subscribed, Never>,
) -> Result<(), Unserved> {
  serve_traced(socket, holding, Trace::Quiet)
}

pub fn serve_traced(
  socket: &Path,
  holding: impl Fn(&EventGroup, SyncSender<Change>) -> Result<Subscribed, Never>,
  trace: Trace,
) -> Result<(), Unserved> {
  let listening = bound(socket)?;
  let waking = latch::pipe().map_err(Unserved::Waking)?;
  let Latch { waiting, saying } = waking;
  let workspace_wake = saying.try_clone().map_err(Unserved::Waking)?;
  let (sources, arriving) = sync_channel::<Change>(256);
  let (carrying, published) = sync_channel::<Change>(256);

  let Ok(()) = threads::let_go(std::thread::spawn(move || {
    let Ok(()) = relay_sources(arriving, carrying, saying);
  }));

  let Ok(latest) = stale();
  let serving = Serving {
    pool: Pool::default(),
    clients: BTreeMap::new(),
    held: Vec::new(),
    sources,
    holding: &holding,
    carried: 0,
    publishing: Publication::Idle,
    workspace: WorkspaceDesk {
      source: None,
      latest,
      line: String::new(),
      wake: workspace_wake,
    },
    trace,
  };

  let served = iterate((serving, Door::Open), |(mut serving_2, door)| {
    let ready = match ready(&serving_2, &listening, &waiting, door) {
      Ok(ready) => ready,
      Err(fault) => return Ok(Step::Halt(Err(fault))),
    };

    let mut door_2 = match door {
      Door::Resting => Door::Retrying,
      Door::Open | Door::Retrying => door,
    };

    for (watched, flags) in ready {
      match watched {
        Watched::Latch => {
          let Ok(()) = latch::drain(&waiting);
          serving_2.publishing = Publication::Queued;
          let Ok(()) = workspace_updates(&mut serving_2);

          match ::core::primitive::bool::then_some(
            flags.intersects(PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL),
            (),
          ) {
            ::core::option::Option::Some(()) => {
              let fault = std::io::Error::from(ErrorKind::BrokenPipe);

              return Ok(Step::Halt(Err(Unserved::Waking(fault))));
            }
            ::core::option::Option::None => {}
          }
        }
        Watched::Door => {
          let Ok(let_in) = let_in(&mut serving_2, &listening, door_2);

          door_2 = let_in;
        }
        Watched::Client(who) => {
          let Ok(()) = read_client(&mut serving_2, who, flags);
        }
      }
    }

    match serving_2.publishing {
      Publication::Idle => {}
      Publication::Queued => {
        let Ok(publishing) = publish_pending(&mut serving_2, &published);
        serving_2.publishing = publishing;
      }
    }

    let Ok(()) = flush(&mut serving_2);

    Ok(Step::Again((serving_2, door_2)))
  });

  match served {
    Ok(served_2) => served_2,
    Err(Endless) => Ok(()),
  }
}

#[cfg_attr(
  dylint_lib = "explicit055_no_unbounded_loop",
  expect(
    explicit055_no_unbounded_loop,
    reason = "the daemon stores the source sender and its receiver ends when the daemon drops it; a failed forward or wake ends the relay too"
  )
)]
fn relay_sources(
  arriving: Receiver<Change>,
  carrying: SyncSender<Change>,
  mut saying: std::fs::File,
) -> Result<(), Never> {
  for change in arriving {
    match carrying.send(change) {
      Ok(()) => {}
      Err(_no_one_is_listening) => return Ok(()),
    }

    match saying.write_all(&[1]) {
      Ok(()) => {}
      Err(fault) => {
        match ::core::primitive::bool::then_some(fault.kind() == ErrorKind::WouldBlock, ()) {
          ::core::option::Option::Some(()) => continue,
          ::core::option::Option::None => {
            console_core_logging::warn!("console-events: could not wake the pool: {fault}");

            return Ok(());
          }
        }
      }
    }
  }

  Ok(())
}

fn bound(socket: &Path) -> Result<UnixListener, Unserved> {
  let at = match socket.parent() {
    Some(at) => at,
    None => return Err(Unserved::Rootless),
  };
  let made = std::fs::create_dir_all(at);

  made.map_err(|fault| Unserved::Holding(at.to_path_buf(), fault))?;

  let deleted = std::fs::remove_file(socket);

  match deleted {
    Ok(()) => {}
    Err(fault) => {
      match ::core::primitive::bool::then_some(fault.kind() == ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          return Err(Unserved::Unbound(socket.to_path_buf(), fault));
        }
      }
    }
  }

  let listening =
    UnixListener::bind(socket).map_err(|fault| Unserved::Unbound(socket.to_path_buf(), fault))?;
  let nonblocking = listening.set_nonblocking(true);

  nonblocking.map_err(|fault| Unserved::Unbound(socket.to_path_buf(), fault))?;

  Ok(listening)
}

fn ready(
  serving: &Serving<'_>,
  listening: &UnixListener,
  waiting: &std::os::fd::OwnedFd,
  door: Door,
) -> Result<Vec<(Watched, PollFlags)>, Unserved> {
  let mut watch = vec![PollFd::new(waiting, PollFlags::IN)];
  let mut which = vec![Watched::Latch];

  match door {
    Door::Open | Door::Retrying => {
      watch.push(PollFd::new(listening, PollFlags::IN));
      which.push(Watched::Door);
    }
    Door::Resting => {}
  }

  for (who, client) in &serving.clients {
    let flags = match (client.outbox.is_empty()
      && !matches!(client.workspace, WorkspaceWatch::Pending))
    .then_some(())
    {
      ::core::option::Option::Some(()) => PollFlags::IN,
      ::core::option::Option::None => PollFlags::IN | PollFlags::OUT,
    };

    watch.push(PollFd::from_borrowed_fd(client.connection.as_fd(), flags));
    which.push(Watched::Client(*who));
  }

  let Ok(breath) = breath(door, serving.publishing);

  match poll(&mut watch, breath.as_ref()) {
    Ok(_) => {}
    Err(rustix::io::Errno::INTR) => return Ok(Vec::new()),
    Err(fault) => return Err(Unserved::Waiting(fault)),
  }

  Ok(
    which
      .into_iter()
      .zip(watch.iter().map(|fd| fd.revents()))
      .filter(|(_, flags)| !flags.is_empty())
      .collect(),
  )
}

fn breath(door: Door, publishing: Publication) -> Result<Option<Timespec>, Never> {
  Ok(match (door, publishing) {
    (_, Publication::Queued) => Some(Timespec {
      tv_sec: 0,
      tv_nsec: 0,
    }),
    (Door::Resting, Publication::Idle) => {
      let Ok(seconds) = fitted::<u64, Secs>(BREATH.as_secs());
      let Ok(nanoseconds) = fitted::<u32, Nsecs>(BREATH.subsec_nanos());

      Some(Timespec {
        tv_sec: seconds,
        tv_nsec: nanoseconds,
      })
    }
    (Door::Open | Door::Retrying, Publication::Idle) => None,
  })
}

fn let_in(
  serving: &mut Serving<'_>,
  listening: &UnixListener,
  door: Door,
) -> Result<Door, Never> {
  let Ok(budget) = index(ACCEPTS_PER_TURN);

  for accepted in listening.incoming().take(budget) {
    let fault = match accepted {
      Ok(stream) => {
        let Ok(()) = arrived(serving, stream);

        continue;
      }
      Err(fault) => fault,
    };

    let Ok(refused) = refused(&fault);

    return Ok(match (refused, door) {
      (Rejected::NotYet, Door::Open | Door::Resting) => door,
      (Rejected::NotYet, Door::Retrying) => Door::Open,
      (Rejected::Interrupted, Door::Open | Door::Resting | Door::Retrying) => continue,
      (Rejected::Gone, Door::Open) => {
        eprintln!("console-events: no one can be let in: {fault}");

        Door::Resting
      }
      (Rejected::Gone, Door::Resting | Door::Retrying) => Door::Resting,
    });
  }

  Ok(Door::Open)
}

fn arrived(serving: &mut Serving<'_>, stream: UnixStream) -> Result<(), Never> {
  match stream.set_nonblocking(true) {
    Ok(()) => {}
    Err(fault) => {
      eprintln!("console-events: a program connected and could not be read: {fault}");

      return Ok(());
    }
  }

  let Ok(who) = serving.pool.joined();

  let _ = serving.clients.insert(
    who,
    Client {
      connection: stream,
      heard: Vec::new(),
      outbox: VecDeque::new(),
      workspace: WorkspaceWatch::Off,
    },
  );

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "arrived sets each client descriptor nonblocking before accepting requests"
  )
)]
fn read_client(serving: &mut Serving<'_>, who: Who, flags: PollFlags) -> Result<(), Never> {
  match ::core::primitive::bool::then_some(
    flags.intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL),
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(()),
  }

  let mut buffer = [0_u8; 8 << 10];

  for _attempt in 0..READS_PER_TURN {
    let socket = match serving.clients.get_mut(&who) {
      Some(client) => &mut client.connection,
      None => return Ok(()),
    };
    let bytes = match socket.read(&mut buffer) {
      Ok(0) => return let_go(serving, who),
      Ok(many) => match buffer.get(..many) {
        Some(present) => present,
        None => return let_go(serving, who),
      },
      Err(fault) => {
        let Ok(refusal) = refused(&fault);

        match refusal {
          Rejected::NotYet => return Ok(()),
          Rejected::Interrupted => continue,
          Rejected::Gone => return let_go(serving, who),
        }
      }
    };

    for part in bytes.split_inclusive(|byte| *byte == b'\n') {
      let client = match serving.clients.get_mut(&who) {
        Some(client) => client,
        None => return Ok(()),
      };

      match frame(&mut client.heard, part, serving.trace) {
        Ok(Some(Frame::Legacy(message))) => {
          let Ok(()) = received(serving, who, message);
        }
        Ok(Some(Frame::Versioned { serial, request })) => {
          let Ok(()) = requested(serving, who, serial, request);
        }
        Ok(None) => {}
        Err(FrameError::TooLong) => {
          console_core_logging::warn!("console-events: {who} exceeded the request limit");

          return let_go(serving, who);
        }
        Err(FrameError::Invalid) => {
          console_core_logging::warn!("console-events: {who} sent an invalid request");

          return let_go(serving, who);
        }
      }
    }
  }

  Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum FrameError {
  TooLong,
  Invalid,
}

struct RequestSerial(u64);

enum Frame {
  Legacy(Message),
  Versioned {
    serial: RequestSerial,
    request: Result<Request, Fault>,
  },
}

fn frame(heard: &mut Vec<u8>, part: &[u8], trace: Trace) -> Result<Option<Frame>, FrameError> {
  let terminated = part.strip_suffix(b"\n");
  let bytes = match terminated {
    Some(bytes) => bytes,
    None => part,
  };
  let Ok(total) = fixed_index(heard.len().saturating_add(bytes.len()));

  match (total <= REQUEST_BYTES).then_some(()) {
    Some(()) => heard.extend_from_slice(bytes),
    None => return Err(FrameError::TooLong),
  }

  match (terminated, heard.is_empty()) {
    (None, _) | (Some(_), true) => return Ok(None),
    (Some(_), false) => {}
  }

  let decoded = match std::str::from_utf8(heard) {
    Ok(line) => decode_request(line, trace),
    Err(_invalid_utf8) => Err(FrameError::Invalid),
  };
  heard.clear();

  decoded.map(Some)
}

fn decode_request(line: &str, trace: Trace) -> Result<Frame, FrameError> {
  match line.starts_with("ipc ").then_some(()) {
    Some(()) => {
      let Ok(()) = ipc::trace_line(line, trace, ipc::Direction::Receive);
      Request::read(line)
        .map(|(serial, request)| Frame::Versioned {
          serial: RequestSerial(serial),
          request,
        })
        .map_err(|_malformed_header| FrameError::Invalid)
    }
    None => {
      let Ok(message) = wire::decoded(line);

      message.map(Frame::Legacy).ok_or(FrameError::Invalid)
    }
  }
}

fn stale() -> Result<Published, Never> {
  Ok(Published {
    payload: ipc::STALE_PAYLOAD.to_owned(),
    fresh_at: None,
  })
}

impl WorkspaceDesk {
  fn start(&mut self) -> Result<(), Fault> {
    match &self.source {
      Some(_) => return Ok(()),
      None => {}
    }

    let wake = self.wake.try_clone().map_err(|_| Fault::Unavailable)?;
    let source = workspace::Subscription::start(wake).map_err(|fault| {
      console_core_logging::warn!("console-events: starting the workspace source: {fault}");
      Fault::Unavailable
    })?;
    self.source = Some(source);

    Ok(())
  }
}

fn requested(
  serving: &mut Serving<'_>,
  who: Who,
  serial: RequestSerial,
  request: Result<Request, Fault>,
) -> Result<(), Never> {
  let reply = match request {
    Ok(request_2) => answer_request(serving, who, &serial, request_2),
    Err(fault) => Err(fault),
  };
  let Ok(snapshot_bytes) = fixed_index(serving.workspace.line.len());
  let waiting = match serving.clients.get(&who) {
    Some(client) => {
      let Ok(waiting) = queued(client, snapshot_bytes);

      waiting
    }
    None => return Ok(()),
  };
  let line = match reply {
    Ok(line) => {
      let Ok(bytes) = fixed_index(line.len());
      let fits = bytes <= u64::from(ipc::REPLY_BYTES)
        && waiting.saturating_add(bytes) <= OUTBOX.saturating_sub(IPC_FAULT_RESERVE);

      match fits.then_some(()) {
        Some(()) => Ok(line),
        None => Reply::Fault(Fault::Overloaded).line(serial.0),
      }
    }
    Err(fault) => Reply::Fault(fault).line(serial.0),
  };

  match line {
    Ok(line_2) => {
      let Ok(()) = ipc::trace_line(&line_2, serving.trace, ipc::Direction::Send);

      send(serving, who, line_2.as_bytes())
    }
    Err(fault) => {
      console_core_logging::warn!("console-events: formatting IPC reply: {fault}");

      let_go(serving, who)
    }
  }
}

fn answer_request(
  serving: &mut Serving<'_>,
  who: Who,
  serial: &RequestSerial,
  request: Request,
) -> Result<String, Fault> {
  match request {
    Request::Ping => Reply::Pong
      .line(serial.0)
      .map_err(|_not_serializable| Fault::Unavailable),
    Request::GetWorkspace { max_age_ms } => {
      serving.workspace.start()?;
      let latest = &serving.workspace.latest;
      let current = latest
        .fresh_at
        .filter(|stamp| stamp.elapsed() <= Duration::from_millis(u64::from(max_age_ms)));
      let payload = match current {
        Some(_within_the_limit) => latest.payload.as_str(),
        None => ipc::STALE_PAYLOAD,
      };

      let Ok(line) = ipc::workspace_frame(payload, serial.0);

      Ok(line)
    }
    Request::WatchWorkspace => {
      serving.workspace.start()?;
      let client = serving.clients.get_mut(&who).ok_or(Fault::Unavailable)?;
      client.workspace = WorkspaceWatch::Current;
      let Ok(line) = ipc::workspace_frame(&serving.workspace.latest.payload, serial.0);

      Ok(line)
    }
  }
}

fn workspace_updates(serving: &mut Serving<'_>) -> Result<(), Never> {
  for _publication in 0..4 {
    let outcome = match &serving.workspace.source {
      Some(source) => source.updates.try_recv(),
      None => break,
    };
    let published = match outcome {
      Ok(update) => update,
      Err(TryRecvError::Empty) => break,
      Err(TryRecvError::Disconnected) => {
        serving.workspace.source = None;
        let Ok(lost) = stale();

        lost
      }
    };
    let Ok(()) = publish_workspace(serving, published);
  }

  Ok(())
}

fn publish_workspace(serving: &mut Serving<'_>, update: Published) -> Result<(), Never> {
  let Ok(encoded) = ipc::workspace_frame(&update.payload, 0);
  let Ok(size) = fixed_index(encoded.len());
  let (published, line) = match (size <= u64::from(ipc::REPLY_BYTES)).then_some(()) {
    Some(()) => (update, encoded),
    None => {
      console_core_logging::warn!("console-events: workspace snapshot exceeded its wire limit");
      let Ok(lost) = stale();
      let Ok(smaller) = ipc::workspace_frame(&lost.payload, 0);

      (lost, smaller)
    }
  };
  let Ok(()) = ipc::trace_line(&line, serving.trace, ipc::Direction::Publish);
  let Ok(bytes) = fixed_index(line.len());
  serving.workspace.latest = published;
  serving.workspace.line = line;

  let mut gone = Vec::new();

  for (who, client) in &mut serving.clients {
    match client.workspace {
      WorkspaceWatch::Off => {}
      WorkspaceWatch::Current | WorkspaceWatch::Pending => {
        let Ok(outbox) = fixed_index(client.outbox.len());

        match (outbox.saturating_add(bytes) <= OUTBOX).then_some(()) {
          Some(()) => client.workspace = WorkspaceWatch::Pending,
          None => gone.push(*who),
        }
      }
    }
  }

  for who in gone {
    let Ok(()) = let_go(serving, who);
  }

  Ok(())
}

fn let_go(serving: &mut Serving<'_>, who: Who) -> Result<(), Never> {
  let Ok(()) = serving.pool.remove(who);
  let _ = serving.clients.remove(&who);

  Ok(())
}

fn received(serving: &mut Serving<'_>, who: Who, message: Message) -> Result<(), Never> {
  match message {
    Message::Subscribe(event_group) => {
      let Ok(replay) = serving.pool.subscribe(who, event_group.clone());
      let Ok(()) = hold(serving, &event_group);

      match replay {
        Some(change) => {
          let Ok(spelled) = wire::encoded(&Message::Publish(change));
          let Ok(()) = send(serving, who, format!("{spelled}\n").as_bytes());
        }
        None => {}
      }
    }
    Message::Unsubscribe(event_group) => {
      let Ok(()) = serving.pool.unsubscribe(who, &event_group);
    }
    Message::Publish(_) => eprintln!("console-events: {who} tried to tell the pool something"),
  }

  Ok(())
}

fn hold(serving: &mut Serving<'_>, event_group: &EventGroup) -> Result<(), Never> {
  match ::core::primitive::bool::then_some(serving.held.contains(event_group), ()) {
    ::core::option::Option::Some(()) => return Ok(()),
    ::core::option::Option::None => {}
  }

  let Ok(held_now) = (serving.holding)(event_group, serving.sources.clone());

  match held_now {
    Subscribed::Yes => serving.held.push(event_group.clone()),
    Subscribed::No => {
      let Ok(token) = wire::token(event_group);

      eprintln!(
        "console-events: nothing here watches {token}, so anyone listening to it will \
                 hear nothing"
      );
    }
  }

  Ok(())
}

fn publish_pending(
  serving: &mut Serving<'_>,
  published: &Receiver<Change>,
) -> Result<Publication, Never> {
  for _publication in 0..PUBLICATIONS_PER_TURN {
    let change = match published.try_recv() {
      Ok(change) => change,
      Err(TryRecvError::Empty | TryRecvError::Disconnected) => return Ok(Publication::Idle),
    };
    let Ok(()) = publish(serving, &change);

    match ::core::primitive::bool::then_some(serving.carried >= BATCH, ()) {
      ::core::option::Option::Some(()) => return Ok(Publication::Queued),
      ::core::option::Option::None => {}
    }
  }

  Ok(Publication::Queued)
}

fn publish(serving: &mut Serving<'_>, change: &Change) -> Result<(), Never> {
  let Ok(everyone) = serving.pool.publish(change);

  match everyone.first() {
    Some(_someone_asked_for_this) => {}
    None => return Ok(()),
  }

  let Ok(spelled) = wire::encoded(&Message::Publish(change.clone()));
  let line = format!("{spelled}\n");

  for who in everyone {
    let Ok(()) = send(serving, who, line.as_bytes());
  }

  Ok(())
}

fn send(serving: &mut Serving<'_>, who: Who, line: &[u8]) -> Result<(), Never> {
  let Ok(snapshot_bytes) = fixed_index(serving.workspace.line.len());
  let client = match serving.clients.get_mut(&who) {
    Some(client) => client,
    None => return Ok(()),
  };
  let Ok(waiting) = queued(client, snapshot_bytes);
  let Ok(bytes) = fixed_index(line.len());

  match (waiting.saturating_add(bytes) > OUTBOX).then_some(()) {
    Some(()) => {
      console_core_logging::warn!("console-events: {who} exceeded its outbox budget");

      return let_go(serving, who);
    }
    None => {}
  }

  client.outbox.extend(line.iter().copied());
  serving.carried = serving.carried.saturating_add(bytes);

  Ok(())
}

fn queued(client: &Client, snapshot_bytes: u64) -> Result<u64, Never> {
  let pending = match client.workspace {
    WorkspaceWatch::Pending => snapshot_bytes,
    WorkspaceWatch::Current | WorkspaceWatch::Off => 0,
  };
  let Ok(outbox) = fixed_index(client.outbox.len());

  Ok(outbox.saturating_add(pending))
}

fn flush(serving: &mut Serving<'_>) -> Result<(), Never> {
  serving.carried = 0;

  let mut gone: Vec<Who> = Vec::new();

  for (who, client) in &mut serving.clients {
    let Ok(wrote) = wrote(client, &serving.workspace.line);

    match wrote {
      Wrote::Retained => {}
      Wrote::Gone => gone.push(*who),
    }
  }

  for who in gone {
    let Ok(()) = let_go(serving, who);
  }

  Ok(())
}

enum Wrote {
  Retained,
  Gone,
}

fn wrote(client: &mut Client, newest: &str) -> Result<Wrote, Never> {
  match (client.outbox.is_empty(), &client.workspace) {
    (true, WorkspaceWatch::Pending) => {
      client.outbox.extend(newest.as_bytes().iter().copied());
      client.workspace = WorkspaceWatch::Current;
    }
    (true, WorkspaceWatch::Off | WorkspaceWatch::Current) => return Ok(Wrote::Retained),
    (false, _) => {}
  }

  let (head, tail) = client.outbox.as_slices();
  let Ok(head_size) = fixed_index(head.len());
  let Ok(tail_size) = fixed_index(tail.len());
  let head_bytes = head_size.min(WRITE_BYTES);
  let tail_bytes = tail_size.min(WRITE_BYTES.saturating_sub(head_bytes));
  let Ok(head_end) = index(head_bytes);
  let Ok(tail_end) = index(tail_bytes);
  let first = match head.get(..head_end) {
    Some(present) => present,
    None => return Ok(Wrote::Gone),
  };
  let second = match tail.get(..tail_end) {
    Some(present) => present,
    None => return Ok(Wrote::Gone),
  };
  let batch = [IoSlice::new(first), IoSlice::new(second)];

  Ok(match client.connection.write_vectored(&batch) {
    Ok(0) => Wrote::Gone,
    Ok(many) => {
      drop(client.outbox.drain(..many));

      Wrote::Retained
    }
    Err(fault) => match refused(&fault) {
      Ok(Rejected::NotYet | Rejected::Interrupted) => Wrote::Retained,
      Ok(Rejected::Gone) => Wrote::Gone,
    },
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn frames_are_bounded_before_appending_and_survive_read_boundaries() {
    let mut partial = Vec::new();
    assert!(matches!(
      frame(&mut partial, b"subsc", Trace::Quiet),
      Ok(None)
    ));
    assert!(matches!(
      frame(&mut partial, b"ribe sound\n", Trace::Quiet),
      Ok(Some(Frame::Legacy(Message::Subscribe(EventGroup::Sound))))
    ));
    assert!(partial.is_empty());

    let maximum = [b'x'; 16 << 10];
    assert!(matches!(
      frame(&mut partial, &maximum, Trace::Quiet),
      Ok(None)
    ));
    assert!(matches!(
      frame(&mut partial, b"more", Trace::Quiet),
      Err(FrameError::TooLong)
    ));
    assert_eq!(partial.len(), maximum.len());
  }
}

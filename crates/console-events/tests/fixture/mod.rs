//! A fake compositor and real relay in their own runtime directory.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, Instant};

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_events::ipc::{Client, Reply, Request, Trace, Workspace};
use console_program_lifetime::threads;
use console_waiting::latch;
use console_waiting::{Outcome, Ready, Schedule, until};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use serde_json::json;

pub type Failure = Box<dyn std::error::Error>;
pub const TIMEOUT: Duration = Duration::from_secs(5);
const SIGNATURE: &str = "benchmark";

pub struct Runtime {
  directory: PathBuf,
}

impl Runtime {
  pub fn new() -> Result<Self, Failure> {
    let directory = console_core_temporary_directories::fresh("ipc")?;

    Ok(Self { directory })
  }

  pub fn directory(&self) -> Result<&Path, Never> {
    Ok(&self.directory)
  }
}

impl Drop for Runtime {
  fn drop(&mut self) {
    let _cleaned = fs::remove_dir_all(&self.directory);
  }
}

pub struct Process {
  child: Child,
}

impl Process {
  pub fn start(command: &mut Command, socket: &Path) -> Result<Self, Failure> {
    let child = command.spawn()?;
    let started = Self { child };
    let Ok(schedule) = Schedule::asking_every(TIMEOUT, Duration::from_millis(5));
    let Ok(ready) = until(schedule, || {
      Ok(match fs::metadata(socket) {
        Ok(info) => match info.file_type().is_socket().then_some(()) {
          Some(()) => Ready::Yes,
          None => Ready::NotYet,
        },
        Err(_not_yet_created) => Ready::NotYet,
      })
    });

    match ready {
      Outcome::Happened => Ok(started),
      Outcome::RanOut => Err(Failure::from(format!(
        "no listener at {}",
        socket.display()
      ))),
    }
  }

  pub fn pid(&self) -> Result<u32, Never> {
    Ok(self.child.id())
  }
}

impl Drop for Process {
  fn drop(&mut self) {
    let _already_ended_or_killed = self.child.kill();
    let _reaped = self.child.wait();
  }
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the fixture driver sets a workspace ID observed by its fake compositor thread"
  )
)]
pub struct Fixture {
  pub socket: PathBuf,
  pub requests: PathBuf,
  events: UnixListener,
  front: Arc<AtomicI64>,
  _query_worker: File,
  pub process: Process,
  _runtime: Runtime,
}

impl Fixture {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the fixture and fake compositor share one atomic workspace ID"
    )
  )]
  pub fn start(binary: &Path) -> Result<Self, Failure> {
    let runtime = Runtime::new()?;
    let Ok(directory) = runtime.directory();
    let at = directory.join("hypr").join(SIGNATURE);
    fs::create_dir_all(&at)?;
    let events = UnixListener::bind(at.join(".socket2.sock"))?;
    events.set_nonblocking(true)?;
    let requests = at.join(".socket.sock");
    let query = UnixListener::bind(&requests)?;
    query.set_nonblocking(true)?;
    let socket = runtime.directory.join("console").join("events.sock");
    let process = Process::start(
      Command::new(binary)
        .env("XDG_RUNTIME_DIR", &runtime.directory)
        .env("HYPRLAND_INSTANCE_SIGNATURE", SIGNATURE),
      &socket,
    )?;
    let stop = latch::pipe()?;
    let front = Arc::new(AtomicI64::new(1));
    let shared = Arc::clone(&front);
    let thread = std::thread::Builder::new().spawn(move || {
      let served = serve_queries(&query, &stop.waiting, &shared);

      match served {
        Ok(()) => {}
        Err(fault) => eprintln!("fixture compositor: {fault}"),
      }
    })?;
    let Ok(()) = threads::let_go(thread);

    Ok(Self {
      socket,
      requests,
      events,
      front,
      process,
      _query_worker: stop.saying,
      _runtime: runtime,
    })
  }

  pub fn source(&self) -> Result<UnixStream, Failure> {
    let mut watched = [PollFd::new(&self.events, PollFlags::IN)];
    let timeout = Timespec {
      tv_sec: 5,
      tv_nsec: 0,
    };
    let ready =
      poll(&mut watched, Some(&timeout)).map(console_core_number_conversion::fixed_index)?;

    match ready {
      Ok(0) => Err(Failure::from("compositor event socket timed out")),
      Ok(_ready) => {
        let (stream, _address) = self.events.accept()?;
        stream.set_write_timeout(Some(TIMEOUT))?;

        Ok(stream)
      }
    }
  }

  pub fn change_front(&self, id: i64) -> Result<(), Never> {
    self.front.store(id, Ordering::SeqCst);

    Ok(())
  }

  pub fn watch(&self) -> Result<(Client, UnixStream, u64), Failure> {
    let mut watch = Client::at(&self.socket, Trace::Quiet)?;
    let initial = watch.call(Request::WatchWorkspace, TIMEOUT)?;
    let upstream = self.source()?;

    match initial {
      Reply::Workspace(Workspace::Stale) => {
        let generation = fresh(&mut watch, 1, 0)?;

        Ok((watch, upstream, generation))
      }
      Reply::Workspace(Workspace::Fresh { .. }) | Reply::Pong | Reply::Fault(_) => {
        Err(Failure::from("the fixture relay did not start stale"))
      }
    }
  }
}

pub fn fresh(client: &mut Client, workspace: i64, previous: u64) -> Result<u64, Failure> {
  let deadline = Instant::now()
    .checked_add(TIMEOUT)
    .ok_or("clock overflow")?;
  let update = client.next_workspace(Some(deadline))?;

  match update {
    Workspace::Fresh {
      generation,
      revision: 1,
      snapshot,
    } => {
      let is_new = generation > previous
        && snapshot
          .front
          .as_ref()
          .is_some_and(|front| front.id == workspace);

      match is_new.then_some(()) {
        Some(()) => Ok(generation),
        None => Err(Failure::from("the workspace snapshot did not advance")),
      }
    }
    Workspace::Fresh { .. } | Workspace::Stale => {
      Err(Failure::from("unexpected workspace generation"))
    }
  }
}

fn serve_queries(listener: &UnixListener, stop: &OwnedFd, front: &AtomicI64) -> io::Result<()> {
  let served = iterate((), |()| {
    let ready = query_turn(listener, stop, front);

    Ok(match ready {
      Ok(Ready::Yes) => Step::Again(()),
      Ok(Ready::NotYet) => Step::Halt(Ok(())),
      Err(fault) => Step::Halt(Err(fault)),
    })
  });

  match served {
    Ok(outcome) => outcome,
    Err(Endless) => Err(io::Error::from(io::ErrorKind::Other)),
  }
}

fn query_turn(listener: &UnixListener, stop: &OwnedFd, front: &AtomicI64) -> io::Result<Ready> {
  let mut watching = [
    PollFd::new(stop, PollFlags::IN),
    PollFd::new(listener, PollFlags::IN),
  ];
  poll(&mut watching, None)?;
  let [stopped, _incoming] = watching;

  match stopped.revents().is_empty().then_some(()) {
    Some(()) => {
      let (mut stream, _address) = listener.accept()?;
      let id = front.load(Ordering::SeqCst);
      answer(&mut stream, id)?;

      Ok(Ready::Yes)
    }
    None => Ok(Ready::NotYet),
  }
}

const QUERIES: [&[u8]; 5] = [
  b"[[BATCH]]j/activeworkspace;j/workspaces;j/clients;j/monitors",
  b"j/activeworkspace",
  b"j/workspaces",
  b"j/clients",
  b"j/monitors",
];

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the fixture gives each accepted stream a read deadline and rejects any prefix exceeding its query budget"
  )
)]
fn answer(stream: &mut UnixStream, id: i64) -> io::Result<()> {
  stream.set_read_timeout(Some(TIMEOUT))?;
  stream.set_write_timeout(Some(TIMEOUT))?;
  let front = json!({"id": id, "name": id.to_string(), "windows": 0});
  let workspaces = json!([&front]);
  let clients = json!([]);
  let monitors = json!([{"name": "eDP-1", "scale": 1.5}]);
  let batch = format!("{front}\n\n\n{workspaces}\n\n\n{clients}\n\n\n{monitors}");
  let replies = [
    batch,
    front.to_string(),
    workspaces.to_string(),
    clients.to_string(),
    monitors.to_string(),
  ];
  let mut called = Vec::new();
  let mut one = [0_u8; 1];

  for _byte in 0..128 {
    stream.read_exact(&mut one)?;
    called.extend(one);

    match QUERIES.iter().position(|line| *line == called) {
      Some(found) => {
        let reply = replies.get(found).ok_or(io::ErrorKind::InvalidData)?;

        return stream.write_all(reply.as_bytes());
      }
      None => {}
    }
  }

  Err(io::Error::from(io::ErrorKind::InvalidData))
}

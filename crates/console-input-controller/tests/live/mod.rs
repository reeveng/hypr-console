//! The daemon as a program, against devices the kernel really made.
//!
//! The fast tier runs the loop in this process against a world that is not this
//! machine's. It answers what the daemon decides. It cannot answer whether the
//! devices the emulator builds are the ones the daemon goes looking for, because
//! in that tier the daemon is handed them.
//!
//! This is the other half, and it is the whole path: uinput devices built from
//! the capture of the real four, the daemon started as its own program with
//! nothing told to it, and what comes out read back off a device the kernel
//! published.
//!
//! Nothing reaches the desktop you are sitting at. The daemon's output device is
//! grabbed the moment it appears, and a grabbed device delivers to the one that
//! grabbed it and to nothing else.

use std::collections::BTreeSet;
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use console_core_never::Never;
use console_input_event_devices::{Device, EventType, InputEvent};
use console_input_gamepad::capture::load_capture;
use console_input_gamepad::devices::Devices;
use console_input_gamepad::go::LegionGo;
use console_input_gamepad::router::every_profile;
use console_input_gamepad::uinput::Uinput;
use console_operating_system_linux::program_lifetime::{BoundToParent, alongside};
use console_waiting::clock::TestClock;
use console_waiting::{Outcome, Ready, Schedule, until, until_handed, until_some};

pub type Failure = Box<dyn Error>;

pub const READS: [&str; 3] = ["keyboard", "pad", "touchpad"];

const PUBLISHED: &str = "controller-desktop";

const INSTEAD: [&str; 8] = [
  "controller-profile",
  "session-game",
  "hyprctl",
  "launcher",
  "console-brightness",
  "console-buttons",
  "console-screenshot",
  "settings-panel",
];

const INSTANCE: &str = "console-live";

const RECORDER: &str = "#!/bin/sh\n\
    {\n\
    \x20   printf '%s' \"${0##*/}\"\n\
    \x20   for argument in \"$@\"; do printf '\\t%s' \"$argument\"; done\n\
    \x20   printf '\\n'\n\
    } >> \"$CONSOLE_RAN\"\n";

const PATIENCE: Duration = Duration::from_secs(5);

fn root() -> Result<PathBuf, Never> {
  let from = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));

  match from.canonicalize() {
    Ok(found) => Ok(found),
    Err(_not_there) => Ok(from),
  }
}

fn every_device() -> Result<BTreeSet<PathBuf>, Never> {
  let Ok(every) = Device::every();

  Ok(every.into_iter().map(|device| device.path).collect())
}

fn wait_for(name: &str, since: &BTreeSet<PathBuf>) -> Result<Option<Device>, Never> {
  let Ok(patience) = Schedule::of(PATIENCE);

  until_some(patience, || {
    let Ok(every) = Device::every();

    Ok(
      every
        .into_iter()
        .filter(|device| !since.contains(&device.path))
        .find(|device| device.name.as_deref() == Some(name)),
    )
  })
}

#[cfg_attr(
  dylint_lib = "explicit086_no_climbing_join",
  expect(
    explicit086_no_climbing_join,
    reason = "INSTEAD is a fixed list of executable names without path separators"
  )
)]
fn instead_of_the_desktop(here: &Path) -> Result<PathBuf, Failure> {
  let bin = here.join("bin");

  std::fs::create_dir_all(&bin)?;
  let recorder = bin.join("recorder");

  console_core_atomic_writes::whole_with_permissions(
    &recorder,
    RECORDER.as_bytes(),
    std::fs::Permissions::from_mode(0o755),
  )?;

  for name in INSTEAD {
    std::os::unix::fs::symlink(&recorder, bin.join(name))?;
  }

  Ok(bin)
}

fn path_for(
  paths: &std::collections::BTreeMap<String, String>,
  role: &str,
) -> Result<String, Failure> {
  let path = paths
    .get(role)
    .ok_or_else(|| format!("the emulator made no {role}"))?;

  Ok(path.clone())
}

struct Recording {
  socket: Option<PathBuf>,
  at: PathBuf,
}

impl Recording {
  fn stop(&mut self) -> Result<(), Never> {
    match self.socket.take() {
      Some(socket) => match UnixStream::connect(&socket) {
        Ok(wake) => drop(wake),
        Err(fault) => eprintln!("the test compositor did not receive its stop signal: {fault}"),
      },
      None => {}
    }

    Ok(())
  }
}

impl Drop for Recording {
  fn drop(&mut self) {
    let Ok(()) = self.stop();
  }
}

fn a_compositor_that_writes_down(here: &Path) -> Result<Recording, Failure> {
  let instance = here.join("hypr").join(INSTANCE);
  let socket = instance.join(console_compositor::socket::Socket::Requests);
  let at = here.join("dispatches");

  std::fs::create_dir_all(&instance)?;
  console_core_atomic_writes::whole(&at, b"")?;
  #[cfg_attr(
    dylint_lib = "explicit040_no_torn_write",
    expect(
      explicit040_no_torn_write,
      reason = "the test compositor writes one stream of dispatches to this temporary log"
    )
  )]
  let mut recording = std::fs::OpenOptions::new().append(true).open(&at)?;
  let listener = UnixListener::bind(&socket)?;

  let Ok(()) = console_program_lifetime::threads::let_go(std::thread::spawn(move || {
    for asking in listener.incoming() {
      let mut stream = match asking {
        Ok(stream) => stream,
        Err(fault) => {
          eprintln!("the test compositor could not accept a connection: {fault}");

          return;
        }
      };
      let mut reading = BufReader::new(&stream);
      let line = match reading.fill_buf() {
        Ok([]) => return,
        Ok(bytes) => match std::str::from_utf8(bytes) {
          Ok(line) => line,
          Err(fault) => {
            eprintln!("the test compositor received invalid UTF-8: {fault}");

            continue;
          }
        },
        Err(fault) => {
          eprintln!("the test compositor could not read a request: {fault}");

          continue;
        }
      };
      let told = match line.strip_prefix('/') {
        Some(rest) => rest.replacen(' ', "\t", 1),
        None => line.replacen(' ', "\t", 1),
      };
      let entry = format!("hyprctl\t{told}\n");

      match recording.write_all(entry.as_bytes()) {
        Ok(()) => {}
        Err(fault) => {
          eprintln!("the test compositor could not record a request: {fault}");

          continue;
        }
      }

      match stream.write_all(b"ok") {
        Ok(()) => {}
        Err(fault) => eprintln!("the test compositor could not answer a request: {fault}"),
      }
    }
  }));

  Ok(Recording {
    socket: Some(socket),
    at,
  })
}

pub struct Running {
  go: LegionGo<Uinput, TestClock>,
  out: Option<Device>,
  said_at: PathBuf,
  process: Option<BoundToParent>,
  ran_at: PathBuf,
  asked: Recording,
  here: PathBuf,
}

impl Running {
  pub fn new() -> Result<Self, Failure> {
    let Ok(root) = root();
    let here = console_core_temporary_directories::private("controller-live")?;
    let ran_at = here.join("ran");
    let said_at = here.join("stderr");

    console_core_atomic_writes::whole(&ran_at, b"")?;
    console_core_atomic_writes::whole(&said_at, b"")?;
    #[cfg_attr(
      dylint_lib = "explicit040_no_torn_write",
      expect(
        explicit040_no_torn_write,
        reason = "the daemon appends stderr to a temporary test log that is read while the daemon runs"
      )
    )]
    let voice = std::fs::OpenOptions::new().append(true).open(&said_at)?;
    let seen = load_capture()?;
    let world = load_capture()?;
    let uinput = Uinput::of(&seen)?;
    let Ok(devices) = Devices::new(world, uinput);
    let Ok(paths) = devices.paths();
    let profiles = every_profile(&root)?;
    let go = LegionGo::new(
      profiles,
      devices,
      TestClock::default(),
      console_input_gamepad::router::NAME,
    )?;
    let asked = a_compositor_that_writes_down(&here)?;
    let Ok(was) = every_device();
    #[cfg_attr(
      dylint_lib = "explicit026_env_read_once",
      allow(
        explicit026_env_read_once,
        reason = "the daemon under test is handed the PATH this test was run with, with the recorder in front of it"
      )
    )]
    let path = std::env::var("PATH")?;
    let instead = instead_of_the_desktop(&here)?;
    let pad = path_for(&paths, "pad")?;
    let keys = path_for(&paths, "keyboard")?;
    let touchpad = path_for(&paths, "touchpad")?;
    let process = alongside(
      Command::new(env!("CARGO_BIN_EXE_controller-desktop"))
        .env("PATH", format!("{}:{path}", instead.display()))
        .env("CONSOLE_RAN", &ran_at)
        .env("XDG_RUNTIME_DIR", &here)
        .env(console_compositor::session::SIGNATURE, INSTANCE)
        .env("CONSOLE_PAD", pad)
        .env("CONSOLE_KEYS", keys)
        .env("CONSOLE_TOUCHPAD", touchpad)
        .stderr(Stdio::from(voice)),
    )?;
    let Ok(out) = wait_for(PUBLISHED, &was);

    match &out {
      Some(published) => published.grab()?,
      None => {}
    }

    let running = Running {
      go,
      out,
      said_at,
      process: Some(process),
      ran_at,
      asked,
      here,
    };
    let Ok(_) = running.reading();

    Ok(running)
  }

  fn reading(&self) -> Result<Outcome, Never> {
    let Ok(patience) = Schedule::of(PATIENCE);

    until(patience, || {
      let Ok(said) = self.said();

      Ok(
        match ::core::primitive::bool::then_some(
          READS
            .iter()
            .all(|name| said.contains(&format!("reading the {name}"))),
          (),
        ) {
          ::core::option::Option::Some(()) => Ready::Yes,
          ::core::option::Option::None => Ready::NotYet,
        },
      )
    })
  }

  pub fn ran(&self) -> Result<Vec<Vec<String>>, Failure> {
    let Ok(patience) = Schedule::of(PATIENCE);
    let Ok(_) = until(patience, || {
      let answered = [&self.ran_at, &self.asked.at]
        .into_iter()
        .any(|at| std::fs::read_to_string(at).is_ok_and(|held| held.contains('\n')));

      Ok(match ::core::primitive::bool::then_some(answered, ()) {
        ::core::option::Option::Some(()) => Ready::Yes,
        ::core::option::Option::None => Ready::NotYet,
      })
    });

    self.commands()
  }

  pub fn events(&mut self, duration: Duration) -> Result<Vec<InputEvent>, Failure> {
    let out = match self.out.as_mut() {
      Some(out) => out,
      None => return Ok(Vec::new()),
    };

    out.nonblocking()?;
    let mut every: Vec<InputEvent> = Vec::new();
    let mut failure = None;
    let Ok(window) = Schedule::asking_every(duration, Duration::from_millis(10));
    let Ok(_) = until_handed(
      window,
      &mut (out, &mut every, &mut failure),
      |(out_2, every_2, fault_slot)| {
        match out_2.read_events() {
          Ok(arrived) => every_2.extend(
            arrived
              .into_iter()
              .filter(|event| event.kind != EventType::SYNCHRONIZATION),
          ),
          Err(fault) => match ::core::primitive::bool::then_some(
            fault.kind() == std::io::ErrorKind::WouldBlock,
            (),
          ) {
            ::core::option::Option::Some(()) => return Ok(Ready::NotYet),
            ::core::option::Option::None => {
              **fault_slot = Some(fault);

              return Ok(Ready::Yes);
            }
          },
        }

        Ok(Ready::NotYet)
      },
    );

    match failure {
      Some(fault) => Err(Failure::from(fault)),
      None => Ok(every),
    }
  }

  pub fn total(
    &mut self,
    (kind, code): (EventType, u16),
    duration: Duration,
  ) -> Result<i32, Failure> {
    let events = self.events(duration)?;

    Ok(
      events
        .iter()
        .filter(|event| event.kind == kind && event.code == code)
        .fold(0, |sum, event| sum.saturating_add(event.value)),
    )
  }

  pub fn commands(&self) -> Result<Vec<Vec<String>>, Failure> {
    let recorded = std::fs::read_to_string(&self.ran_at)?;
    let asked = std::fs::read_to_string(&self.asked.at)?;

    Ok(
      recorded
        .split_inclusive('\n')
        .chain(asked.split_inclusive('\n'))
        .filter_map(|line| line.strip_suffix('\n'))
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').map(str::to_string).collect())
        .collect(),
    )
  }

  pub fn names(&self) -> Result<Vec<String>, Failure> {
    let ran = self.ran()?;

    Ok(
      ran
        .into_iter()
        .filter_map(|arguments| arguments.into_iter().next())
        .collect(),
    )
  }

  pub fn go(&mut self) -> Result<&mut LegionGo<Uinput, TestClock>, Never> {
    Ok(&mut self.go)
  }

  pub fn said(&self) -> Result<String, Never> {
    Ok(match std::fs::read_to_string(&self.said_at) {
      Ok(said) => said,
      Err(_diagnostic_is_unreadable) => String::new(),
    })
  }
}

impl Drop for Running {
  fn drop(&mut self) {
    match self.out.as_mut() {
      Some(out) => match out.ungrab() {
        Ok(()) => {}
        Err(fault) => eprintln!("the test output device could not be ungrabbed: {fault}"),
      },
      None => {}
    }

    drop(self.process.take());
    let Ok(()) = self.go.close();
    let Ok(()) = self.asked.stop();

    match std::fs::remove_dir_all(&self.here) {
      Ok(()) => {}
      Err(fault) => eprintln!("could not remove the controller test directory: {fault}"),
    }
  }
}

pub fn required() -> Result<Running, Failure> {
  let _ = std::fs::metadata("/etc/console-test-vm").map_err(|fault| {
    std::io::Error::new(
      fault.kind(),
      "virtual input requires a test VM. Run just test e2e-live",
    )
  })?;
  let running = Running::new()?;
  let Ok(said) = running.said();

  assert!(
    running.out.is_some(),
    "the daemon never published a device: {said}"
  );

  Ok(running)
}

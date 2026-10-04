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
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use console_core_never::Never;
use console_input_event_devices::{Device, EventType, InputEvent};
use console_input_gamepad::capture::load_capture;
use console_input_gamepad::devices::Devices;
use console_input_gamepad::go::LegionGo;
use console_waiting::clock::TestClock;
use console_input_gamepad::router::every_profile;
use console_input_gamepad::uinput::Uinput;
use console_program_lifetime::{BoundToParent, alongside};
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

type Received = Arc<Mutex<Vec<String>>>;

fn root() -> Result<PathBuf, Never> {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

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

        Ok(every
            .into_iter()
            .filter(|device| !since.contains(&device.path))
            .find(|device| device.name.as_deref() == Some(name)))
    })
}

fn instead_of_the_desktop(here: &Path) -> Result<PathBuf, Failure> {
    let bin = here.join("bin");

    std::fs::create_dir_all(&bin)?;
    let recorder = bin.join("recorder");

    console_core_atomic_writes::whole(&recorder, RECORDER.as_bytes())?;
    std::fs::set_permissions(&recorder, std::fs::Permissions::from_mode(0o755))?;

    for name in INSTEAD {
        std::os::unix::fs::symlink(&recorder, bin.join(name))?;
    }

    Ok(bin)
}

fn path_for(paths: &std::collections::BTreeMap<String, String>, role: &str) -> Result<String, Failure> {
    let path = paths.get(role).ok_or_else(|| format!("the emulator made no {role}"))?;

    Ok(path.clone())
}

fn held(heard: &Received) -> Result<Vec<String>, Never> {
    Ok(match heard.lock() {
        Ok(heard) => heard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    })
}

fn a_compositor_that_writes_down(here: &Path) -> Result<Received, Failure> {
    let instance = here.join("hypr").join(INSTANCE);

    std::fs::create_dir_all(&instance)?;
    let listener = UnixListener::bind(instance.join(".socket.sock"))?;
    let heard: Received = Arc::new(Mutex::new(Vec::new()));
    let writing = Arc::clone(&heard);

    let Ok(()) = console_program_lifetime::threads::let_go(std::thread::spawn(move || {
        for asking in listener.incoming() {
            let mut asking = match asking {
                Ok(asking) => asking,
                Err(_gone) => return,
            };

            let line = {
                let mut reading = BufReader::new(&asking);

                match reading.fill_buf() {
                    Ok(said) => String::from_utf8_lossy(said).to_string(),
                    Err(_unread) => String::new(),
                }
            };

            let told = match line.strip_prefix('/') {
                Some(rest) => rest.replacen(' ', "\t", 1),
                None => line.replacen(' ', "\t", 1),
            };

            match writing.lock() {
                Ok(mut heard) => heard.push(format!("hyprctl\t{told}")),
                Err(poisoned) => poisoned.into_inner().push(format!("hyprctl\t{told}")),
            }

            let _ = asking.write_all(b"ok");
        }
    }));

    Ok(heard)
}

pub struct Running {
    pub go: LegionGo<Uinput, TestClock>,
    pub out: Option<Device>,
    said: Arc<Mutex<String>>,
    process: Option<BoundToParent>,
    ran_at: PathBuf,
    asked: Received,
    here: PathBuf,
}

impl Running {
    pub fn new() -> Result<Self, Failure> {
        let Ok(root) = root();
        let here = console_core_temporary_directories::fresh("live")?;
        let ran_at = here.join("ran");

        console_core_atomic_writes::whole(&ran_at, b"")?;
        let seen = load_capture()?;
        let world = load_capture()?;
        let uinput = Uinput::of(&seen)?;
        let Ok(devices) = Devices::new(world, uinput);
        let Ok(paths) = devices.paths();
        let profiles = every_profile(&root)?;
        let go = LegionGo::new(profiles, devices, TestClock::default(), console_input_gamepad::router::NAME)?;
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
        let mut process = alongside(
            Command::new(env!("CARGO_BIN_EXE_controller-desktop"))
                .env("PATH", format!("{}:{path}", instead.display()))
                .env("CONSOLE_RAN", &ran_at)
                .env("XDG_RUNTIME_DIR", &here)
                .env("HYPRLAND_INSTANCE_SIGNATURE", INSTANCE)
                .env("CONSOLE_PAD", pad)
                .env("CONSOLE_KEYS", keys)
                .env("CONSOLE_TOUCHPAD", touchpad)
                .stderr(Stdio::piped()),
        )?;
        let said = Arc::new(Mutex::new(String::new()));
        let heard = Arc::clone(&said);
        let Ok(voice) = process.take_stderr();
        let voice = voice.ok_or("the daemon's stderr")?;
        let Ok(()) = console_program_lifetime::threads::let_go(std::thread::spawn(move || {
            for line in BufReader::new(voice).lines().map_while(Result::ok) {
                match heard.lock() {
                    Ok(mut said) => said.push_str(&format!("{line}\n")),
                    Err(poisoned) => poisoned.into_inner().push_str(&format!("{line}\n")),
                }
            }
        }));
        let Ok(out) = wait_for(PUBLISHED, &was);

        match &out {
            Some(published) => published.grab()?,
            None => {},
        }

        let running = Running { go, out, said, process: Some(process), ran_at, asked, here };
        let Ok(_) = running.reading();

        Ok(running)
    }

    fn reading(&self) -> Result<Outcome, Never> {
        let Ok(patience) = Schedule::of(PATIENCE);

        until(patience, || {
            let Ok(said) = self.said();

            Ok(match READS.iter().all(|name| said.contains(&format!("reading the {name}"))) {
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        })
    }

    pub fn ran(&self) -> Result<Vec<Vec<String>>, Failure> {
        let Ok(patience) = Schedule::of(PATIENCE);
        let Ok(_) = until(patience, || {
            let Ok(asked) = held(&self.asked);

            let recorded = match std::fs::metadata(&self.ran_at) {
                Ok(file) => file.len(),
                Err(_not_written_yet) => 0,
            };

            Ok(match (recorded, asked.is_empty()) {
                (0, true) => Ready::NotYet,
                (_, _) => Ready::Yes,
            })
        });

        self.commands()
    }

    pub fn events(&mut self, seconds: f64) -> Result<Vec<InputEvent>, Failure> {
        let out = match self.out.as_mut() {
            Some(out) => out,
            None => return Ok(Vec::new()),
        };

        out.nonblocking()?;
        let mut every: Vec<InputEvent> = Vec::new();
        let Ok(window) = Schedule::asking_every(Duration::from_secs_f64(seconds), Duration::from_millis(10));
        let Ok(_) = until_handed(window, &mut (out, &mut every), |(out, every)| {
            match out.read_events() {
                Ok(arrived) => every.extend(arrived.into_iter().filter(|event| event.kind != EventType::SYNCHRONIZATION)),
                Err(_nothing_yet) => {},
            }

            Ok(Ready::NotYet)
        });

        Ok(every)
    }

    pub fn total(&mut self, (kind, code): (EventType, u16), seconds: f64) -> Result<i32, Failure> {
        let events = self.events(seconds)?;

        Ok(events
            .iter()
            .filter(|event| event.kind == kind && event.code == code)
            .fold(0, |sum, event| sum.saturating_add(event.value)))
    }

    pub fn commands(&self) -> Result<Vec<Vec<String>>, Failure> {
        let recorded = std::fs::read_to_string(&self.ran_at)?;
        let Ok(asked) = held(&self.asked);

        Ok(recorded
            .lines()
            .map(str::to_string)
            .chain(asked)
            .filter(|line| !line.is_empty())
            .map(|line| line.split('\t').map(str::to_string).collect())
            .collect())
    }

    pub fn names(&self) -> Result<Vec<String>, Failure> {
        let ran = self.ran()?;

        Ok(ran.into_iter().filter_map(|arguments| arguments.into_iter().next()).collect())
    }

    pub fn said(&self) -> Result<String, Never> {
        Ok(match self.said.lock() {
            Ok(said) => said.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        })
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        match self.out.as_mut() {
            Some(out) => {
                let _ = out.ungrab();
            },
            None => {},
        }

        drop(self.process.take());
        let Ok(()) = self.go.close();
        let _ = std::fs::remove_dir_all(&self.here);
    }
}

pub fn or_skip() -> Result<Option<Running>, Failure> {
    match std::fs::File::open("/dev/uinput") {
        Ok(_) => {},
        Err(_no_way_in) => {
            eprintln!("skipped: no way in to /dev/uinput; see docs/emulator.md");

            return Ok(None);
        },
    }

    let running = Running::new()?;
    let Ok(said) = running.said();

    assert!(running.out.is_some(), "the daemon never published a device: {said}");

    Ok(Some(running))
}

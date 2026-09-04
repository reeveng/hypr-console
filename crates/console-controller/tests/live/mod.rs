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
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use evdev::{Device, EventType, InputEvent};
use console_pad::capture::captured;
use console_pad::devices::Devices;
use console_pad::go::{Held, LegionGo};
use console_pad::router::every_profile;
use console_pad::uinput::Uinput;

/// The three of the four the daemon reads, by what it calls them. The mouse is
/// one it writes through, and it never opens it.
pub const READS: [&str; 3] = ["keyboard", "pad", "touchpad"];

/// What the daemon calls the device it publishes.
const PUBLISHED: &str = "stick-scroll";

/// Everything the daemon may try to run, which stands in for the desktop.
const INSTEAD: [&str; 8] = [
    "controller-profile",
    "game-mode",
    "hyprctl",
    "launcher",
    "console-brightness",
    "console-buttons",
    "console-screenshot",
    "settings-panel",
];

/// Stands in for whatever the daemon meant to run, and writes down that it was
/// asked. Every name in the directory is a link to this file, so the name it was
/// called by is the first thing written.
const RECORDER: &str = "#!/bin/sh\n\
    {\n\
    \x20   printf '%s' \"${0##*/}\"\n\
    \x20   for argument in \"$@\"; do printf '\\t%s' \"$argument\"; done\n\
    \x20   printf '\\n'\n\
    } >> \"$CONSOLE_RAN\"\n";

/// The repository, which is where the profiles are.
fn root() -> PathBuf {
    {
    // Tidied by `canonicalize` where that works and left as it stands where it
    // does not. What `CARGO_MANIFEST_DIR` gives is already absolute and already
    // right; canonicalizing only takes the `../..` out of the middle. It fails
    // under a sandbox that will not let a process resolve a path it can
    // otherwise read, and a test that stops there reports the sandbox as a
    // missing repository.
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

/// Whether this user can make a device at all.
pub fn uinput_is_open() -> bool {
    std::fs::OpenOptions::new().write(true).open("/dev/uinput").is_ok()
}

fn every_device() -> BTreeSet<PathBuf> {
    evdev::enumerate().map(|(path, _)| path).collect()
}

/// The device a daemon just made, by the name it gave it.
fn wait_for(name: &str, since: &BTreeSet<PathBuf>) -> Option<Device> {
    let by = Instant::now() + Duration::from_secs(5);
    while Instant::now() < by {
        let found = evdev::enumerate()
            .filter(|(path, _)| !since.contains(path))
            .find(|(_, device)| device.name() == Some(name));
        if let Some((_, device)) = found {
            return Some(device);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

/// A directory of programs that do nothing but say they were asked.
fn instead_of_the_desktop(here: &Path) -> PathBuf {
    let bin = here.join("bin");
    std::fs::create_dir_all(&bin).expect("somewhere to put them");
    let recorder = bin.join("recorder");
    std::fs::write(&recorder, RECORDER).expect("the recorder");
    std::fs::set_permissions(&recorder, std::fs::Permissions::from_mode(0o755))
        .expect("something runnable");
    for name in INSTEAD {
        let _ = std::os::unix::fs::symlink(&recorder, bin.join(name));
    }
    bin
}

/// A daemon, the devices it reads, and the device it writes.
///
/// A daemon publishes what it writes before it has opened what it reads, so
/// waiting for its device is not waiting for it to be ready. It says which
/// devices it found as it finds them, and that is what is waited for here: a
/// press sent in between would go to a device nobody was reading yet.
pub struct Running {
    pub go: LegionGo<Uinput, Held>,
    pub out: Option<Device>,
    said: Arc<Mutex<String>>,
    process: Child,
    ran_at: PathBuf,
    here: PathBuf,
}

impl Running {
    pub fn new() -> Result<Self, String> {
        let root = root();
        let here = std::env::temp_dir().join(format!("console-live-{}", std::process::id()));
        std::fs::create_dir_all(&here).map_err(|fault| fault.to_string())?;
        let ran_at = here.join("ran");
        std::fs::File::create(&ran_at).map_err(|fault| fault.to_string())?;

        let devices = Devices::new(captured().expect("the capture carried in this program parses"), Uinput::of(&captured().expect("the capture carried in this program parses"))?);
        let paths = devices.paths();
        let go = LegionGo::new(every_profile(&root)?, devices, Held::default(), console_pad::router::NAME)?;

        let was = every_device();
        let path = std::env::var("PATH").unwrap_or_default();
        let mut process = Command::new(env!("CARGO_BIN_EXE_stick-scroll"))
            .env("PATH", format!("{}:{path}", instead_of_the_desktop(&here).display()))
            .env("CONSOLE_RAN", &ran_at)
            .env("CONSOLE_PAD", paths.get("pad").cloned().unwrap_or_default())
            .env("CONSOLE_KEYS", paths.get("keyboard").cloned().unwrap_or_default())
            .env("CONSOLE_TOUCHPAD", paths.get("touchpad").cloned().unwrap_or_default())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|fault| fault.to_string())?;

        let said = Arc::new(Mutex::new(String::new()));
        let heard = Arc::clone(&said);
        let voice = process.stderr.take().expect("its voice");
        std::thread::spawn(move || {
            for line in BufReader::new(voice).lines().map_while(Result::ok) {
                heard.lock().expect("what it said").push_str(&format!("{line}\n"));
            }
        });

        let mut out = wait_for(PUBLISHED, &was);
        if let Some(published) = out.as_mut() {
            let _ = published.grab();
        }
        let mut running = Running { go, out, said, process, ran_at, here };
        running.reading();
        Ok(running)
    }

    /// Wait until it has said it found every device it reads.
    fn reading(&mut self) -> bool {
        let by = Instant::now() + Duration::from_secs(5);
        while Instant::now() < by {
            if READS.iter().all(|name| self.said().contains(&format!("reading the {name}"))) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    /// Let the daemon get round to what it was sent.
    pub fn settle(&self) {
        std::thread::sleep(Duration::from_millis(250));
    }

    /// Everything the daemon wrote, read off its own device.
    pub fn events(&mut self, seconds: f64) -> Vec<InputEvent> {
        let Some(out) = self.out.as_mut() else { return Vec::new() };
        let by = Instant::now() + Duration::from_secs_f64(seconds);
        let mut every = Vec::new();
        let _ = out.set_nonblocking(true);
        while Instant::now() < by {
            if let Ok(arrived) = out.fetch_events() {
                every.extend(arrived.filter(|event| event.event_type() != EventType::SYNCHRONIZATION));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        every
    }

    pub fn total(&mut self, kind: EventType, code: u16, seconds: f64) -> i32 {
        self.events(seconds)
            .iter()
            .filter(|event| event.event_type() == kind && event.code() == code)
            .map(|event| event.value())
            .sum()
    }

    /// Every program the daemon started, as the name and its arguments.
    pub fn commands(&self) -> Vec<Vec<String>> {
        std::fs::read_to_string(&self.ran_at)
            .unwrap_or_default()
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| line.split('\t').map(str::to_string).collect())
            .collect()
    }

    pub fn names(&self) -> Vec<String> {
        self.commands().into_iter().filter_map(|argv| argv.into_iter().next()).collect()
    }

    /// What the daemon has printed about itself so far.
    pub fn said(&self) -> String {
        self.said.lock().expect("what it said").clone()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(out) = self.out.as_mut() {
            let _ = out.ungrab();
        }
        let _ = self.process.kill();
        let _ = self.process.wait();
        self.go.close();
        let _ = std::fs::remove_dir_all(&self.here);
    }
}

/// The one thing a fast tier cannot ask: whether there is a kernel here to ask
/// it of. Skipped rather than failed, because everything these prove about what
/// the daemon decides is proved in the fast tier too.
pub fn or_skip() -> Option<Running> {
    if !uinput_is_open() {
        eprintln!("skipped: no way in to /dev/uinput; see docs/emulator.md");
        return None;
    }
    let running = Running::new().expect("a daemon");
    assert!(running.out.is_some(), "the daemon never published a device: {}", running.said());
    Some(running)
}

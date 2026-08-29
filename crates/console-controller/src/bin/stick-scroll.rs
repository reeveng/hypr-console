//! The desktop half of the controller: scrolling, and the buttons that ask the
//! compositor for something.
//!
//! Everything that decides anything is in `console_controller`, where it can be
//! asked the same question twice. What is here is a machine's real devices,
//! offered to that as somewhere the devices are plugged in.

use std::collections::BTreeMap;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{
    AbsoluteAxisCode, AttributeSet, Device, InputEvent, KeyCode, RelativeAxisCode,
};
use console_controller::doing::Doing;
use console_controller::finding::Says;
use console_controller::reading::{From, Ranges};
use console_controller::turning::{Gone, Plugged, READ, Turning};

fn main() -> std::process::ExitCode {
    let mut out = match published() {
        Ok(out) => out,
        Err(fault) => {
            eprintln!("stick-scroll: {fault}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let mut machine = Machine::default();
    let mut turning = Turning::pointed_at(told());
    let started = Instant::now();
    let mut holding: BTreeMap<From, String> = BTreeMap::new();
    let mut running: Vec<Child> = Vec::new();
    loop {
        for what in turning.turn(&mut machine, started.elapsed().as_secs_f64()) {
            running.extend(done(&what, &mut out));
        }
        running = reaped(running);
        say_what_changed(&mut holding, &turning);
        std::thread::sleep(Duration::from_secs_f64(turning.poll()));
    }
}

/// This machine's own devices.
#[derive(Default)]
struct Machine {
    open: BTreeMap<String, Device>,
}

impl Plugged for Machine {
    fn every(&self) -> Vec<Says> {
        evdev::enumerate().map(|(path, device)| says(&path.display().to_string(), &device)).collect()
    }

    fn open(&mut self, path: &str) -> bool {
        if self.open.contains_key(path) {
            return true;
        }
        let opened = Device::open(path).and_then(|device| {
            device.set_nonblocking(true)?;
            Ok(device)
        });
        match opened {
            Ok(device) => {
                self.open.insert(path.to_string(), device);
                true
            }
            Err(fault) => {
                eprintln!("stick-scroll: {path}: {fault}");
                false
            }
        }
    }

    /// The pad's ranges, read off it once when it is found.
    ///
    /// Asked for here and not per event, because the pad goes away whenever a
    /// profile is switched and arithmetic that has to ask a device that is not
    /// there is arithmetic that stops.
    fn ranges(&self, path: &str) -> Ranges {
        let Some(device) = self.open.get(path) else { return Ranges::default() };
        let mut told: BTreeMap<u16, (i32, i32)> = BTreeMap::new();
        if let Ok(states) = device.get_absinfo() {
            for (axis, info) in states {
                told.insert(axis.0, (info.minimum(), info.maximum()));
            }
        }
        let stick = [AbsoluteAxisCode::ABS_RX, AbsoluteAxisCode::ABS_RY]
            .iter()
            .filter_map(|axis| told.get(&axis.0))
            .map(|(low, high)| low.abs().max(high.abs()))
            .max()
            .filter(|span| *span > 0)
            .unwrap_or(Ranges::default().stick);
        let trigger =
            told.get(&AbsoluteAxisCode::ABS_Z.0).copied().unwrap_or(Ranges::default().trigger);
        Ranges { stick, trigger }
    }

    fn drain(&mut self, path: &str) -> Result<Vec<InputEvent>, Gone> {
        let Some(device) = self.open.get_mut(path) else { return Err(Gone) };
        let arrived = match device.fetch_events() {
            Ok(arrived) => Ok(arrived.collect()),
            Err(fault) if fault.kind() == std::io::ErrorKind::WouldBlock => Ok(Vec::new()),
            Err(_) => Err(Gone),
        };
        if arrived.is_err() {
            self.open.remove(path);
        }
        arrived
    }
}

/// What one device says about itself, in the words the rules are written in.
fn says(path: &str, device: &Device) -> Says {
    Says {
        path: path.to_string(),
        name: device.name().unwrap_or_default().to_string(),
        phys: device.physical_path().unwrap_or_default().to_string(),
        keys: device
            .supported_keys()
            .map(|keys| keys.iter().map(|key| key.0).collect())
            .unwrap_or_default(),
        axes: device
            .supported_absolute_axes()
            .map(|axes| axes.iter().map(|axis| axis.0).collect())
            .unwrap_or_default(),
    }
}

/// A device this was pointed at rather than left to find.
fn told() -> BTreeMap<From, String> {
    let named = |which| match which {
        From::Pad => "CONSOLE_PAD",
        From::Keys => "CONSOLE_KEYS",
        From::Touch => "CONSOLE_TOUCHPAD",
    };
    READ.into_iter()
        .filter_map(|which| {
            let path = std::env::var(named(which)).ok()?;
            (!path.is_empty()).then_some((which, path))
        })
        .collect()
}

/// A device found or lost, said once each time it happens.
///
/// The journal is where a pad that never came back is diagnosed, and a line
/// per turn would bury it.
fn say_what_changed(holding: &mut BTreeMap<From, String>, turning: &Turning) {
    let now = turning.holding();
    let name = |which| match which {
        From::Pad => "pad",
        From::Keys => "keyboard",
        From::Touch => "touchpad",
    };
    for which in READ {
        match (holding.get(&which), now.get(&which)) {
            (None, Some(path)) => eprintln!("stick-scroll: reading the {} at {path}", name(which)),
            (Some(_), None) => eprintln!("stick-scroll: the {} has gone", name(which)),
            _ => (),
        }
    }
    *holding = now.clone();
}

/// The device this daemon publishes: a wheel, a pointer and one button.
fn published() -> Result<VirtualDevice, String> {
    let mut keys = AttributeSet::<KeyCode>::new();
    keys.insert(KeyCode::BTN_LEFT);
    let mut axes = AttributeSet::<RelativeAxisCode>::new();
    for axis in [
        RelativeAxisCode::REL_HWHEEL,
        RelativeAxisCode::REL_WHEEL,
        RelativeAxisCode::REL_X,
        RelativeAxisCode::REL_Y,
    ] {
        axes.insert(axis);
    }
    VirtualDevice::builder()
        .map_err(|fault| format!("no way in to /dev/uinput: {fault}"))?
        .name("stick-scroll")
        .with_keys(&keys)
        .map_err(|fault| format!("the button: {fault}"))?
        .with_relative_axes(&axes)
        .map_err(|fault| format!("the wheel: {fault}"))?
        .build()
        .map_err(|fault| format!("the device would not build: {fault}"))
}

/// One decision, done. What it started, if it started anything.
fn done(what: &Doing, out: &mut VirtualDevice) -> Option<Child> {
    match what {
        Doing::Frame(frame) => {
            let events: Vec<InputEvent> = frame
                .iter()
                .map(|written| InputEvent::new(written.kind.0, written.code, written.value))
                .collect();
            if let Err(fault) = out.emit(&events) {
                eprintln!("stick-scroll: nothing came out: {fault}");
            }
            None
        }
        Doing::Run(argv) => {
            eprintln!("stick-scroll: {}", argv.join(" "));
            run(argv)
        }
    }
}

/// The ones that have ended, forgotten.
///
/// A child nobody asks after stays in the table as a zombie, and the daemon
/// starts one every time a button is pressed. Asking is all it takes.
fn reaped(running: Vec<Child>) -> Vec<Child> {
    running
        .into_iter()
        .filter_map(|mut child| match child.try_wait() {
            Ok(None) => Some(child),
            _ => None,
        })
        .collect()
}

/// Start something.
///
/// Whatever this starts is in this service's control group and stays there: a
/// control group is inherited by every child, and nothing a program can do to
/// itself leaves one. So a signal sent to the unit reaches all of it, which is
/// why `osk-hook` names --kill-whom=main when it stops this daemon to raise
/// the keyboard.
///
/// It keeps what they say on the way out. This daemon's stderr is the journal,
/// and a chooser that refuses to open is otherwise a button reported as broken
/// against a journal showing the press arriving and the chooser starting.
fn run(argv: &[String]) -> Option<Child> {
    let (program, rest) = argv.split_first()?;
    match Command::new(program).args(rest).stdout(Stdio::null()).stderr(Stdio::inherit()).spawn() {
        Ok(child) => Some(child),
        Err(fault) => {
            eprintln!("stick-scroll: cannot run {program}: {fault}");
            None
        }
    }
}

//! Legion left, held, while Game Mode has the screen.
//!
//! Everything else on the front of the machine belongs to Steam for as long as
//! it is up, and so does this button: the press reaches it untouched. What is
//! watched for here is the hold, which is how somebody comes back to the
//! desktop with the button they left on.
//!
//! It is a program of its own because the desktop's own daemon is not there to
//! do it. Game Mode stops `console.target` behind it, and what is left running
//! is this, started by the Game Mode session and stopped with it.
//!
//! Everything that decides anything is in `console_controller::returning`,
//! where it can be asked the same question twice. What is here is a machine's
//! real pad, and the pad going away and coming back, which a profile switch
//! does every time.

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use evdev::{Device, InputEvent};
use console_controller::clock::since_boot;
use console_controller::doing::Doing;
use console_controller::finding::{self, Says};
use console_controller::reading::POLL;
use console_controller::returning::Returning;
use console_controller::turning::{AWAY_SECONDS, HUNT_SECONDS};

fn main() -> std::process::ExitCode {
    let mut returning = Returning::default();
    let mut pad: Option<(String, Device)> = None;
    let mut hunted: Option<f64> = None;
    let mut running: Vec<Child> = Vec::new();
    let mut last: Option<f64> = None;
    loop {
        // The clock that counts a suspend, as the desktop's own daemon uses.
        let now = since_boot();
        // A gap means the machine was not running, and a button that was down
        // when it stopped is not a button somebody is holding now. Left to
        // stand, the hold is however long the machine slept and coming back is
        // the first thing it does on waking.
        if last.is_some_and(|was| now - was > AWAY_SECONDS) {
            returning.gone();
        }
        last = Some(now);
        if pad.is_none() && hunted.is_none_or(|was| now - was >= HUNT_SECONDS) {
            hunted = Some(now);
            pad = found();
            if let Some((path, _)) = &pad {
                eprintln!("game-return: reading the pad at {path}");
            }
        }
        if let Some((path, device)) = pad.as_mut() {
            match drain(device) {
                Ok(arrived) => {
                    for event in arrived {
                        returning.saw(event.event_type(), event.code(), event.value(), now);
                    }
                }
                Err(Gone) => {
                    eprintln!("game-return: the pad at {path} has gone");
                    pad = None;
                    returning.gone();
                }
            }
        }
        if let Some(Doing::Run(argv)) = returning.turn(now) {
            eprintln!("game-return: {}", argv.join(" "));
            running.extend(run(&argv));
        }
        running = reaped(running);
        std::thread::sleep(Duration::from_secs_f64(POLL));
    }
}

/// A device that is no longer there.
struct Gone;

/// The pad InputPlumber publishes, or the one this was pointed at.
///
/// Pointed at rather than found is how a test hands it a device it made, on a
/// machine whose own pad answers to the same description.
fn found() -> Option<(String, Device)> {
    let path = match std::env::var("CONSOLE_PAD") {
        Ok(told) if !told.is_empty() => told,
        _ => {
            let every: Vec<Says> = evdev::enumerate()
                .map(|(path, device)| says(&path.display().to_string(), &device))
                .collect();
            finding::gamepad(&every)?.path.clone()
        }
    };
    let opened = Device::open(&path).and_then(|device| {
        device.set_nonblocking(true)?;
        Ok(device)
    });
    match opened {
        Ok(device) => Some((path, device)),
        Err(fault) => {
            eprintln!("game-return: {path}: {fault}");
            None
        }
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

/// Everything waiting on the pad, or word that it has gone.
fn drain(device: &mut Device) -> Result<Vec<InputEvent>, Gone> {
    match device.fetch_events() {
        Ok(arrived) => Ok(arrived.collect()),
        Err(fault) if fault.kind() == std::io::ErrorKind::WouldBlock => Ok(Vec::new()),
        Err(_) => Err(Gone),
    }
}

/// Start something, keeping what it says on the way out: this program's stderr
/// is the journal, and a way back that refused to work is otherwise a button
/// reported as broken against a journal showing the hold arriving.
fn run(argv: &[String]) -> Option<Child> {
    let (program, rest) = argv.split_first()?;
    match Command::new(program).args(rest).stdout(Stdio::null()).stderr(Stdio::inherit()).spawn() {
        Ok(child) => Some(child),
        Err(fault) => {
            eprintln!("game-return: {program} did not start: {fault}");
            None
        }
    }
}

/// The ones that have ended, forgotten. A child nobody asks after stays in the
/// table as a zombie.
fn reaped(running: Vec<Child>) -> Vec<Child> {
    running
        .into_iter()
        .filter_map(|mut child| match child.try_wait() {
            Ok(None) => Some(child),
            _ => None,
        })
        .collect()
}

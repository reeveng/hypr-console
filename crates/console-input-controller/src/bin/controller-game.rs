//! Legion left, held, while Game Mode has the screen.  Everything it decides is
//! in `console_input_controller::returning`, where a second of holding a button
//! can be pressed in no time and a machine that went to sleep with a thumb on
//! it can be pressed at all. What is here is a real pad, and the pad going away
//! and coming back, which a profile switch does every time.  It is a program of
//! its own because the desktop's own daemon is not there to do it. Game Mode
//! stops `console.target` behind it, and what is left running is this, started
//! by the Game Mode session and stopped with it.  A missing pad is looked for
//! once a second and not more often, and finding one is said out loud where not
//! finding one is not: a profile switch destroys the pad and builds another, so
//! an empty moment is the ordinary state of things rather than a fault.

use std::process::ExitCode;
use std::time::Duration;

use console_input_event_devices::{Device, InputEvent};
use console_input_controller::finding::{self, DeviceInfo};
use console_input_controller::reading::From;
use console_input_controller::returning::{ReturningEvent, Return};
use console_input_controller::turning::HUNT_SECONDS;
use console_core_never::Never;
use console_program_contract::{Arguments, Timer, Elapsed, Event};
use console_program_runtime::Interpreter;

fn main() -> ExitCode {
    let Ok(code) = console_program_runtime::run::<Return, Pad>(
        "controller-game",
        &Arguments::default(),
        &mut Pad::default(),
    );

    code
}

#[derive(Default)]
struct Pad {
    held: Option<(String, Device)>,
    hunted: Option<Elapsed>,
}

impl Interpreter for Pad {
    type Event = ReturningEvent;
    type Effect = Never;

    fn interpret(&mut self, acts: &Never) -> Vec<Event<ReturningEvent>> {
        match *acts {}
    }

    fn tick(&mut self, _timer: &Timer, since: Elapsed) -> Result<Vec<Event<ReturningEvent>>, Never> {
        let Ok(()) = self.find(since);

        self.drained(since)
    }
}

fn again() -> Result<Duration, Never> {
    Ok(Duration::from_secs_f64(HUNT_SECONDS))
}

impl Pad {
    fn find(&mut self, since: Elapsed) -> Result<(), Never> {
        let Ok(again) = again();

        let looking = match &self.held {
            Some(_) => return Ok(()),
            None => self.hunted.is_none_or(|was| since.saturating_sub(was) >= again),
        };

        match looking {
            true => {},
            false => return Ok(()),
        }

        self.hunted = Some(since);

        let Ok(found) = found();

        self.held = found;

        match &self.held {
            Some((path, _)) => eprintln!("controller-game: reading the pad at {path}"),
            None => {},
        }

        Ok(())
    }

    fn drained(&mut self, since: Elapsed) -> Result<Vec<Event<ReturningEvent>>, Never> {
        let (path, device) = match self.held.as_mut() {
            Some((path, device)) => (path, device),
            None => return Ok(Vec::new()),
        };

        Ok(match drain(device) {
            Ok(arrived) => arrived
                .into_iter()
                .map(|event| {
                    Event::Custom(ReturningEvent::Saw {
                        kind: event.kind,
                        code: event.code,
                        value: event.value,
                        at: since,
                    })
                })
                .collect(),
            Err(Closed) => {
                eprintln!("controller-game: the pad at {path} has gone");
                self.held = None;

                vec![Event::Custom(ReturningEvent::Closed)]
            }
        })
    }
}

struct Closed;

fn found() -> Result<Option<(String, Device)>, Never> {
    let Ok(told) = From::Pad.told();

    let path = match told {
        Some(told) => told,
        None => {
            let Ok(devices) = Device::every();
            let every: Vec<DeviceInfo> = devices
                .iter()
                .map(|device| {
                    let Ok(info) = describe(&device.path.display().to_string(), device);

                    info
                })
                .collect();
            let Ok(gamepad) = finding::gamepad(&every);

            let found = match gamepad {
                Some(found) => found,
                None => return Ok(None),
            };

            found.path.clone()
        }
    };
    let opened = Device::open(std::path::Path::new(&path)).and_then(|device| {
        device.nonblocking()?;
        Ok(device)
    });

    Ok(match opened {
        Ok(device) => Some((path, device)),
        Err(fault) => {
            eprintln!("controller-game: {path}: {fault}");
            None
        }
    })
}

fn describe(path: &str, device: &Device) -> Result<DeviceInfo, Never> {
    console_input_gamepad::finding::describe(path, device)
}

fn drain(device: &mut Device) -> Result<Vec<InputEvent>, Closed> {
    match device.read_events() {
        Ok(arrived) => Ok(arrived),
        Err(fault) => match fault.kind() == std::io::ErrorKind::WouldBlock {
            true => Ok(Vec::new()),
            false => Err(Closed),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hunt_is_the_daemons_own_timer() {
        let Ok(again) = again();

        assert_eq!(again, Duration::from_secs(1));
        assert_eq!(again.as_secs_f64(), HUNT_SECONDS);
    }
}

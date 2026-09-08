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

use evdev::{Device, InputEvent};
use console_input_controller::finding::{self, Says};
use console_input_controller::returning::{Heard, Return};
use console_input_controller::turning::HUNT_SECONDS;
use console_core_never::Never;
use console_program_contract::{Argv, Round, Since, Word};
use console_program_runtime::Carrying;

fn main() -> ExitCode {
    let Ok(code) = console_program_runtime::run::<Return, Pad>(
        "game-return",
        &Argv::default(),
        &mut Pad::default(),
    );

    code
}

#[derive(Default)]
struct Pad {
    held: Option<(String, Device)>,
    hunted: Option<Since>,
}

impl Carrying for Pad {
    type Hears = Heard;
    type Does = Never;

    fn its(&mut self, doing: &Never) -> Vec<Word<Heard>> {
        match *doing {}
    }

    fn came(&mut self, _round: &Round, since: Since) -> Result<Vec<Word<Heard>>, Never> {
        let Ok(()) = self.find(since);

        self.drained(since)
    }
}

fn again() -> Result<Duration, Never> {
    Ok(Duration::from_secs_f64(HUNT_SECONDS))
}

impl Pad {
    fn find(&mut self, since: Since) -> Result<(), Never> {
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
            Some((path, _)) => eprintln!("game-return: reading the pad at {path}"),
            None => {},
        }

        Ok(())
    }

    fn drained(&mut self, since: Since) -> Result<Vec<Word<Heard>>, Never> {
        let (path, device) = match self.held.as_mut() {
            Some((path, device)) => (path, device),
            None => return Ok(Vec::new()),
        };

        Ok(match drain(device) {
            Ok(arrived) => arrived
                .into_iter()
                .map(|event| {
                    Word::Its(Heard::Saw {
                        kind: event.event_type(),
                        code: event.code(),
                        value: event.value(),
                        at: since,
                    })
                })
                .collect(),
            Err(Gone) => {
                eprintln!("game-return: the pad at {path} has gone");
                self.held = None;

                vec![Word::Its(Heard::Gone)]
            }
        })
    }
}

struct Gone;

fn found() -> Result<Option<(String, Device)>, Never> {
    let path = match std::env::var("CONSOLE_PAD") {
        Ok(told) if !told.is_empty() => told,
        Ok(_) | Err(_) => {
            let every: Vec<Says> = evdev::enumerate()
                .map(|(path, device)| {
                    let Ok(says) = says(&path.display().to_string(), &device);

                    says
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
    let opened = Device::open(&path).and_then(|device| {
        device.set_nonblocking(true)?;
        Ok(device)
    });

    Ok(match opened {
        Ok(device) => Some((path, device)),
        Err(fault) => {
            eprintln!("game-return: {path}: {fault}");
            None
        }
    })
}

fn says(path: &str, device: &Device) -> Result<Says, Never> {
    Ok(Says {
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
    })
}

fn drain(device: &mut Device) -> Result<Vec<InputEvent>, Gone> {
    match device.fetch_events() {
        Ok(arrived) => Ok(arrived.collect()),
        Err(fault) if fault.kind() == std::io::ErrorKind::WouldBlock => Ok(Vec::new()),
        Err(_) => Err(Gone),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hunt_is_the_daemons_own_stretch() {
        let Ok(again) = again();

        assert_eq!(again, Duration::from_secs(1));
        assert_eq!(again.as_secs_f64(), HUNT_SECONDS);
    }
}

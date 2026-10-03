//! The half of recovery that touches the machine: every event node, the text
//! console, and systemd's own socket to say the menu is up.
//!
//! The pad is `console_input_event_devices`'s, which watches for one plugged in after
//! this started. It is waited on by a thread of its own that hands every press
//! to the runtime's loop, which is the only loop here: no timer, because
//! nothing here happens unless somebody presses something.
//!
//! Ready is said once the menu is on the screen, which is the only claim this
//! rung makes. A menu that could not be written is not said to be up, so a
//! unit that never hears it gives up on it, and the next rung down is
//! whatever its `OnFailure=` names.

use std::io::{self, Write};
use std::process::ExitCode;

use console_core_iteration::Step;
use console_core_never::Never;
use console_program_contract::{Arguments, Event, Exit};
use console_program_runtime::{Delivery, Interpreter, Subscribed, Tell, run};
use console_program_lifetime::threads;
use console_input_event_devices::device::INPUT;
use console_input_event_devices::devices::Devices;
use console_input_event_devices::presses::ButtonPress;
use console_recovery::menu::{Menu, Recovery, Screen, screen};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Readiness {
    Ready,
    NotYet,
}

struct Console {
    devices: Option<Devices>,
    said: Readiness,
}

impl Interpreter for Console {
    type Event = ButtonPress;
    type Effect = Screen;

    fn interpret(&mut self, effect: &Screen) -> Vec<Event<ButtonPress>> {
        match effect {
            Screen::Show(menu) => {
                let Ok(()) = self.show(menu);
            }
        }

        Vec::new()
    }

    fn listen(&mut self, tell: Tell<ButtonPress>) -> Result<Subscribed, Never> {
        let devices = match self.devices.take() {
            Some(devices) => devices,
            None => return Ok(Subscribed::No),
        };

        let Ok(()) = threads::let_go(std::thread::spawn(move || {
            let _ended = console_core_iteration::iterate(devices, |mut devices| {
                let woke = match devices.wait(None, None) {
                    Ok(woke) => woke,
                    Err(why) => {
                        let _ = tell.end(Exit::Failure(format!("cannot wait for a press: {why}")));

                        return Ok(Step::Halt(()));
                    }
                };

                let gone = woke.presses.into_iter().find_map(|press| {
                    let Ok(heard) = tell.tell(press);

                    match heard {
                        Delivery::Yes => None,
                        Delivery::Gone => Some(Delivery::Gone),
                    }
                });

                Ok(match gone {
                    Some(_gone) => Step::Halt(()),
                    None => Step::Again(devices),
                })
            });
        }));

        Ok(Subscribed::Yes)
    }
}

impl Console {
    fn show(&mut self, menu: &Menu) -> Result<(), Never> {
        match draw(menu) {
            Ok(()) => {},
            Err(why) => {
                eprintln!("recovery: cannot write to the console: {why}");

                return Ok(());
            }
        }

        match self.said {
            Readiness::Ready => {},
            Readiness::NotYet => match console_readiness::ready() {
                Ok(()) => self.said = Readiness::Ready,
                Err(why) => eprintln!("recovery: cannot tell systemd the menu is up: {why}"),
            },
        }

        Ok(())
    }
}

fn main() -> ExitCode {
    let devices = match Devices::open() {
        Ok(devices) => devices,
        Err(why) => {
            eprintln!("recovery: cannot watch {INPUT}: {why}");

            return ExitCode::FAILURE;
        }
    };

    let Ok(arguments) = Arguments::of(&[]);
    let mut console = Console { devices: Some(devices), said: Readiness::NotYet };
    let Ok(ended) = run::<Recovery, Console>("recovery", &arguments, &mut console);

    ended
}

fn draw(menu: &Menu) -> io::Result<()> {
    let Ok(shown) = screen(menu);
    let mut console = io::stdout().lock();

    console.write_all(shown.as_bytes())?;
    console.flush()?;

    Ok(())
}

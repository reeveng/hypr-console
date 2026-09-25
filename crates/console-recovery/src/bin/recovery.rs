//! The half of recovery that touches the machine: every event node, the text
//! console, `systemctl`, and systemd's own socket to say the menu is up.
//!
//! The pad is `console_input_event_devices`'s, which watches for one plugged in after
//! this started. The loop waits on it and on nothing else: no timer, because
//! nothing here happens unless somebody presses something.
//!
//! Ready is said once the menu is on the screen, which is the only claim this
//! rung makes. A unit that never hears it gives up on it, and the next rung
//! down is whatever its `OnFailure=` names.

use std::collections::VecDeque;
use std::env;
use std::io::{self, Write};
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixDatagram};
use std::process::ExitCode;

use console_core_never::Never;
use console_program_contract::{
    Answer, Arguments, Command, Effect, Event, Executable, Exit, ExitStatus, Initial, Program, Update,
};
use console_input_event_devices::device::INPUT;
use console_input_event_devices::devices::Devices;
use console_input_event_devices::presses::ButtonPress;
use console_recovery::menu::{Menu, Recovery, screen};

const READY: &[u8] = b"READY=1";

enum Ended {
    Stopped(Exit),
    Error(String),
}

fn main() -> ExitCode {
    let Ok(ended) = recovered();

    match ended {
        Ended::Stopped(Exit::Success) => ExitCode::SUCCESS,
        Ended::Stopped(Exit::Failure(why)) | Ended::Error(why) => {
            eprintln!("recovery: {why}");

            ExitCode::FAILURE
        }
    }
}

fn recovered() -> Result<Ended, Never> {
    let Ok(arguments) = Arguments::of(&[]);
    let Initial { state, subscriptions: _ } = Recovery::init(&arguments);
    let mut menu = state;
    let mut devices = match Devices::watched() {
        Ok(devices) => devices,
        Err(why) => return Ok(Ended::Error(format!("cannot watch {INPUT}: {why}"))),
    };

    match drawn(&menu) {
        Ok(()) => {}
        Err(why) => return Ok(Ended::Error(format!("cannot write to the console: {why}"))),
    }

    match ready() {
        Ok(()) => {}
        Err(why) => return Ok(Ended::Error(format!("cannot tell systemd the menu is up: {why}"))),
    }

    loop {
        let woke = match devices.waited(None) {
            Ok(woke) => woke,
            Err(why) => return Ok(Ended::Error(format!("cannot wait for a press: {why}"))),
        };
        let mut queue: VecDeque<Event<ButtonPress>> = woke.presses.into_iter().map(Event::Custom).collect();

        while let Some(event) = queue.pop_front() {
            let Update { state: next, effects } = Recovery::update(&menu, &event);

            let changed = next != menu;
            menu = next;

            match changed {
                true => match drawn(&menu) {
                    Ok(()) => {}
                    Err(why) => return Ok(Ended::Error(format!("cannot write to the console: {why}"))),
                },
                false => {}
            }

            for effect in effects {
                let Ok(outcome) = outcome(effect);

                match outcome {
                    Outcome::Answer(answer) => queue.push_back(Event::Replied(answer)),
                    Outcome::Stop(exit) => return Ok(Ended::Stopped(exit)),
                    Outcome::None => {}
                }
            }
        }
    }
}

enum Outcome {
    Answer(Answer),
    Stop(Exit),
    None,
}

fn outcome(effect: Effect<Never>) -> Result<Outcome, Never> {
    Ok(match effect {
        Effect::Run(command) => {
            let Ok(answer) = ran(command);

            Outcome::Answer(answer)
        }
        Effect::Stop(exit) => Outcome::Stop(exit),
        Effect::Custom(never) => match never {},
        Effect::Stream(_)
        | Effect::Prompt(_)
        | Effect::Spawn(_)
        | Effect::Subscribe(_)
        | Effect::Unsubscribe(_)
        | Effect::Write(_)
        | Effect::Notify(_)
        | Effect::Print(_) => Outcome::None,
    })
}

fn ran(command: Command) -> Result<Answer, Never> {
    let program = match command.program {
        Executable::External(program) => program,
        Executable::Internal(_) => {
            return Ok(Answer { command, output: String::new(), status: ExitStatus::Failure(None) });
        }
    };
    let Ok(mut running) = program.command();
    let finished = running.args(&command.arguments).status();

    let status = match finished {
        Ok(finished) => match finished.success() {
            true => ExitStatus::Success,
            false => ExitStatus::Failure(finished.code()),
        },
        Err(why) => {
            eprintln!("recovery: {why}");

            ExitStatus::Failure(None)
        }
    };

    Ok(Answer { command, output: String::new(), status })
}

fn drawn(menu: &Menu) -> io::Result<()> {
    let Ok(shown) = screen(menu);
    let mut console = io::stdout().lock();

    console.write_all(shown.as_bytes())?;
    console.flush()?;

    Ok(())
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "NOTIFY_SOCKET is systemd's handed to this unit alone, read once at the one moment it is answered, and nothing else in the tree speaks the notify protocol"
    )
)]
fn ready() -> io::Result<()> {
    let socket = match env::var_os("NOTIFY_SOCKET") {
        Some(socket) => socket,
        None => return Ok(()),
    };
    let named = match socket.as_encoded_bytes().split_first() {
        Some((b'@', name)) => SocketAddr::from_abstract_name(name),
        Some(_) => SocketAddr::from_pathname(&socket),
        None => return Ok(()),
    };
    let address = named?;
    let made = UnixDatagram::unbound();
    let sender = made?;

    sender.send_to_addr(READY, &address)?;

    Ok(())
}

//! What wakes a reading up.
//!
//! Each of these has something that says when it changed, so nothing here polls
//! for the sake of it: the sound is told by pipewire, the network by
//! NetworkManager, and every one of them by the compositor when a panel opens
//! over it. The tick under them is the net, for a machine where one of those
//! is not running and for the battery, which nothing announces.
//!
//! Two of them are asked through `console-events` rather than opened here,
//! because two of them have a source in the pool: the compositor, and the
//! sound. `nmcli monitor` is still this program's own and moves over when the
//! pool learns to hold it -- and `busctl monitor` under `watching_notices` with
//! it, which is `Topic::Notices` waiting for the same thing.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use console_program_lifetime::alongside;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_program_contract::Topic;
use console_core_reconnect::{Round, keep};

use crate::reading::What;

pub const BELL: Duration = Duration::from_secs(10);

pub fn tick(what: What) -> Result<Duration, Never> {
    Ok(match what {
        What::Battery => Duration::from_secs(30),
        What::Bluetooth => Duration::from_secs(10),
        What::Network => Duration::from_secs(10),
        What::Sound => Duration::from_secs(10),
    })
}

enum Told {
    ThePool(Topic),
    Ours(Vec<&'static str>),
    Nothing,
}

fn teller(what: What) -> Result<Told, Never> {
    let Ok(nmcli) = Program::Nmcli.name();

    Ok(match what {
        What::Battery => Told::Nothing,
        What::Bluetooth => Told::Nothing,
        What::Network => Told::Ours(vec![nmcli, "monitor"]),
        What::Sound => Told::ThePool(Topic::Sound),
    })
}

pub fn watching(what: What) -> Result<Receiver<()>, Never> {
    let (say, heard) = channel();
    let Ok(()) = layers(say.clone());
    let Ok(telling) = teller(what);

    match telling {
        Told::ThePool(topic) => {
            let Ok(()) = console_events::again::about(&topic, say);
        }
        Told::Ours(argv) => {
            let Ok(()) = lines(argv, say);
        }
        Told::Nothing => {},
    }

    Ok(heard)
}

fn layers(say: Sender<()>) -> Result<(), Never> {
    console_events::again::layers(say)
}

pub fn watching_notices() -> Result<Receiver<()>, Never> {
    let (say, heard) = channel();
    let Ok(()) = layers(say.clone());
    let Ok(stdbuf) = Program::Stdbuf.name();
    let Ok(busctl) = Program::Busctl.name();
    let Ok(()) = lines(
        vec![
            stdbuf,
            "-oL",
            busctl,
            "--user",
            "monitor",
            "org.freedesktop.Notifications",
        ],
        say,
    );
    Ok(heard)
}

pub fn lines(argv: Vec<&'static str>, say: Sender<()>) -> Result<(), Never> {
    let Ok(()) = keep(move || {
        let Ok(round) = once(&argv, &say);

        round
    });

    Ok(())
}

fn once(argv: &[&'static str], say: &Sender<()>) -> Result<Round, Never> {
    let Some((program, rest)) = argv.split_first() else { return Ok(Round::Done) };

    let mut asking = Command::new(program);
    asking.args(rest).stdout(Stdio::piped()).stderr(Stdio::null());

    let Ok(mut running) = alongside(&mut asking) else {
        return Ok(Round::Another);
    };

    let Ok(Some(out)) = running.reading() else {
        return Ok(Round::Another);
    };

    for _ in BufReader::new(out).lines().map_while(Result::ok) {
        match say.send(()) {
            Ok(()) => {}
            Err(_gone) => return Ok(Round::Done),
        }
    }

    Ok(Round::Another)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_watcher_whose_program_ends_is_started_again() {
        let (say, heard) = channel();
        let Ok(echo) = Program::Echo.name();
        let Ok(()) = lines(vec![echo, "something happened"], say);
        for word in 1..=2 {
            heard
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| panic!("word {word} of 2"));
        }
    }

    #[test]
    fn a_watcher_nobody_is_listening_to_stops() {
        let (say, heard) = channel::<()>();
        drop(heard);
        let Ok(echo) = Program::Echo.name();

        assert_eq!(once(&[echo, "anything"], &say), Ok(Round::Done));
    }

    #[test]
    fn a_program_that_will_not_start_is_worth_another_try() {
        let (say, _heard) = channel::<()>();
        assert_eq!(once(&["console-nothing-is-called-this"], &say), Ok(Round::Another));
    }
}

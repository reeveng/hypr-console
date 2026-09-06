//! What wakes a reading up.
//!
//! Each of these has something that says when it changed, so nothing here polls
//! for the sake of it: the sound is told by pipewire, the network by
//! NetworkManager, and every one of them by the compositor when a panel opens
//! over it. The tick under them is the net, for a machine where one of those
//! is not running and for the battery, which nothing announces.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use console_child_processes::alongside;
use console_external_programs::Program;
use console_never::Never;
use console_panel::door::watching_layers;
use console_reconnect::{Round, keep};

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

fn teller(what: What) -> Result<Option<Vec<&'static str>>, Never> {
    let Ok(nmcli) = Program::Nmcli.name();
    let Ok(pactl) = Program::Pactl.name();

    Ok(match what {
        What::Battery => None,
        What::Bluetooth => None,
        What::Network => Some(vec![nmcli, "monitor"]),
        What::Sound => Some(vec![pactl, "subscribe"]),
    })
}

pub fn watching(what: What) -> Result<Receiver<()>, Never> {
    let (say, heard) = channel();
    let Ok(()) = layers(say.clone());
    let Ok(telling) = teller(what);

    match telling {
        Some(argv) => {
            let Ok(()) = lines(argv, say);
        },
        None => {}
    }

    Ok(heard)
}

fn layers(say: Sender<()>) -> Result<(), Never> {
    match watching_layers(say) {
        Ok(()) => {}
        Err(why) => eprintln!("bar: no compositor to watch for a panel over the bar: {why}"),
    }

    Ok(())
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

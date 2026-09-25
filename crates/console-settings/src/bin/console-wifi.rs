//! The machine, moved onto whichever of the Wi-Fi networks it knows is doing
//! better.
//!
//! What decides is `console_settings::wifi::better`; this is the part that
//! needs a machine. It wakes when NetworkManager says a scan finished, reads
//! the list that scan left behind rather than asking for another -- a scan of
//! its own would be heard as the next scan finishing, and it would never stop
//! -- asks the driver how fast the link in use is receiving, and asks
//! NetworkManager to join the network the decision named.
//!
//! Nothing here waits on a clock. On a weak link the supplicant scans every
//! half minute by itself and on a strong one every few minutes, which is how
//! soon a network going bad is left and how soon a router walked up to is
//! noticed.
//!
//! What it remembers is the networks it left for being slow and how strong
//! they were then, for as long as it runs. That is what keeps a five gigahertz
//! network that is loud and slow from being joined at every scan.

use std::process::{ExitCode, ExitStatus, Output};
use std::sync::mpsc::channel;

use console_core_external_programs::Program;
use console_events::again::{about, scanned};
use console_notifications::saying::journal;
use console_program_contract::Topic;
use console_settings::wifi::{self, DEVICES, IN_RANGE, KNOWN, Left, Receiving, Why};

fn main() -> ExitCode {
    let (say, heard) = channel();
    let Ok(()) = about(&Topic::Wifi, scanned, say);
    let mut left = Left::new();

    for () in heard.iter() {
        match look(&mut left) {
            Ok(()) => {},
            Err(fault) => eprintln!("{fault}"),
        }
    }

    eprintln!("the event pool stopped saying anything, so there is nothing left to wake for");

    ExitCode::FAILURE
}

#[derive(Debug)]
enum Unasked {
    Failed { asked: String, status: ExitStatus, said: String },
    NotFound(&'static str, std::io::Error),
}

impl std::fmt::Display for Unasked {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unasked::Failed { asked, status, said } => write!(to, "{asked} said {status}: {said}"),
            Unasked::NotFound(program, fault) => write!(to, "no {program} to ask: {fault}"),
        }
    }
}

impl std::error::Error for Unasked {}

fn look(left: &mut Left) -> Result<(), Unasked> {
    let in_range = asked(Program::Nmcli, &IN_RANGE)?;
    let known = asked(Program::Nmcli, &KNOWN)?;
    let devices = asked(Program::Nmcli, &DEVICES)?;

    let Ok(networks) = wifi::networks(&in_range);
    let Ok(saved) = wifi::saved(&known);
    let Ok(device) = wifi::wireless(&devices);

    let link = match device {
        Some(device) => {
            let said = asked(Program::Iw, &["dev", &device, "link"])?;
            let Ok(link) = wifi::receiving(&said);

            link
        }
        None => Receiving::Unknown,
    };

    let Ok(better) = wifi::better(&networks, &saved, link, left);

    let chosen = match better {
        Some(chosen) => chosen,
        None => return Ok(()),
    };

    let why = match chosen.why {
        Why::Weak => "it is weak",
        Why::Slow => "it is receiving at the slowest rate",
        Why::Near => "the faster band is strong here",
    };

    let Ok(()) = journal(&format!(
        "leaving {} at {}% for {} at {}%: {why}",
        chosen.from.name, chosen.from.signal, chosen.to.name, chosen.to.signal,
    ));

    match chosen.why {
        Why::Slow => {
            let _ = left.insert(chosen.from.name.clone(), chosen.from.signal);
        },
        Why::Weak | Why::Near => {},
    }

    let _ = asked(Program::Nmcli, &["connection", "up", "id", &chosen.to.name])?;

    Ok(())
}

fn asked(program: Program, words: &[&str]) -> Result<String, Unasked> {
    let Ok(mut asking) = program.command();
    let Ok(spelled) = program.arguments(words);

    match asking.args(words).output() {
        Ok(Output { status, stdout, stderr }) => match status.success() {
            true => Ok(String::from_utf8_lossy(&stdout).into_owned()),
            false => Err(Unasked::Failed {
                asked: spelled.join(" "),
                status,
                said: String::from_utf8_lossy(&stderr).trim().to_string(),
            }),
        },
        Err(fault) => {
            let Ok(name) = program.name();

            Err(Unasked::NotFound(name, fault))
        }
    }
}

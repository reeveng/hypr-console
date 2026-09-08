//! What wakes a reading up.
//!
//! Each of these has something that says when it changed, so nothing here polls
//! for the sake of it: the sound is told by pipewire, the network by
//! NetworkManager, the battery by udev when the kernel says a supply changed,
//! and every one of them by the compositor when a panel opens over it. The tick
//! under them is the net, for a machine where one of those is not running.
//!
//! The battery was the one nothing told, and a reading every thirty seconds is
//! worst at the one moment somebody is watching it: the cable has just gone in
//! and the bar is where they look to find out whether it went in. `udevadm
//! monitor` is on the whole `power_supply` subsystem rather than on a name,
//! because what changes when this device is plugged in is a USB-C supply on one
//! day and the adapter on another, and the two files the reading comes from are
//! the same either way. It needs no `stdbuf`: udevadm line-buffers its own
//! output, on the grounds that whoever asked to be told wants telling now.
//!
//! All but one are asked through `console-events` rather than opened here,
//! because the pool holds a source for each: the compositor, the sound, the
//! network and the bell. The battery is the one still opening its own, and it
//! is the one that should -- `udevadm monitor` is watched on a subsystem
//! rather than on a name, which is a question about this machine's supplies
//! rather than a thing another program on this desktop would ever ask.
//!
//! **A reading whose own asking is heard as a change never stops.** `pactl
//! subscribe` says a client appeared, changed, and went away; reading the
//! volume with `wpctl` *is* a client appearing, changing and going away. Handed
//! the line whole, this woke, asked, and was woken by its own asking: measured
//! on the device it stood at forty-seven readings a second with nobody
//! touching the machine, and the sound server answering those connections was
//! most of what the desktop did while it was idle. The bell had the same shape
//! and only its settle kept it off the same cliff, because `busctl monitor`
//! shows `makoctl` connecting to ask what is waiting.
//!
//! So no source's line is a reason to look until somebody says it is. Each
//! watch carries what its own reading comes from, decided on this side of the
//! socket the way `console-sky` decides what is worth waking for, and the two
//! that want every line say `anything` rather than say nothing. The tick
//! underneath is what makes this safe to get wrong in the careful direction: a
//! line nobody recognised costs one cadence, where a line nobody filtered costs
//! the machine.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use console_events::again::{Worth, Worthwhile, about, anything, layers};
use console_events::bus;
use console_events::sources::{MAKO, NOTICES};
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
    ThePool(Topic, Worthwhile),
    Ours(Vec<&'static str>, Worthwhile),
    Nothing,
}

fn teller(what: What) -> Result<Told, Never> {
    let Ok(udevadm) = Program::Udevadm.name();

    Ok(match what {
        What::Battery => Told::Ours(
            vec![udevadm, "monitor", "--udev", "--subsystem-match=power_supply"],
            anything,
        ),
        What::Bluetooth => Told::Nothing,
        What::Network => Told::ThePool(Topic::Network, anything),
        What::Sound => Told::ThePool(Topic::Sound, sound_worth_asking_after),
    })
}

pub fn watching(what: What) -> Result<Receiver<()>, Never> {
    let (say, heard) = channel();
    let Ok(()) = layers(say.clone());
    let Ok(telling) = teller(what);

    match telling {
        Told::ThePool(topic, worth) => {
            let Ok(()) = about(&topic, worth, say);
        }
        Told::Ours(argv, worth) => {
            let Ok(()) = lines(argv, worth, say);
        }
        Told::Nothing => {},
    }

    Ok(heard)
}

pub fn sound_worth_asking_after(line: &str) -> Result<Worth, Never> {
    let mut said = line.split_whitespace().skip_while(|word| *word != "on");

    Ok(match said.nth(1) {
        Some("sink" | "server") => Worth::Asking,
        Some(_not_what_the_reading_comes_from) => Worth::Ignoring,
        None => Worth::Asking,
    })
}

pub fn notice_worth_asking_after(line: &str) -> Result<Worth, Never> {
    let Ok(said) = bus::message(line);

    let said = match said {
        Some(said) => said,
        None => return Ok(Worth::Ignoring),
    };

    Ok(match (said.interface, said.member) {
        (MAKO, "ListNotifications" | "ListModes") => Worth::Ignoring,
        (MAKO | NOTICES, _something_happened) => Worth::Asking,
        (_somebody_elses_conversation, _member) => Worth::Ignoring,
    })
}

pub fn watching_notices() -> Result<Receiver<()>, Never> {
    let (say, heard) = channel();
    let Ok(()) = layers(say.clone());
    let Ok(()) = about(&Topic::Notices, notice_worth_asking_after, say);

    Ok(heard)
}

pub fn lines(
    argv: Vec<&'static str>,
    worth: Worthwhile,
    say: Sender<()>,
) -> Result<(), Never> {
    let Ok(()) = keep(move || {
        let Ok(round) = once(&argv, worth, &say);

        round
    });

    Ok(())
}

fn once(
    argv: &[&'static str],
    worth: Worthwhile,
    say: &Sender<()>,
) -> Result<Round, Never> {
    let (program, rest) = match argv.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(Round::Done),
    };

    let mut asking = Command::new(program);
    asking.args(rest).stdout(Stdio::piped()).stderr(Stdio::null());

    let mut running = match alongside(&mut asking) {
        Ok(running) => running,
        Err(_fault) => return Ok(Round::Another),
    };

    let out = match running.reading() {
        Ok(Some(out)) => out,
        Ok(None) | Err(_) => return Ok(Round::Another),
    };

    for line in BufReader::new(out).lines().map_while(Result::ok) {
        let Ok(asking) = worth(&line);

        match asking {
            Worth::Asking => {
                match say.send(()) {
                    Ok(()) => {}
                    Err(_gone) => return Ok(Round::Done),
                }
            }
            Worth::Ignoring => {},
        }
    }

    Ok(Round::Another)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_battery_is_told_when_a_supply_changes() {
        let Ok(told) = teller(What::Battery);
        let Ok(udevadm) = Program::Udevadm.name();

        match told {
            Told::Ours(argv, _worth) => assert_eq!(argv.first(), Some(&udevadm)),
            Told::ThePool(_, _) | Told::Nothing => {
                panic!("the cable going in waits for the tick again")
            }
        }
    }

    #[test]
    fn a_watcher_whose_program_ends_is_started_again() {
        let (say, heard) = channel();
        let Ok(echo) = Program::Echo.name();
        let Ok(()) = lines(vec![echo, "something happened"], anything, say);
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

        assert_eq!(once(&[echo, "anything"], anything, &say), Ok(Round::Done));
    }

    #[test]
    fn a_program_that_will_not_start_is_worth_another_try() {
        let (say, _heard) = channel::<()>();
        assert_eq!(
            once(&["console-nothing-is-called-this"], anything, &say),
            Ok(Round::Another)
        );
    }

    #[test]
    fn the_sound_is_read_again_when_a_sink_or_the_server_changed() {
        let Ok(sink) = sound_worth_asking_after("Event 'change' on sink #49");
        let Ok(server) = sound_worth_asking_after("Event 'change' on server");
        let Ok(gone) = sound_worth_asking_after("Event 'remove' on sink #49");

        assert_eq!(sink, Worth::Asking);
        assert_eq!(server, Worth::Asking);
        assert_eq!(gone, Worth::Asking);
    }

    #[test]
    fn asking_what_the_volume_is_does_not_ask_what_the_volume_is() {
        for line in [
            "Event 'new' on client #312779",
            "Event 'change' on client #312779",
            "Event 'remove' on client #312779",
        ] {
            let Ok(worth) = sound_worth_asking_after(line);

            assert_eq!(worth, Worth::Ignoring, "{line}");
        }
    }

    #[test]
    fn a_stream_starting_is_not_the_volume_changing() {
        let Ok(worth) = sound_worth_asking_after("Event 'new' on sink-input #74");

        assert_eq!(worth, Worth::Ignoring);
    }

    #[test]
    fn a_line_pactl_has_never_printed_is_read_again_rather_than_dropped() {
        let Ok(worth) = sound_worth_asking_after("Subscribed.");

        assert_eq!(worth, Worth::Asking);
    }

    #[test]
    fn a_notice_arriving_or_going_rings_the_bell() {
        let arrived = concat!(
            "  Sender=:1.92 Destination=org.freedesktop.Notifications ",
            "Path=/org/freedesktop/Notifications ",
            "Interface=org.freedesktop.Notifications  Member=Notify"
        );
        let closed = concat!(
            "  Sender=:1.65 Path=/org/freedesktop/Notifications ",
            "Interface=org.freedesktop.Notifications  Member=NotificationClosed"
        );
        let Ok(arrived) = notice_worth_asking_after(arrived);
        let Ok(closed) = notice_worth_asking_after(closed);

        assert_eq!(arrived, Worth::Asking);
        assert_eq!(closed, Worth::Asking);
    }

    #[test]
    fn asking_what_is_waiting_does_not_ask_what_is_waiting() {
        let listed = concat!(
            "  Sender=:1.92 Destination=org.freedesktop.Notifications ",
            "Path=/fr/emersion/Mako Interface=fr.emersion.Mako  Member=ListNotifications"
        );
        let modes = concat!(
            "  Sender=:1.92 Destination=org.freedesktop.Notifications ",
            "Path=/fr/emersion/Mako Interface=fr.emersion.Mako  Member=ListModes"
        );
        let Ok(listed) = notice_worth_asking_after(listed);
        let Ok(modes) = notice_worth_asking_after(modes);

        assert_eq!(listed, Worth::Ignoring);
        assert_eq!(modes, Worth::Ignoring);
    }

    #[test]
    fn the_mode_changing_rings_the_bell() {
        let set = concat!(
            "  Sender=:1.92 Destination=org.freedesktop.Notifications ",
            "Path=/fr/emersion/Mako Interface=fr.emersion.Mako  Member=SetMode"
        );
        let Ok(set) = notice_worth_asking_after(set);

        assert_eq!(set, Worth::Asking);
    }

    #[test]
    fn the_bus_talking_about_connections_is_not_a_notice() {
        for line in [
            concat!(
                "  Sender=:1.92 Destination=org.freedesktop.DBus ",
                "Path=/org/freedesktop/DBus Interface=org.freedesktop.DBus  Member=Hello"
            ),
            concat!(
                "  Sender=org.freedesktop.DBus Path=/org/freedesktop/DBus ",
                "Interface=org.freedesktop.DBus  Member=NameOwnerChanged"
            ),
        ] {
            let Ok(worth) = notice_worth_asking_after(line);

            assert_eq!(worth, Worth::Ignoring, "{line}");
        }
    }

    #[test]
    fn somebody_elses_conversation_on_the_bus_is_not_a_notice() {
        let mpris = concat!(
            "  Sender=:1.92 Destination=org.mpris.MediaPlayer2.console ",
            "Path=/org/mpris/MediaPlayer2 ",
            "Interface=org.freedesktop.DBus.Properties  Member=Get"
        );
        let Ok(worth) = notice_worth_asking_after(mpris);

        assert_eq!(worth, Worth::Ignoring);
    }

    #[test]
    fn what_carries_no_message_at_all_is_not_a_notice() {
        for line in ["‣ Type=method_call  Endian=l  Flags=0", "  };", ""] {
            let Ok(worth) = notice_worth_asking_after(line);

            assert_eq!(worth, Worth::Ignoring, "{line}");
        }
    }
}

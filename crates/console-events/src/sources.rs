//! Where the words come from, one subscription each.
//!
//! The one that is left out is left out for a reason rather than for want of
//! an afternoon. A source is held on the first `listen`
//! for its topic and never before, so a topic no one is listening to costs
//! nothing at all: what decides whether one belongs here is whether the thing
//! that says a change has happened already exists as a program, not whether
//! anyone is asking yet. What a topic with no source does is say so on the
//! journal and hand out nothing, which is a topic that is quiet rather than a
//! pool that is broken.
//!
//! **`Units` has no source because nothing on this machine emits one.** systemd
//! sends `UnitNew`, `JobRemoved` and the rest only while some connection is
//! holding a `Subscribe` open, and a monitor sees what is sent rather than
//! asking for it -- so a `busctl monitor` here would sit watching a bus that
//! stays silent and report a machine where no unit ever changes, which is worse
//! than a topic that says it is quiet. What it wants is a connection that
//! subscribes and stays, and that is a program rather than a line in this file.
//!
//! **`Path` is a folder, and the kernel is its source.** Every other topic
//! here is one watcher for the whole machine; a path is a watcher per path, and
//! that fits the same shape because a `Topic::Path` names its folder: two
//! folders are two topics, held once each like any other. `watching` is the
//! watcher. A path that is not absolute is not a folder anyone can mean, and
//! is refused rather than read against wherever the pool was started.
//!
//! Nothing a program says is passed on. There was one topic a program could
//! publish on, a download finishing, and it was a program speaking for the
//! machine about one kind of change; the folder the download landed in says the
//! same thing now, for every kind of change and every program, and a pool that
//! took words from programs was a pool that had to decide which of them to
//! believe.
//!
//! **What the bus watches is narrowed where it is asked rather than where it is
//! read.** `busctl monitor` given a name is not filtered to that name -- traffic
//! to the music player turns up in a monitor of the notification service -- so
//! without a match the pool would relay every message on the session bus to
//! whoever asked about notifications. The match is the pool spelling what it *asks*,
//! which is its own business the way `pactl subscribe` and `nmcli monitor`
//! already are; what an answer means is still the subscriber's, and the names
//! are exported so that the one filter that reads them does not spell them a
//! second time.
//!
//! **Bluetooth is heard through `gdbus monitor`, because `busctl` may not
//! watch the system bus.** `busctl --system monitor` asks the bus to make it a
//! monitor, and the system bus answers that for root alone -- `Access denied`,
//! tried rather than read. `gdbus monitor --dest` asks for nothing so large:
//! it adds an ordinary match for the signals one name sends, which any
//! connection may, and prints each on a line of its own. Signals are all it
//! hears, so the bar asking bluetoothd what it is -- a method call and its
//! reply -- is not something it can mistake for bluetoothd saying something
//! changed; `bluetoothctl show` was run beside it to see, and it printed
//! nothing. It goes through `stdbuf` for the reason `busctl` does: what it
//! writes into a pipe waits for a full buffer otherwise.
//!
//! **The Wi-Fi is NetworkManager on the system bus, for the same reason.**
//! `nmcli monitor` is `Network`, and it says a device connected and never how
//! strong anything is. What the bus says that it does not is every access
//! point's strength and the moment a scan finished, which is when a list of
//! what is in range is new rather than remembered. It is a topic of its own
//! because the bar wakes on every line of `Network`, and a street full of
//! access points being seen again is not a line it should wake for.
//!
//! **The battery is the kernel's `power_supply` uevents, which is a netlink
//! socket.** `udevadm monitor` on the subsystem rather than on a name, because
//! what changes when this device is plugged in is a USB-C supply one day and
//! the adapter another. It is the one source here that is not a socket in the
//! runtime directory or a bus, and so the one that asked the unit for
//! `AF_NETLINK`, which is a socket to the kernel and still no way off this
//! machine. UPower would have said the same thing on the bus, and it is not in
//! `[packages]`: a daemon installed so that one line could be read from it is a
//! daemon polling the battery so that nothing here has to, which is what the
//! bar's tick already is.
//!
//! **A source that is someone else's program is started `alongside`.** The
//! pool is the only thing holding it, so it dies when the pool does -- by a
//! death signal and by a drop, because a pool that was killed outright must not
//! leave a `pactl subscribe` behind it. Leaving one behind is the exact fault
//! this crate exists to end, and it would be a poor joke to commit it here.
//!
//! **What a source says is not read.** `pactl subscribe` writes a line per
//! event and the line is passed on whole; whoever asked for `Sound` is the one
//! that knows a sink from a source, and the pool that decided would be
//! deciding for every listener at once.
//!
//! What the sound was watched doing, under the unit's own confinement rather
//! than in a shell. `PrivateDevices=yes` and `RestrictAddressFamilies=AF_UNIX`
//! do not stop it: `pactl` reaches the server over a socket in the runtime
//! directory and never opens `/dev/snd`, so the sandbox the pool already had
//! is the sandbox this runs in. Killed outright it is started again and
//! whoever was listening goes on hearing without reconnecting, because the
//! reaching-again is on this side of the pool rather than the far side. And a
//! pool killed with `-9` leaves nothing behind, which was the thing to check
//! rather than assume.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::process::Stdio;
use std::sync::mpsc::Sender;

use console_program_lifetime::alongside;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_program_contract::{Change, Topic};
use console_core_reconnect::{Round, keep};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subscribed {
    Yes,
    No,
}

pub const NOTIFICATIONS: &str = "org.freedesktop.Notifications";

pub const OURS: &str = "console.Notifications";

pub const PLAYERS: &str = "/org/mpris/MediaPlayer2";

pub const NETWORK_MANAGER: &str = "org.freedesktop.NetworkManager";

pub const SCANNED: &str = "'LastScan'";

pub const BLUEZ: &str = "org.bluez";

pub const ADAPTER: &str = "org.bluez.Adapter1";

pub const DEVICE: &str = "org.bluez.Device1";

pub fn handed_to(
    waiting: Option<&Sender<Sender<Change>>>,
    wanted: &Topic,
    topic: &Topic,
    say: Sender<Change>,
) -> Result<Subscribed, Never> {
    Ok(match (topic == wanted, waiting) {
        (true, Some(waiting)) => match waiting.send(say) {
            Ok(()) => Subscribed::Yes,
            Err(_nobody_waiting_any_more) => Subscribed::No,
        },
        (true, None) | (false, _) => Subscribed::No,
    })
}

pub fn hold(topic: &Topic, say: Sender<Change>) -> Result<Subscribed, Never> {
    Ok(match topic {
        Topic::Compositor => {
            let Ok(()) = compositor(say);

            Subscribed::Yes
        }
        Topic::Sound => {
            let Ok(arguments) = worded(&["subscribe"]);
            let Ok(()) = theirs(Topic::Sound, Program::Pactl, arguments, say);

            Subscribed::Yes
        }
        Topic::Network => {
            let Ok(arguments) = worded(&["monitor"]);
            let Ok(()) = theirs(Topic::Network, Program::Nmcli, arguments, say);

            Subscribed::Yes
        }
        Topic::Wifi => {
            let Ok(gdbus) = Program::Gdbus.name();
            let Ok(arguments) = worded(&["-oL", gdbus, "monitor", "--system", "--dest", NETWORK_MANAGER]);
            let Ok(()) = theirs(Topic::Wifi, Program::Stdbuf, arguments, say);

            Subscribed::Yes
        }
        Topic::Bluetooth => {
            let Ok(gdbus) = Program::Gdbus.name();
            let Ok(arguments) = worded(&["-oL", gdbus, "monitor", "--system", "--dest", BLUEZ]);
            let Ok(()) = theirs(Topic::Bluetooth, Program::Stdbuf, arguments, say);

            Subscribed::Yes
        }
        Topic::Battery => {
            let Ok(arguments) = worded(&["monitor", "--udev", "--subsystem-match=power_supply"]);
            let Ok(()) = theirs(Topic::Battery, Program::Udevadm, arguments, say);

            Subscribed::Yes
        }
        Topic::Notifications => {
            let Ok(arguments) = monitoring(&[
                format!("--match=interface={NOTIFICATIONS}"),
                format!("--match=interface={OURS}"),
            ]);
            let Ok(()) = theirs(Topic::Notifications, Program::Stdbuf, arguments, say);

            Subscribed::Yes
        }
        Topic::Player => {
            let Ok(arguments) = monitoring(&[format!("--match=path={PLAYERS}")]);
            let Ok(()) = theirs(Topic::Player, Program::Stdbuf, arguments, say);

            Subscribed::Yes
        }
        Topic::Units => Subscribed::No,
        Topic::Path(folder) => match folder.is_absolute() {
            true => {
                let Ok(()) = crate::watching::watch(folder.clone(), say);

                Subscribed::Yes
            }
            false => Subscribed::No,
        },
    })
}

fn worded(arguments: &[&str]) -> Result<Vec<String>, Never> {
    Ok(arguments.iter().map(|word| (*word).to_string()).collect())
}

fn monitoring(matches: &[String]) -> Result<Vec<String>, Never> {
    let Ok(busctl) = Program::Busctl.name();
    let Ok(mut arguments) = worded(&["-oL", busctl, "--user", "monitor"]);

    arguments.extend(matches.iter().cloned());

    Ok(arguments)
}

fn compositor(say: Sender<Change>) -> Result<(), Never> {
    let mut said = false;

    let Ok(()) = keep(move || {
        let socket = match console_onscreen::events() {
            Ok(socket) => socket,
            Err(fault) => {
                match said {
                    true => {},
                    false => {
                        eprintln!("console-events: {fault}");
                        said = true;
                    }
                }

                return Round::Another;
            }
        };

        let stream = match UnixStream::connect(&socket) {
            Ok(stream) => stream,
            Err(fault) => {
                match said {
                    true => {},
                    false => {
                        eprintln!(
                            "console-events: {} would not open: {fault}; nothing is being told \
                             what the compositor is doing until it does",
                            socket.display()
                        );
                        said = true;
                    }
                }

                return Round::Another;
            }
        };

        said = false;

        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            let sent = say.send(Change { topic: Topic::Compositor, text: line });

            match sent {
                Ok(()) => {},
                Err(_) => return Round::Finished,
            }
        }

        Round::Another
    });

    Ok(())
}

fn theirs(
    about: Topic,
    program: Program,
    arguments: Vec<String>,
    say: Sender<Change>,
) -> Result<(), Never> {
    let Ok(()) = keep(move || {
        let Ok(mut asking) = program.command();

        asking.args(&arguments).stdout(Stdio::piped()).stderr(Stdio::null());

        let mut running = match alongside(&mut asking) {
            Ok(running) => running,
            Err(_fault) => return Round::Another,
        };

        let reading = match running.reading() {
            Ok(Some(reading)) => reading,
            Ok(None) | Err(_) => return Round::Another,
        };

        for line in BufReader::new(reading).lines().map_while(Result::ok) {
            let sent = say.send(Change { topic: about.clone(), text: line });

            match sent {
                Ok(()) => {},
                Err(_) => return Round::Finished,
            }
        }

        Round::Another
    });

    Ok(())
}

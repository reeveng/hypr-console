//! Where the words come from, one subscription each.
//!
//! Five of them now, and the two that are left out are left out for a reason
//! rather than for want of an afternoon. A source is held on the first `listen`
//! for its topic and never before, so a topic nobody is listening to costs
//! nothing at all: what decides whether one belongs here is whether the thing
//! that says a change has happened already exists as a program, not whether
//! anybody is asking yet. What a topic with no source does is say so on the
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
//! **`Path` has no source because it is not one subscription.** Every other
//! topic here is one watcher for the whole machine; a path is a different
//! watcher per path, held for as long as somebody wants that path and dropped
//! when they stop, and nothing here can tell them apart -- `held` is a list of
//! topics that have been started once. It also wants inotify, which is a
//! package this desktop does not have or a crate it does not carry. Both of
//! those are decisions, and neither is this one.
//!
//! **What the bus watches is narrowed where it is asked rather than where it is
//! read.** `busctl monitor` given a name is not filtered to that name -- traffic
//! to the music player turns up in a monitor of the notification service -- so
//! without a match the pool would relay every message on the session bus to
//! whoever asked about notices. The match is the pool spelling what it *asks*,
//! which is its own business the way `pactl subscribe` and `nmcli monitor`
//! already are; what an answer means is still the subscriber's, and the names
//! are exported so that the one filter that reads them does not spell them a
//! second time.
//!
//! **A source that is somebody else's program is started `alongside`.** The
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
use console_program_contract::{Changed, Topic};
use console_core_reconnect::{Round, keep};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    Yes,
    Nothing,
}

pub const NOTICES: &str = "org.freedesktop.Notifications";

pub const MAKO: &str = "fr.emersion.Mako";

pub const PLAYERS: &str = "/org/mpris/MediaPlayer2";

pub fn hold(topic: &Topic, say: Sender<Changed>) -> Result<Held, Never> {
    Ok(match topic {
        Topic::Compositor => {
            let Ok(()) = compositor(say);

            Held::Yes
        }
        Topic::Sound => {
            let Ok(argv) = worded(&["subscribe"]);
            let Ok(()) = theirs(Topic::Sound, Program::Pactl, argv, say);

            Held::Yes
        }
        Topic::Network => {
            let Ok(argv) = worded(&["monitor"]);
            let Ok(()) = theirs(Topic::Network, Program::Nmcli, argv, say);

            Held::Yes
        }
        Topic::Notices => {
            let Ok(argv) = monitoring(&[
                format!("--match=interface={NOTICES}"),
                format!("--match=interface={MAKO}"),
            ]);
            let Ok(()) = theirs(Topic::Notices, Program::Stdbuf, argv, say);

            Held::Yes
        }
        Topic::Player => {
            let Ok(argv) = monitoring(&[format!("--match=path={PLAYERS}")]);
            let Ok(()) = theirs(Topic::Player, Program::Stdbuf, argv, say);

            Held::Yes
        }
        Topic::Units => Held::Nothing,
        Topic::Path(_) => Held::Nothing,
    })
}

fn worded(argv: &[&str]) -> Result<Vec<String>, Never> {
    Ok(argv.iter().map(|word| (*word).to_string()).collect())
}

fn monitoring(matches: &[String]) -> Result<Vec<String>, Never> {
    let Ok(busctl) = Program::Busctl.name();
    let Ok(mut argv) = worded(&["-oL", busctl, "--user", "monitor"]);

    argv.extend(matches.iter().cloned());

    Ok(argv)
}

fn compositor(say: Sender<Changed>) -> Result<(), Never> {
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
            let told = say.send(Changed { about: Topic::Compositor, said: line });

            match told {
                Ok(()) => {},
                Err(_) => return Round::Done,
            }
        }

        Round::Another
    });

    Ok(())
}

fn theirs(
    about: Topic,
    program: Program,
    argv: Vec<String>,
    say: Sender<Changed>,
) -> Result<(), Never> {
    let Ok(()) = keep(move || {
        let Ok(mut asking) = program.command();

        asking.args(&argv).stdout(Stdio::piped()).stderr(Stdio::null());

        let mut running = match alongside(&mut asking) {
            Ok(running) => running,
            Err(_fault) => return Round::Another,
        };

        let reading = match running.reading() {
            Ok(Some(reading)) => reading,
            Ok(None) | Err(_) => return Round::Another,
        };

        for line in BufReader::new(reading).lines().map_while(Result::ok) {
            let told = say.send(Changed { about: about.clone(), said: line });

            match told {
                Ok(()) => {},
                Err(_) => return Round::Done,
            }
        }

        Round::Another
    });

    Ok(())
}

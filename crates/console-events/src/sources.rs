//! Where the words come from, one subscription each.
//!
//! Two so far -- the compositor and the sound -- because the document this
//! comes from says one at a time and means it: each of the others is a program
//! of somebody else's whose output has to be watched in a nested desktop
//! before anybody can say what it does when it is restarted underneath. What a
//! topic with no source does is say so on the journal and hand out nothing,
//! which is a topic that is quiet rather than a pool that is broken.
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

pub fn hold(topic: &Topic, say: Sender<Changed>) -> Result<Held, Never> {
    Ok(match topic {
        Topic::Compositor => {
            let Ok(()) = compositor(say);

            Held::Yes
        }
        Topic::Sound => {
            let Ok(()) = theirs(Topic::Sound, Program::Pactl, &["subscribe"], say);

            Held::Yes
        }
        Topic::Network | Topic::Notices | Topic::Units | Topic::Player => Held::Nothing,
        Topic::Path(_) => Held::Nothing,
    })
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
    argv: &'static [&'static str],
    say: Sender<Changed>,
) -> Result<(), Never> {
    let Ok(()) = keep(move || {
        let Ok(mut asking) = program.command();

        asking.args(argv).stdout(Stdio::piped()).stderr(Stdio::null());

        let Ok(mut running) = alongside(&mut asking) else {
            return Round::Another;
        };

        let Ok(Some(reading)) = running.reading() else {
            return Round::Another;
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

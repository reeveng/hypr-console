//! Where the words come from, one subscription each.
//!
//! One source is held here so far -- the compositor -- because the document
//! this comes from says one at a time and means it: each of the others is a
//! program of somebody else's whose output has to be watched in a nested
//! desktop before anybody can say what it does when it is restarted underneath.
//! What a topic with no source does is say so on the journal and hand out
//! nothing, which is a topic that is quiet rather than a pool that is broken.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Sender;

use console_never::Never;
use console_program_contract::{Changed, Topic};
use console_reconnect::{Round, keep};

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
        Topic::Sound | Topic::Network | Topic::Notices | Topic::Units | Topic::Player => {
            Held::Nothing
        }
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

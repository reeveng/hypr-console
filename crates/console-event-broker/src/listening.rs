//! The other end: a program asking the pool to tell it things.
//!
//! What comes back is a channel of [`Changed`], which is the same shape a
//! program's own subscription had, so moving one over is a change of what it
//! opens rather than of how it is written.
//!
//! **It re-subscribes on every reconnection, and that is the point.** A pool
//! that was restarted knows nothing about who was listening to it, and a
//! program that subscribed once at start would go quiet for ever without
//! saying so. Re-subscribing also asks for the replay again, which is right:
//! after a gap, what a program wants is what is true now rather than the next
//! thing to change.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};

use console_never::Never;
use console_program_contract::{Changed, Topic};
use console_reconnect::{Round, keep};

use crate::place;
use crate::wire::{self, Says};

pub fn listen(topics: &[Topic]) -> Result<Receiver<Changed>, Never> {
    let socket = match place::socket() {
        Ok(socket) => socket,
        Err(fault) => {
            eprintln!("console-events: {fault}, so there is no pool to ask");

            let (_say, heard) = channel();

            return Ok(heard);
        }
    };

    listen_at(&socket, topics)
}

pub fn listen_at(socket: &Path, topics: &[Topic]) -> Result<Receiver<Changed>, Never> {
    let (say, heard) = channel();
    let wanted: Vec<Topic> = topics.to_vec();
    let at = socket.to_path_buf();

    let Ok(()) = keep(move || {
        let Ok(round) = round(&at, &wanted, &say);

        round
    });

    Ok(heard)
}

fn round(socket: &Path, wanted: &[Topic], say: &Sender<Changed>) -> Result<Round, Never> {
    let Ok(mut stream) = UnixStream::connect(socket) else {
        return Ok(Round::Another);
    };

    for topic in wanted {
        let Ok(spelt) = wire::spelt(&Says::Listen(topic.clone()));

        let asked = writeln!(stream, "{spelt}");

        match asked {
            Ok(()) => {},
            Err(_) => return Ok(Round::Another),
        }
    }

    let reading = match stream.try_clone() {
        Ok(reading) => reading,
        Err(_) => return Ok(Round::Another),
    };

    for line in BufReader::new(reading).lines().map_while(Result::ok) {
        let Ok(said) = wire::read(&line);

        match said {
            Some(Says::Said(changed)) => {
                let told = say.send(changed);

                match told {
                    Ok(()) => {},
                    Err(_) => return Ok(Round::Done),
                }
            }
            Some(Says::Listen(_) | Says::Deafen(_)) | None => {},
        }
    }

    Ok(Round::Another)
}

//! The other end: a program asking the pool to tell it things.
//!
//! What comes back is a channel carrying the same [`Changed`] a program's own
//! subscription handed it, so moving one over is a change of what it opens
//! rather than of how it is written.
//!
//! **It re-subscribes on every reconnection, and that is the point.** A pool
//! that was restarted knows nothing about who was listening to it, and a
//! program that subscribed once at start would go quiet for ever without
//! saying so. Re-subscribing also asks for the replay again, which is right:
//! after a gap, what a program wants is what is true now rather than the next
//! thing to change.
//!
//! **What is wanted can change while it is being heard.** A panel that has
//! gone behind something wants nothing until it comes back, and the whole of
//! the pool's saving is in that: a listener that stays connected and quiet is
//! a handheld that stays asleep. So the topics are held rather than taken
//! once, [`Listening::also`] and [`Listening::not`] write a `listen` or a
//! `deafen` down the connection that is already open, and the reconnection
//! asks for whatever is wanted at the moment it gets in. Coming back is
//! nothing more than asking again, because the pool replays the last word on a
//! topic to whoever has just subscribed -- so a panel that is uncovered knows
//! the volume without waiting for somebody to change it.
//!
//! **Getting in is itself worth hearing.** A subscriber that was away has
//! missed whatever happened while it was, and for a topic where the last word
//! is the whole answer the replay says so by itself -- but for one where the
//! words are *something changed, ask again*, a gap is a reading that is quietly
//! out of date and nothing arrives to correct it. So [`Heard::GotIn`] is said
//! each time the subscription is made, which is what every watch this replaced
//! did when it connected. Whoever does not need it matches it and does nothing,
//! because the replay is already on its way behind it.
//!
//! **The channel outlives every connection to the pool.** The sender is kept
//! here as well as in the thread that reconnects, so a program waiting on the
//! words is told nothing while the pool is down rather than told that the
//! channel has gone -- which is the difference between a wait and a spin. It
//! is also why a program that wants no topics at all can still wait here: the
//! nothing it hears is the same nothing, and the pool is not asked for a
//! connection until something is wanted.

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};

use console_core_never::Never;
use console_program_contract::{Changed, Topic};
use console_core_reconnect::{Round, keep};

use crate::place;
use crate::wire::{self, Says};

pub struct Listening {
    wanted: Arc<Mutex<Wanted>>,
    said: Sender<Heard>,
    heard: Receiver<Heard>,
    at: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    GotIn,
    Said(Changed),
}

#[derive(Default)]
struct Wanted {
    topics: BTreeSet<Topic>,
    telling: Option<UnixStream>,
    asking: Asking,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Asking {
    Yes,
    #[default]
    NotYet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wanting {
    Something,
    Nothing,
}

impl Listening {
    pub fn heard(&self) -> Result<&Receiver<Heard>, Never> {
        Ok(&self.heard)
    }

    pub fn wanting(&self) -> Result<Wanting, Never> {
        let Ok(wanted) = held(&self.wanted);

        Ok(match wanted.topics.is_empty() {
            true => Wanting::Nothing,
            false => Wanting::Something,
        })
    }

    pub fn also(&self, topic: &Topic) -> Result<(), Never> {
        let Ok(mut wanted) = held(&self.wanted);

        match wanted.topics.insert(topic.clone()) {
            true => {},
            false => return Ok(()),
        }

        let Ok(()) = asks(&mut wanted, &Says::Listen(topic.clone()));

        match wanted.asking {
            Asking::Yes => return Ok(()),
            Asking::NotYet => {},
        }

        wanted.asking = Asking::Yes;

        drop(wanted);

        let Some(at) = &self.at else { return Ok(()) };

        keeping(at.clone(), Arc::clone(&self.wanted), self.said.clone())
    }

    pub fn not(&self, topic: &Topic) -> Result<(), Never> {
        let Ok(mut wanted) = held(&self.wanted);

        match wanted.topics.remove(topic) {
            true => {},
            false => return Ok(()),
        }

        asks(&mut wanted, &Says::Deafen(topic.clone()))
    }
}

pub fn listen(topics: &[Topic]) -> Result<Listening, Never> {
    let at = match place::socket() {
        Ok(at) => Some(at),
        Err(fault) => {
            eprintln!("console-events: {fault}, so there is no pool to ask");

            None
        }
    };

    started(at, topics)
}

pub fn listen_at(socket: &Path, topics: &[Topic]) -> Result<Listening, Never> {
    started(Some(socket.to_path_buf()), topics)
}

fn started(at: Option<PathBuf>, topics: &[Topic]) -> Result<Listening, Never> {
    let (said, heard) = channel();
    let listening =
        Listening { wanted: Arc::new(Mutex::new(Wanted::default())), said, heard, at };

    for topic in topics {
        let Ok(()) = listening.also(topic);
    }

    Ok(listening)
}

fn keeping(
    at: PathBuf,
    wanted: Arc<Mutex<Wanted>>,
    say: Sender<Heard>,
) -> Result<(), Never> {
    keep(move || {
        let Ok(round) = round(&at, &wanted, &say);

        round
    })
}

fn round(
    socket: &Path,
    wanted: &Arc<Mutex<Wanted>>,
    say: &Sender<Heard>,
) -> Result<Round, Never> {
    let Ok(stream) = UnixStream::connect(socket) else {
        return Ok(Round::Another);
    };

    let reading = match stream.try_clone() {
        Ok(reading) => reading,
        Err(_) => return Ok(Round::Another),
    };

    let Ok(told) = asked(wanted, stream);

    match told {
        Told::Went => {},
        Told::Refused => return Ok(Round::Another),
    }

    match say.send(Heard::GotIn) {
        Ok(()) => {},
        Err(_) => return Ok(Round::Done),
    }

    for line in BufReader::new(reading).lines().map_while(Result::ok) {
        let Ok(said) = wire::read(&line);

        match said {
            Some(Says::Said(changed)) => {
                let told = say.send(Heard::Said(changed));

                match told {
                    Ok(()) => {},
                    Err(_) => return Ok(Round::Done),
                }
            }
            Some(Says::Listen(_) | Says::Deafen(_)) | None => {},
        }
    }

    let Ok(mut wanted) = held(wanted);

    wanted.telling = None;

    Ok(Round::Another)
}

enum Told {
    Went,
    Refused,
}

fn asked(wanted: &Arc<Mutex<Wanted>>, mut stream: UnixStream) -> Result<Told, Never> {
    let Ok(mut wanted) = held(wanted);

    for topic in &wanted.topics {
        let Ok(spelt) = wire::spelt(&Says::Listen(topic.clone()));

        match writeln!(stream, "{spelt}") {
            Ok(()) => {},
            Err(_) => return Ok(Told::Refused),
        }
    }

    wanted.telling = Some(stream);

    Ok(Told::Went)
}

fn asks(wanted: &mut Wanted, says: &Says) -> Result<(), Never> {
    let Ok(spelt) = wire::spelt(says);

    let told = match &mut wanted.telling {
        Some(stream) => writeln!(stream, "{spelt}"),
        None => return Ok(()),
    };

    match told {
        Ok(()) => {},
        Err(_) => wanted.telling = None,
    }

    Ok(())
}

fn held(wanted: &Arc<Mutex<Wanted>>) -> Result<MutexGuard<'_, Wanted>, Never> {
    Ok(match wanted.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    })
}

//! The other end: a program asking the pool to tell it things.
//!
//! What comes back is a channel carrying the same [`Change`] a program's own
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
//! **What is wanted can change while it is being received.** A panel that has
//! gone behind something wants nothing until it comes back, and the whole of
//! the pool's saving is in that: a listener that stays connected and quiet is a
//! handheld that stays asleep. So the event groups are held rather than taken
//! once, [`Subscriber::also`] and [`Subscriber::not`] write a `listen` or a
//! `stop-listening` down the connection that is already open, and the
//! reconnection asks for whatever is wanted at the moment it gets in. Coming
//! back is nothing more than asking again, because the pool replays the last
//! word on a event group to whoever has just subscribed -- so a panel that is
//! uncovered knows the volume without waiting for someone to change it.
//!
//! **Getting in is itself worth receiving.** A subscriber that was away has
//! missed whatever happened while it was, and for an event group where the last
//! word is the whole answer the replay says so by itself -- but for one where
//! the words are *something changed, ask again*, a gap is a reading that is
//! quietly out of date and nothing arrives to correct it. So
//! [`Received::Connected`] is said each time the subscription is made, which is
//! what every watch this replaced did when it connected. Whoever does not need
//! it matches it and does nothing, because the replay is already on its way
//! behind it.
//!
//! **The channel outlives every connection to the pool.** The sender is kept
//! here as well as in the thread that reconnects, so a program waiting on the
//! words is told nothing while the pool is down rather than told that the
//! channel has gone -- which is the difference between a wait and a spin. It is
//! also why a program that wants no event groups at all can still wait here:
//! the nothing it receives is the same nothing, and the pool is not asked for a
//! connection until something is wanted.
//!
//! **The two halves come apart.** A receiver is read by one thread, and a
//! program that sleeps on something other than this channel -- a panel asleep
//! in `poll` on its compositor -- wants a thread of its own blocked on the
//! words while it goes on choosing what it wants to hear. So [`Subscriber::split`]
//! hands back the [`Subscriptions`] and the receiver separately, and each goes
//! to whoever does that half.

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};

use console_core_never::Never;
use console_program_contract::{Change, EventGroup};
use console_core_reconnect::{Round, keep};

use crate::place;
use crate::wire::{self, Message};

pub struct Subscriber {
    subscriptions: Subscriptions,
    receiver: Receiver<Received>,
}

pub struct Subscriptions {
    desired: Arc<Mutex<Wanted>>,
    sender: Sender<Received>,
    at: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Received {
    Connected,
    Event(Change),
}

#[derive(Default)]
struct Wanted {
    event_groups: BTreeSet<EventGroup>,
    connection: Option<UnixStream>,
    asking: Requested,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Requested {
    Yes,
    #[default]
    NotYet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desired {
    Some,
    None,
}

impl Subscriber {
    pub fn received(&self) -> Result<&Receiver<Received>, Never> {
        Ok(&self.receiver)
    }

    pub fn desired(&self) -> Result<Desired, Never> {
        self.subscriptions.desired()
    }

    pub fn subscribe(&self, event_group: &EventGroup) -> Result<(), Never> {
        self.subscriptions.subscribe(event_group)
    }

    pub fn unsubscribe(&self, event_group: &EventGroup) -> Result<(), Never> {
        self.subscriptions.unsubscribe(event_group)
    }

    pub fn split(self) -> Result<(Subscriptions, Receiver<Received>), Never> {
        Ok((self.subscriptions, self.receiver))
    }
}

impl Subscriptions {
    pub fn desired(&self) -> Result<Desired, Never> {
        let Ok(wanted) = lock(&self.desired);

        Ok(match wanted.event_groups.is_empty() {
            true => Desired::None,
            false => Desired::Some,
        })
    }

    pub fn subscribe(&self, event_group: &EventGroup) -> Result<(), Never> {
        let Ok(mut wanted) = lock(&self.desired);

        match wanted.event_groups.insert(event_group.clone()) {
            true => {},
            false => return Ok(()),
        }

        let Ok(()) = send(&mut wanted, &Message::Subscribe(event_group.clone()));

        match wanted.asking {
            Requested::Yes => return Ok(()),
            Requested::NotYet => {},
        }

        wanted.asking = Requested::Yes;

        drop(wanted);

        let at = match &self.at {
            Some(at) => at,
            None => return Ok(()),
        };

        keep_connected(at.clone(), Arc::clone(&self.desired), self.sender.clone())
    }

    pub fn unsubscribe(&self, event_group: &EventGroup) -> Result<(), Never> {
        let Ok(mut wanted) = lock(&self.desired);

        match wanted.event_groups.remove(event_group) {
            true => {},
            false => return Ok(()),
        }

        send(&mut wanted, &Message::Unsubscribe(event_group.clone()))
    }
}

pub fn connect(event_groups: &[EventGroup]) -> Result<Subscriber, Never> {
    let at = match place::socket() {
        Ok(at) => Some(at),
        Err(fault) => {
            eprintln!("console-events: {fault}, so there is no pool to ask");

            None
        }
    };

    started(at, event_groups)
}

pub fn connect_at(socket: &Path, event_groups: &[EventGroup]) -> Result<Subscriber, Never> {
    started(Some(socket.to_path_buf()), event_groups)
}

fn started(at: Option<PathBuf>, event_groups: &[EventGroup]) -> Result<Subscriber, Never> {
    let (sender, receiver) = channel();
    let subscriptions = Subscriptions { desired: Arc::new(Mutex::new(Wanted::default())), sender, at };
    let subscriber = Subscriber { subscriptions, receiver };

    for event_group in event_groups {
        #[cfg_attr(
            dylint_lib = "explicit043_no_unmatched_listen",
            allow(
                explicit043_no_unmatched_listen,
                reason = "the event groups the connection is opened with, which it is stopped listening by being dropped -- the pair this rule is about is an event group added part way through a life that goes on"
            )
        )]
        let Ok(()) = subscriber.subscribe(event_group);
    }

    Ok(subscriber)
}

fn keep_connected(
    at: PathBuf,
    desired: Arc<Mutex<Wanted>>,
    sender: Sender<Received>,
) -> Result<(), Never> {
    keep(move || {
        let Ok(round) = round(&at, &desired, &sender);

        round
    })
}

fn round(
    socket: &Path,
    desired: &Arc<Mutex<Wanted>>,
    sender: &Sender<Received>,
) -> Result<Round, Never> {
    let stream = match UnixStream::connect(socket) {
        Ok(stream) => stream,
        Err(_no_one_is_listening) => return Ok(Round::Another),
    };

    let reading = match stream.try_clone() {
        Ok(reading) => reading,
        Err(_the_socket_would_not_clone) => return Ok(Round::Another),
    };

    let Ok(subscribed) = subscribed(desired, stream);

    match subscribed {
        SubscribeResult::Subscribed => {},
        SubscribeResult::Rejected => return Ok(Round::Another),
    }

    match sender.send(Received::Connected) {
        Ok(()) => {},
        Err(_no_one_is_listening) => return Ok(Round::Finished),
    }

    for line in BufReader::new(reading).lines().map_while(Result::ok) {
        let Ok(message) = wire::decoded(&line);

        match message {
            Some(Message::Publish(change)) => {
                let sent = sender.send(Received::Event(change));

                match sent {
                    Ok(()) => {},
                    Err(_no_one_is_listening) => return Ok(Round::Finished),
                }
            }
            Some(Message::Subscribe(_) | Message::Unsubscribe(_)) | None => {},
        }
    }

    let Ok(mut wanted) = lock(desired);

    wanted.connection = None;

    Ok(Round::Another)
}

enum SubscribeResult {
    Subscribed,
    Rejected,
}

fn subscribed(wanted: &Arc<Mutex<Wanted>>, mut stream: UnixStream) -> Result<SubscribeResult, Never> {
    let Ok(mut wanted) = lock(wanted);

    for event_group in &wanted.event_groups {
        let Ok(spelled) = wire::encoded(&Message::Subscribe(event_group.clone()));

        match writeln!(stream, "{spelled}") {
            Ok(()) => {},
            Err(_the_socket_has_closed) => return Ok(SubscribeResult::Rejected),
        }
    }

    wanted.connection = Some(stream);

    Ok(SubscribeResult::Subscribed)
}

fn send(wanted: &mut Wanted, message: &Message) -> Result<(), Never> {
    let Ok(spelled) = wire::encoded(message);

    let sent = match &mut wanted.connection {
        Some(stream) => writeln!(stream, "{spelled}"),
        None => return Ok(()),
    };

    match sent {
        Ok(()) => {},
        Err(_the_socket_has_closed) => wanted.connection = None,
    }

    Ok(())
}

fn lock(wanted: &Arc<Mutex<Wanted>>) -> Result<MutexGuard<'_, Wanted>, Never> {
    Ok(match wanted.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    })
}

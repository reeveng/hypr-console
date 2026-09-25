//! The pool over a socket: who is connected, what they asked for, and what to
//! do when a source says something.  Split from the program so that it can be
//! started on a socket of a test's own, with sources of a test's own. What that
//! buys is the one thing the unit tests in `pool` cannot ask: that a program
//! which asks for a topic over a real socket is told the last word on it
//! *before* anything changes, which is the whole reason the pool remembers
//! anything.  Where the words come from is handed in, the same way
//! `console_input_controller::turning` is handed a machine. There is no other
//! way to ask what the pool does about a compositor that says something,
//! because the only compositor is the one this is running under.
//!
//! ## One thread, asleep in `poll`
//!
//! This used to be a thread per program to read it, a thread per program to
//! write to it, a thread per topic to carry a source's words across, and one
//! for the door: two threads a subscriber, most of them asleep in a blocking
//! call, and every word crossing a channel twice before it reached a socket.
//! The pool was always decided on one thread. Now it is heard and told on that
//! thread as well, which waits in one `poll` over the door, every connection,
//! and a pipe that a source's word wakes it through.
//!
//! What sources say still arrives on a channel, because a source is a thread
//! of its own that this does not own and `Holding` hands it a `Sender`. One
//! thread carries every topic's words across and writes a byte down the pipe,
//! which is the one place a channel meets `poll`.
//!
//! ## One program that stops reading is not everyone's silence
//!
//! Everyone is told from this one loop, and a blocking write into a socket
//! whose reader has gone to sleep waits once the kernel's buffer for it is
//! full. That is not the slow program's problem, it is everyone's: the loop
//! that would have told the others is inside that write. On a desktop it reads
//! as the volume freezing on the bar because a panel behind a picker stopped
//! reading, which is a fault in the last place anyone would look for it.
//!
//! So no socket here blocks. A write takes what the kernel will take and the
//! rest waits in that program's outbox until `poll` says there is room, and
//! the slowest subscriber on the machine costs everyone else nothing.
//!
//! What is bounded is the bytes a program has been sent and not read, rather
//! than a count of words: a burst of short lines is nothing to hold and a
//! program that has stopped reading is megabytes within seconds, and the second
//! is the one worth ending. Bounding the count instead let a burst end healthy
//! subscriptions -- a hundred thousand words handed over at once dropped every
//! program on the machine and had them all reconnecting a second later, which
//! measured as a fortieth of the throughput and half the words lost.
//!
//! **A full outbox ends the connection rather than dropping the word.** These
//! words are what is true now, so a program that missed some of them holds a
//! picture that is wrong and has no way to find out -- dropping a line is
//! silent and permanent. Being let go is the recoverable one, and every piece
//! of that is already here: `console-core-reconnect` brings the program back,
//! `Client` asks for its topics again, the pool replays the last word on
//! each, and `Heard::GotIn` tells it there was a gap. It comes back knowing
//! what is true instead of carrying on with what it missed.
//!
//! **A burst is written in pieces rather than a line at a time.** Words that
//! are already waiting are gathered as they are read, spelled once each, and
//! appended to every outbox that wants them; the outboxes are written when
//! nothing else is waiting or when [`BATCH`] has gathered, whichever comes
//! first. One word alone is still one write, so nothing about a quiet desktop
//! changes; a thousand words in a burst is one write per program instead of a
//! thousand. [`BATCH`] exists because a burst gathered whole is a burst that
//! starts arriving only when it has finished, so it bounds the wait as well as
//! the memory.
//!
//! ## A door that cannot open is not a door that is quiet
//!
//! `accept` takes the descriptor it is going to need before it does anything
//! else, so a process with none left is refused with `EMFILE` before the queue
//! is even looked at, and nothing about asking again changes that answer. Under
//! `poll` that is worse than it was under a blocking `accept`: the door stays
//! readable for as long as someone is queued at it, so a refusal is a `poll`
//! that returns at once, for ever, in the one thread everything else needs.
//!
//! So a door that refuses is left out of the next [`BREATH`] of waiting. It is
//! long enough that a door which cannot open costs nothing and short enough
//! that a program waiting to be let in does not notice, and the wait is the
//! whole of it: descriptors come back, the connection that was queued is
//! taken, and the pool carries on. Saying so once rather than every turn is the
//! other half, because a fault that repeats faster than it can be read is a
//! fault no one reads.

use std::collections::BTreeMap;
use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_program_contract::{Change, Topic};
use console_program_lifetime::threads;
use console_waiting::woken::{self, Woken};
use rustix::event::{Nsecs, PollFd, PollFlags, Secs, Timespec, poll};

use crate::Unserved;
use crate::pool::{Pool, Who};
use crate::sources::Subscribed;
use crate::wire::{self, Message};

const BREATH: Duration = Duration::from_millis(200);

pub const OUTBOX: u64 = 4 << 20;

pub const BATCH: u64 = 64 << 10;

const READING: u32 = 64 << 10;

struct Client {
    connection: UnixStream,
    heard: Vec<u8>,
    outbox: Vec<u8>,
}

pub type Holding = fn(&Topic, Sender<Change>) -> Result<Subscribed, Never>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Door {
    Open,
    Resting,
    Retrying,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rejected {
    NotYet,
    Interrupted,
    Gone,
}

fn refused(fault: &std::io::Error) -> Result<Rejected, Never> {
    let kind = fault.kind();

    Ok(match (kind == ErrorKind::WouldBlock, kind == ErrorKind::Interrupted) {
        (true, true) | (true, false) => Rejected::NotYet,
        (false, true) => Rejected::Interrupted,
        (false, false) => Rejected::Gone,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Watched {
    Woken,
    Door,
    Client(Who),
}

struct Serving {
    pool: Pool,
    clients: BTreeMap<Who, Client>,
    held: Vec<Topic>,
    sources: Sender<Change>,
    holding: Holding,
    carried: u64,
}

pub fn serve(socket: &Path, holding: Holding) -> Result<(), Unserved> {
    let listening = bound(socket)?;
    let waking = woken::pipe().map_err(Unserved::Waking)?;
    let Woken { waiting, saying } = waking;
    let (sources, arriving) = channel::<Change>();
    let (carrying, published) = channel::<Change>();

    let Ok(()) = threads::let_go(std::thread::spawn(move || {
        let mut saying = saying;

        for change in arriving {
            match carrying.send(change) {
                Ok(()) => {},
                Err(_no_one_is_listening) => return,
            }

            let _ = saying.write(&[1]);
        }
    }));

    let mut serving =
        Serving { pool: Pool::default(), clients: BTreeMap::new(), held: Vec::new(), sources, holding, carried: 0 };
    let mut door = Door::Open;

    loop {
        let ready = ready(&serving, &listening, &waiting, door)?;

        door = match door {
            Door::Resting => Door::Retrying,
            Door::Open | Door::Retrying => door,
        };

        for (watched, flags) in ready {
            match watched {
                Watched::Woken => {
                    let Ok(()) = woken::drained(&waiting);
                    let Ok(()) = told(&mut serving, &published);
                }
                Watched::Door => {
                    let Ok(let_in) = let_in(&mut serving, &listening, door);

                    door = let_in;
                }
                Watched::Client(who) => {
                    let Ok(()) = heard(&mut serving, who, flags);
                }
            }
        }

        let Ok(()) = written(&mut serving);
    }
}

fn bound(socket: &Path) -> Result<UnixListener, Unserved> {
    let at = match socket.parent() {
        Some(at) => at,
        None => return Err(Unserved::Rootless),
    };
    let made = std::fs::create_dir_all(at);

    made.map_err(|fault| Unserved::Holding(at.to_path_buf(), fault))?;

    let deleted = std::fs::remove_file(socket);

    match deleted {
        Ok(()) => {},
        Err(_nothing_to_remove) => {},
    }

    let listening = UnixListener::bind(socket)
        .map_err(|fault| Unserved::Unbound(socket.to_path_buf(), fault))?;
    let nonblocking = listening.set_nonblocking(true);

    nonblocking.map_err(|fault| Unserved::Unbound(socket.to_path_buf(), fault))?;

    Ok(listening)
}

fn ready(
    serving: &Serving,
    listening: &UnixListener,
    waiting: &std::os::fd::OwnedFd,
    door: Door,
) -> Result<Vec<(Watched, PollFlags)>, Unserved> {
    let mut watch = vec![PollFd::new(waiting, PollFlags::IN)];
    let mut which = vec![Watched::Woken];

    match door {
        Door::Open | Door::Retrying => {
            watch.push(PollFd::new(listening, PollFlags::IN));
            which.push(Watched::Door);
        }
        Door::Resting => {},
    }

    for (who, client) in &serving.clients {
        let flags = match client.outbox.is_empty() {
            true => PollFlags::IN,
            false => PollFlags::IN | PollFlags::OUT,
        };

        watch.push(PollFd::from_borrowed_fd(client.connection.as_fd(), flags));
        which.push(Watched::Client(*who));
    }

    let Ok(breath) = breath(door);

    match poll(&mut watch, breath.as_ref()) {
        Ok(_) => {},
        Err(rustix::io::Errno::INTR) => return Ok(Vec::new()),
        Err(fault) => return Err(Unserved::Waiting(fault)),
    }

    Ok(which
        .into_iter()
        .zip(watch.iter().map(|fd| fd.revents()))
        .filter(|(_, flags)| !flags.is_empty())
        .collect())
}

fn breath(door: Door) -> Result<Option<Timespec>, Never> {
    Ok(match door {
        Door::Resting => {
            let Ok(seconds) = fitted::<u64, Secs>(BREATH.as_secs());
            let Ok(nanoseconds) = fitted::<u32, Nsecs>(BREATH.subsec_nanos());

            Some(Timespec { tv_sec: seconds, tv_nsec: nanoseconds })
        }
        Door::Open | Door::Retrying => None,
    })
}

fn let_in(serving: &mut Serving, listening: &UnixListener, door: Door) -> Result<Door, Never> {
    loop {
        let fault = match listening.accept() {
            Ok((stream, _from)) => {
                let Ok(()) = arrived(serving, stream);

                continue;
            }
            Err(fault) => fault,
        };

        let Ok(refused) = refused(&fault);

        return Ok(match (refused, door) {
            (Rejected::NotYet, Door::Open | Door::Resting) => door,
            (Rejected::NotYet, Door::Retrying) => Door::Open,
            (Rejected::Interrupted, Door::Open | Door::Resting | Door::Retrying) => continue,
            (Rejected::Gone, Door::Open) => {
                eprintln!("console-events: no one can be let in: {fault}");

                Door::Resting
            }
            (Rejected::Gone, Door::Resting | Door::Retrying) => Door::Resting,
        });
    }
}

fn arrived(serving: &mut Serving, stream: UnixStream) -> Result<(), Never> {
    match stream.set_nonblocking(true) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("console-events: a program connected and could not be read: {fault}");

            return Ok(());
        }
    }

    let Ok(who) = serving.pool.joined();

    let _ = serving.clients.insert(who, Client { connection: stream, heard: Vec::new(), outbox: Vec::new() });

    Ok(())
}

fn heard(serving: &mut Serving, who: Who, flags: PollFlags) -> Result<(), Never> {
    match flags.intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR) {
        true => {},
        false => return Ok(()),
    }

    let Ok((lines, still)) = read(serving, who);

    for line in lines.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()) {
        let message = match std::str::from_utf8(line) {
            Ok(line) => wire::decoded(line),
            Err(_not_words) => Ok(None),
        };
        let Ok(message) = message;

        match message {
            Some(message) => {
                let Ok(()) = received(serving, who, message);
            }
            None => eprintln!("console-events: {who} said {:?}, which is nothing", String::from_utf8_lossy(line)),
        }
    }

    match still {
        Still::Connected => Ok(()),
        Still::Gone => let_go(serving, who),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Still {
    Connected,
    Gone,
}

fn read(serving: &mut Serving, who: Who) -> Result<(Vec<u8>, Still), Never> {
    let client = match serving.clients.get_mut(&who) {
        Some(client) => client,
        None => return Ok((Vec::new(), Still::Gone)),
    };
    let Ok(long) = index(READING);
    let mut buffer = vec![0_u8; long];

    let still = loop {
        let fault = match client.connection.read(&mut buffer) {
            Ok(0) => break Still::Gone,
            Ok(many) => {
                client.heard.extend(buffer.iter().take(many));

                continue;
            }
            Err(fault) => fault,
        };
        let Ok(refused) = refused(&fault);

        match refused {
            Rejected::NotYet => break Still::Connected,
            Rejected::Interrupted => {},
            Rejected::Gone => break Still::Gone,
        }
    };

    let rest = match client.heard.iter().rposition(|byte| *byte == b'\n') {
        Some(end) => client.heard.split_off(end.saturating_add(1)),
        None => return Ok((Vec::new(), still)),
    };

    Ok((std::mem::replace(&mut client.heard, rest), still))
}

fn let_go(serving: &mut Serving, who: Who) -> Result<(), Never> {
    let Ok(()) = serving.pool.left(who);
    let _ = serving.clients.remove(&who);

    Ok(())
}

fn received(serving: &mut Serving, who: Who, message: Message) -> Result<(), Never> {
    match message {
        Message::Subscribe(topic) => {
            let Ok(replay) = serving.pool.subscribe(who, topic.clone());
            let Ok(()) = hold(serving, &topic);

            match replay {
                Some(change) => {
                    let Ok(spelled) = wire::encoded(&Message::Publish(change));
                    let Ok(()) = send(serving, who, format!("{spelled}\n").as_bytes());
                }
                None => {},
            }
        }
        Message::Unsubscribe(topic) => {
            let Ok(()) = serving.pool.unsubscribe(who, &topic);
        }
        Message::Publish(_) => eprintln!("console-events: {who} tried to tell the pool something"),
    }

    Ok(())
}

fn hold(serving: &mut Serving, topic: &Topic) -> Result<(), Never> {
    match serving.held.contains(topic) {
        true => return Ok(()),
        false => {},
    }

    let Ok(held_now) = (serving.holding)(topic, serving.sources.clone());

    match held_now {
        Subscribed::Yes => serving.held.push(topic.clone()),
        Subscribed::No => {
            let Ok(token) = wire::token(topic);

            eprintln!(
                "console-events: nothing here watches {token}, so anyone listening to it will \
                 hear nothing"
            );
        }
    }

    Ok(())
}

fn told(serving: &mut Serving, published: &Receiver<Change>) -> Result<(), Never> {
    for change in published.try_iter() {
        let Ok(()) = publish(serving, &change);

        match serving.carried > BATCH {
            true => {
                let Ok(()) = written(serving);
            }
            false => {},
        }
    }

    Ok(())
}

fn publish(serving: &mut Serving, change: &Change) -> Result<(), Never> {
    let Ok(everyone) = serving.pool.publish(change);

    match everyone.first() {
        Some(_someone_asked_for_this) => {},
        None => return Ok(()),
    }

    let Ok(spelled) = wire::encoded(&Message::Publish(change.clone()));
    let line = format!("{spelled}\n");

    for who in everyone {
        let Ok(()) = send(serving, who, line.as_bytes());
    }

    Ok(())
}

fn send(serving: &mut Serving, who: Who, line: &[u8]) -> Result<(), Never> {
    let client = match serving.clients.get_mut(&who) {
        Some(client) => client,
        None => return Ok(()),
    };
    let Ok(held) = fitted::<_, u64>(client.outbox.len());

    match held > OUTBOX {
        true => {
            eprintln!(
                "console-events: {who} is holding {OUTBOX} bytes it has not read and is let go; \
                 it will connect again and be told what is true then"
            );

            return let_go(serving, who);
        }
        false => {},
    }

    client.outbox.extend_from_slice(line);

    let Ok(long) = fitted::<_, u64>(line.len());

    serving.carried = serving.carried.saturating_add(long);

    Ok(())
}

fn written(serving: &mut Serving) -> Result<(), Never> {
    serving.carried = 0;

    let mut gone: Vec<Who> = Vec::new();

    for (who, client) in &mut serving.clients {
        let Ok(wrote) = wrote(client);

        match wrote {
            Wrote::Retained => {},
            Wrote::Gone => gone.push(*who),
        }
    }

    for who in gone {
        let Ok(()) = let_go(serving, who);
    }

    Ok(())
}

enum Wrote {
    Retained,
    Gone,
}

fn wrote(client: &mut Client) -> Result<Wrote, Never> {
    loop {
        match client.outbox.is_empty() {
            true => return Ok(Wrote::Retained),
            false => {},
        }

        let fault = match client.connection.write(&client.outbox) {
            Ok(0) => return Ok(Wrote::Gone),
            Ok(many) => {
                let _ = client.outbox.drain(..many);

                continue;
            }
            Err(fault) => fault,
        };
        let Ok(refused) = refused(&fault);

        match refused {
            Rejected::NotYet => return Ok(Wrote::Retained),
            Rejected::Interrupted => {},
            Rejected::Gone => return Ok(Wrote::Gone),
        }
    }
}

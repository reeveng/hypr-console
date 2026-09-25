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
//! ## A door that cannot open is not a door that is quiet
//!
//! The thread that lets programs in used to read `incoming().flatten()`, which
//! is an iterator that throws every refused `accept` away and asks again. What
//! that costs depends on why the call failed, and there is one reason it never
//! stops failing: `accept` takes the descriptor it is going to need before it
//! does anything else, so a process with none left is refused with `EMFILE`
//! before the queue is even looked at. Nothing about asking again changes that
//! answer, and the loop turns as fast as the machine will turn it, silently, in
//! a thread no one is watching.
//!
//! So the refusal is waited on rather than dropped. [`BREATH`] is long enough
//! that a door which cannot open costs nothing and short enough that a program
//! waiting to be let in does not notification, and the wait is the whole of it:
//! descriptors come back, the connection that was queued is taken, and the pool
//! carries on. Saying so once rather than every turn is the other half, because
//! a fault that repeats faster than it can be read is a fault no one reads.
//!
//! There is no test here, and the reason is worth writing down. The refusal
//! needs a full descriptor table at the moment this thread asks again, and a
//! thread already waiting inside `accept` is holding the descriptor it reserved
//! on the way in -- so it takes the next connection however little is left, and
//! the drought a test creates around it is one it cannot feel. Pressing it
//! wants the client in another process, because a test that opens the socket
//! itself is spending the same table it is trying to empty.
//! ## One program that stops reading is not everyone's silence
//!
//! Everyone is told from this one loop, and a write into a socket whose reader
//! has gone to sleep blocks the moment the kernel's buffer for it is full. That
//! is not the slow program's problem, it is everyone's: the thread that would
//! have told the others is inside that write. On a desktop it reads as the
//! volume freezing on the bar because a panel behind a picker stopped reading,
//! which is a fault in the last place anyone would look for it.
//!
//! So the loop never writes to a socket. Each connection has a thread of its
//! own and a bounded queue in front of it, the loop hands a line over with
//! `try_send` and goes back to what it was doing, and the slowest subscriber
//! on the machine costs everyone else nothing.
//!
//! What is bounded is the bytes a program has asked for and not read, rather
//! than a count of words: a burst of short lines is nothing to hold and a
//! program that has stopped reading is megabytes within seconds, and the second
//! is the one worth ending. Bounding the count instead let a burst end healthy
//! subscriptions -- a hundred thousand words handed over at once dropped every
//! program on the machine and had them all reconnecting a second later, which
//! measured as a fortieth of the throughput and half the words lost.
//!
//! **A full queue ends the connection rather than dropping the word.** These
//! words are what is true now, so a program that missed some of them holds a
//! picture that is wrong and has no way to find out -- dropping a line is
//! silent and permanent. Being let go is the recoverable one, and every piece
//! of that is already here: `console-core-reconnect` brings the program back,
//! `Client` asks for its topics again, the pool replays the last word on
//! each, and `Heard::GotIn` tells it there was a gap. It comes back knowing
//! what is true instead of carrying on with what it missed.
//!
//! **A burst is handed over in pieces rather than a line at a time.** The
//! writing thread drains faster than this loop fills, so it is asleep whenever
//! it is keeping up, and a line sent on its own wakes it -- which is a syscall
//! per line per program, and it was the whole of what this cost under load.
//! So words that are already waiting are gathered as they are read, spelled
//! once each, and appended to a buffer per program; the buffers are handed over
//! when nothing else is waiting or when [`BATCH`] has gathered, whichever comes
//! first. One word alone is still one hand-over and one write, so nothing about
//! a quiet desktop changes; a thousand words in a burst is a handful of
//! wake-ups instead of a thousand, and the writing thread turns each buffer
//! into one `write_all`.
//!
//! [`BATCH`] is a ceiling on how much is gathered before any of it moves, and
//! it exists because a burst gathered whole is a burst that starts arriving
//! only when it has finished. It bounds the wait as well as the memory.
//!
//! Which is why letting go is a shutdown and not just a dropped queue. The
//! thread reading from that program holds the same socket, so closing the
//! writing end alone leaves it connected, subscribed and silent, which is the
//! one state nothing recovers from.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_program_contract::{Change, Topic};
use console_program_lifetime::threads;

use crate::Unserved;
use crate::pool::{Pool, Who};
use crate::sources::Subscribed;
use crate::wire::{self, Message};

const BREATH: Duration = Duration::from_millis(200);

pub const OUTBOX: u64 = 4 << 20;

pub const BATCH: u64 = 64 << 10;

struct Client {
    lines: Sender<String>,
    waiting: Arc<AtomicU64>,
    connection: UnixStream,
}

pub type Holding = fn(&Topic, Sender<Change>) -> Result<Subscribed, Never>;

enum ServerEvent {
    Connected(UnixStream),
    Received(Who, Message),
    Disconnected(Who),
    Published(Change),
}

pub fn serve(socket: &Path, holding: Holding) -> Result<(), Unserved> {

    let at = match socket.parent() {
        Some(at) => at,
        None => return Err(Unserved::Rootless),
    };
    let made = std::fs::create_dir_all(at);

    made.map_err(|fault| Unserved::Holding(at.to_path_buf(), fault))?;

    let deleted = std::fs::remove_file(socket);

    match deleted {
        Ok(()) => {},
        Err(_) => {},
    }

    let listening = UnixListener::bind(socket)
        .map_err(|fault| Unserved::Unbound(socket.to_path_buf(), fault))?;
    let (say, happened) = channel();
    let arriving = say.clone();

    let Ok(()) = threads::let_go(std::thread::spawn(move || {
        let mut quiet = false;

        for coming in listening.incoming() {
            let stream = match coming {
                Ok(stream) => {
                    quiet = false;

                    stream
                }
                Err(fault) => {
                    match quiet {
                        true => {},
                        false => {
                            eprintln!("console-events: no one can be let in: {fault}");

                            quiet = true;
                        }
                    }

                    #[cfg_attr(
                        dylint_lib = "explicit021_no_sleeping",
                        allow(
                            explicit021_no_sleeping,
                            reason = "no one could be let in and nothing announces when that stops being true; without a gap a socket that is refusing spins this thread against the kernel"
                        )
                    )]
                    std::thread::sleep(BREATH);

                    continue;
                }
            };

            let sent = arriving.send(ServerEvent::Connected(stream));

            match sent {
                Ok(()) => {},
                Err(_) => return,
            }
        }
    }));

    let mut pool = Pool::default();
    let mut writing: BTreeMap<Who, Client> = BTreeMap::new();
    let mut held: Vec<Topic> = Vec::new();

    let mut carrying: BTreeMap<Who, String> = BTreeMap::new();
    let mut carried: u64 = 0;

    loop {
        let mut word = match happened.recv() {
            Ok(word) => Some(word),
            Err(_nothing_will_say_anything_again) => return Ok(()),
        };

        'over_words: loop {
            let now = match word.take() {
                Some(now) => now,
                None => break 'over_words,
            };

            match now {
                ServerEvent::Published(change) => {
                    let Ok(()) = published(&mut pool, &mut carrying, &mut carried, &change);
                }
                ServerEvent::Connected(stream) => {
                    let Ok(()) = handed(&mut pool, &mut writing, &mut carrying, &mut carried);
                    let Ok(()) = arrived(&mut pool, &mut writing, stream, &say);
                }
                ServerEvent::Received(who, message) => {
                    let Ok(()) = handed(&mut pool, &mut writing, &mut carrying, &mut carried);
                    let Ok(()) =
                        received(&mut pool, &mut writing, &mut held, who, message, &say, holding);
                }
                ServerEvent::Disconnected(who) => {
                    let Ok(()) = handed(&mut pool, &mut writing, &mut carrying, &mut carried);
                    let Ok(()) = let_go(&mut pool, &mut writing, who);
                }
            }

            match carried > BATCH {
                true => {
                    let Ok(()) = handed(&mut pool, &mut writing, &mut carrying, &mut carried);
                }
                false => {},
            }

            word = match happened.try_recv() {
                Ok(more) => Some(more),
                Err(_nothing_else_is_waiting) => None,
            };
        }

        let Ok(()) = handed(&mut pool, &mut writing, &mut carrying, &mut carried);
    }
}

fn arrived(
    pool: &mut Pool,
    writing: &mut BTreeMap<Who, Client>,
    stream: UnixStream,
    say: &Sender<ServerEvent>,
) -> Result<(), Never> {
    let reading = match stream.try_clone() {
        Ok(reading) => reading,
        Err(fault) => {
            eprintln!("console-events: a program connected and could not be read: {fault}");

            return Ok(());
        }
    };

    let holding = match stream.try_clone() {
        Ok(holding) => holding,
        Err(fault) => {
            eprintln!("console-events: a program connected and could not be told anything: {fault}");

            return Ok(());
        }
    };

    let Ok(who) = pool.joined();
    let Ok((lines, waiting)) = writer_thread(who, stream, say);

    let _ = writing.insert(who, Client { lines, waiting, connection: holding });

    let sender = say.clone();

    let Ok(()) = threads::let_go(std::thread::spawn(move || {
        for line in BufReader::new(reading).lines().map_while(Result::ok) {
            let Ok(message) = wire::decoded(&line);

            match message {
                Some(message) => {
                    let sent = sender.send(ServerEvent::Received(who, message));

                    match sent {
                        Ok(()) => {},
                        Err(_) => return,
                    }
                }
                None => eprintln!("console-events: {who} said {line:?}, which is nothing"),
            }
        }

        let _ = sender.send(ServerEvent::Disconnected(who));
    }));

    Ok(())
}

fn writer_thread(
    who: Who,
    mut stream: UnixStream,
    say: &Sender<ServerEvent>,
) -> Result<(Sender<String>, Arc<AtomicU64>), Never> {
    let (lines, arriving) = channel::<String>();
    let waiting = Arc::new(AtomicU64::new(0));
    let counting = Arc::clone(&waiting);
    let sender = say.clone();

    let Ok(()) = threads::let_go(std::thread::spawn(move || {
        let mut carrying = String::new();

        while let Ok(line) = arriving.recv() {
            carrying.push_str(&line);

            let Ok(mut held) = fitted::<_, u64>(line.len());

            for more in arriving.try_iter() {
                carrying.push_str(&more);

                let Ok(long) = fitted::<_, u64>(more.len());

                held = held.saturating_add(long);
            }

            let written = stream.write_all(carrying.as_bytes());

            carrying.clear();
            let _ = counting.fetch_sub(held, Ordering::Relaxed);

            match written {
                Ok(()) => {},
                Err(_the_program_has_gone) => {
                    let _ = sender.send(ServerEvent::Disconnected(who));

                    return;
                }
            }
        }
    }));

    Ok((lines, waiting))
}

fn let_go(
    pool: &mut Pool,
    writing: &mut BTreeMap<Who, Client>,
    who: Who,
) -> Result<(), Never> {
    let Ok(()) = pool.left(who);

    match writing.remove(&who) {
        Some(client) => {
            let _ = client.connection.shutdown(Shutdown::Both);
        }
        None => {},
    }

    Ok(())
}

fn received(
    pool: &mut Pool,
    writing: &mut BTreeMap<Who, Client>,
    held: &mut Vec<Topic>,
    who: Who,
    message: Message,
    say: &Sender<ServerEvent>,
    holding: Holding,
) -> Result<(), Never> {
    match message {
        Message::Subscribe(topic) => {
            let Ok(replay) = pool.subscribe(who, topic.clone());
            let Ok(()) = hold(held, &topic, say, holding);

            match replay {
                Some(change) => {
                    let Ok(spelled) = wire::encoded(&Message::Publish(change));
                    let Ok(()) = send(pool, writing, who, format!("{spelled}\n"));
                }
                None => {},
            }
        }
        Message::Unsubscribe(topic) => {
            let Ok(()) = pool.unsubscribe(who, &topic);
        }
        Message::Publish(_) => eprintln!("console-events: {who} tried to tell the pool something"),
    }

    Ok(())
}

fn hold(
    held: &mut Vec<Topic>,
    topic: &Topic,
    say: &Sender<ServerEvent>,
    holding: Holding,
) -> Result<(), Never> {
    match held.contains(topic) {
        true => return Ok(()),
        false => {},
    }

    let (sender_to_pool, arriving) = channel();
    let sender = say.clone();

    let Ok(held_now) = holding(topic, sender_to_pool);

    match held_now {
        Subscribed::Yes => {
            held.push(topic.clone());

            let Ok(()) = threads::let_go(std::thread::spawn(move || {
                for change in arriving {
                    let sent = sender.send(ServerEvent::Published(change));

                    match sent {
                        Ok(()) => {},
                        Err(_) => return,
                    }
                }
            }));
        }
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

fn published(
    pool: &mut Pool,
    carrying: &mut BTreeMap<Who, String>,
    carried: &mut u64,
    change: &Change,
) -> Result<(), Never> {
    let Ok(everyone) = pool.publish(change);

    match everyone.first() {
        Some(_someone_asked_for_this) => {},
        None => return Ok(()),
    }

    let Ok(spelled) = wire::encoded(&Message::Publish(change.clone()));

    for who in everyone {
        let held = carrying.entry(who).or_default();

        held.push_str(&spelled);
        held.push('\n');

        let Ok(long) = fitted::<_, u64>(spelled.len());

        *carried = carried.saturating_add(long.saturating_add(1));
    }

    Ok(())
}

fn handed(
    pool: &mut Pool,
    writing: &mut BTreeMap<Who, Client>,
    carrying: &mut BTreeMap<Who, String>,
    carried: &mut u64,
) -> Result<(), Never> {
    *carried = 0;

    for (who, lines) in std::mem::take(carrying) {
        let Ok(()) = send(pool, writing, who, lines);
    }

    Ok(())
}

fn send(
    pool: &mut Pool,
    writing: &mut BTreeMap<Who, Client>,
    who: Who,
    lines: String,
) -> Result<(), Never> {
    let sent = match writing.get(&who) {
        Some(client) => {
            let held = client.waiting.load(Ordering::Relaxed);

            match held > OUTBOX {
                true => Behind::TooFar,
                false => {
                    let Ok(long) = fitted::<_, u64>(lines.len());
                    let _ = client.waiting.fetch_add(long, Ordering::Relaxed);

                    match client.lines.send(lines) {
                        Ok(()) => Behind::No,
                        Err(_no_one_is_writing_for_it) => Behind::Closed,
                    }
                }
            }
        }
        None => return Ok(()),
    };

    match sent {
        Behind::No => {},
        Behind::TooFar => {
            eprintln!(
                "console-events: {who} is holding {OUTBOX} bytes it has not read and is let go; \
                 it will connect again and be told what is true then"
            );

            let Ok(()) = let_go(pool, writing, who);
        }
        Behind::Closed => {
            let Ok(()) = let_go(pool, writing, who);
        }
    }

    Ok(())
}

enum Behind {
    No,
    TooFar,
    Closed,
}

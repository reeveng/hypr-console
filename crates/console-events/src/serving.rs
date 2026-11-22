//! The pool over a socket: who is connected, what they asked for, and what to
//! do when a source says something.  Split from the program so that it can be
//! started on a socket of a test's own, with sources of a test's own. What that
//! buys is the one thing the unit tests in `pool` cannot ask: that a program
//! which asks for an event group over a real socket is told the last word on it
//! *before* anything changes, which is the whole reason the pool remembers
//! anything.  Where the words come from is handed in, the same way
//! `console_input_controller::turning` is handed a machine. There is no other
//! way to ask what the pool does about a compositor that says something,
//! because the only compositor is the one this is running under.
//!
//! ## One thread, asleep in `poll`
//!
//! This used to be a thread per program to read it, a thread per program to
//! write to it, a thread per event group to carry a source's words across, and
//! one for the door: two threads a subscriber, most of them asleep in a
//! blocking call, and every word crossing a channel twice before it reached a
//! socket. The pool was always decided on one thread. Now it is heard and told
//! on that thread as well, which waits in one `poll` over the door, every
//! connection, and a pipe that a source's word wakes it through.
//!
//! What sources say still arrives on a sync_channel, because a source is a thread of
//! its own that this does not own and `Holding` hands it a `SyncSender`. One thread
//! carries every event group's words across and writes a byte down the pipe,
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
//! `Client` asks for its event groups again, the pool replays the last word on
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
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::Duration;

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_program_contract::{Change, EventGroup};
use console_program_lifetime::threads;
use console_waiting::latch::{self, Latch};
use rustix::event::{Nsecs, PollFd, PollFlags, Secs, Timespec, poll};

use crate::Unserved;
use crate::pool::{Pool, Who};
use crate::sources::Subscribed;
use crate::wire::{self, Message};

const BREATH: Duration = Duration::from_millis(200);

pub const OUTBOX: u64 = 4 << 20;

pub const BATCH: u64 = 64 << 10;

struct Client {
  connection: UnixStream,
  heard: Vec<u8>,
  outbox: Vec<u8>,
}

pub type Holding = fn(&EventGroup, SyncSender<Change>) -> Result<Subscribed, Never>;

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

  Ok(
    match (
      kind == ErrorKind::WouldBlock,
      kind == ErrorKind::Interrupted,
    ) {
      (true, true) | (true, false) => Rejected::NotYet,
      (false, true) => Rejected::Interrupted,
      (false, false) => Rejected::Gone,
    },
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Watched {
  Latch,
  Door,
  Client(Who),
}

struct Serving<'source> {
  pool: Pool,
  clients: BTreeMap<Who, Client>,
  held: Vec<EventGroup>,
  sources: SyncSender<Change>,
  holding: &'source dyn Fn(&EventGroup, SyncSender<Change>) -> Result<Subscribed, Never>,
  carried: u64,
}

pub fn serve(
  socket: &Path,
  holding: impl Fn(&EventGroup, SyncSender<Change>) -> Result<Subscribed, Never>,
) -> Result<(), Unserved> {
  let listening = bound(socket)?;
  let waking = latch::pipe().map_err(Unserved::Waking)?;
  let Latch { waiting, saying } = waking;
  let (sources, arriving) = sync_channel::<Change>(256);
  let (carrying, published) = sync_channel::<Change>(256);

  let Ok(()) = threads::let_go(std::thread::spawn(move || {
    let Ok(()) = relay_sources(arriving, carrying, saying);
  }));

  let serving = Serving {
    pool: Pool::default(),
    clients: BTreeMap::new(),
    held: Vec::new(),
    sources,
    holding: &holding,
    carried: 0,
  };

  let served = iterate((serving, Door::Open), |(mut serving_2, door)| {
    let ready = match ready(&serving_2, &listening, &waiting, door) {
      Ok(ready) => ready,
      Err(fault) => return Ok(Step::Halt(Err(fault))),
    };

    let mut door_2 = match door {
      Door::Resting => Door::Retrying,
      Door::Open | Door::Retrying => door,
    };

    for (watched, flags) in ready {
      match watched {
        Watched::Latch => {
          let Ok(()) = latch::drain(&waiting);
          let Ok(()) = publish_pending(&mut serving_2, &published);

          match ::core::primitive::bool::then_some(flags.intersects(PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL), ()) {
            ::core::option::Option::Some(()) => {
              let fault = std::io::Error::from(ErrorKind::BrokenPipe);

              return Ok(Step::Halt(Err(Unserved::Waking(fault))));
            }
            ::core::option::Option::None => {}
          }
        }
        Watched::Door => {
          let Ok(let_in) = let_in(&mut serving_2, &listening, door_2);

          door_2 = let_in;
        }
        Watched::Client(who) => {
          let Ok(()) = read_client(&mut serving_2, who, flags);
        }
      }
    }

    let Ok(()) = flush(&mut serving_2);

    Ok(Step::Again((serving_2, door_2)))
  });

  match served {
    Ok(served_2) => served_2,
    Err(Endless) => Ok(()),
  }
}

#[cfg_attr(
  dylint_lib = "explicit055_no_unbounded_loop",
  expect(
    explicit055_no_unbounded_loop,
    reason = "the daemon stores the source sender and its receiver ends when the daemon drops it; a failed forward or wake ends the relay too"
  )
)]
fn relay_sources(
  arriving: Receiver<Change>,
  carrying: SyncSender<Change>,
  mut saying: std::fs::File,
) -> Result<(), Never> {
  for change in arriving {
    match carrying.send(change) {
      Ok(()) => {}
      Err(_no_one_is_listening) => return Ok(()),
    }

    match saying.write_all(&[1]) {
      Ok(()) => {}
      Err(fault) => match ::core::primitive::bool::then_some(fault.kind() == ErrorKind::WouldBlock, ()) {
        ::core::option::Option::Some(()) => continue,
        ::core::option::Option::None => {
          console_core_logging::warn!("console-events: could not wake the pool: {fault}");

          return Ok(());
        }
      },
    }
  }

  Ok(())
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
    Ok(()) => {}
    Err(fault) => match ::core::primitive::bool::then_some(fault.kind() == ErrorKind::NotFound, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(Unserved::Unbound(socket.to_path_buf(), fault)),
    },
  }

  let listening =
    UnixListener::bind(socket).map_err(|fault| Unserved::Unbound(socket.to_path_buf(), fault))?;
  let nonblocking = listening.set_nonblocking(true);

  nonblocking.map_err(|fault| Unserved::Unbound(socket.to_path_buf(), fault))?;

  Ok(listening)
}

fn ready(
  serving: &Serving<'_>,
  listening: &UnixListener,
  waiting: &std::os::fd::OwnedFd,
  door: Door,
) -> Result<Vec<(Watched, PollFlags)>, Unserved> {
  let mut watch = vec![PollFd::new(waiting, PollFlags::IN)];
  let mut which = vec![Watched::Latch];

  match door {
    Door::Open | Door::Retrying => {
      watch.push(PollFd::new(listening, PollFlags::IN));
      which.push(Watched::Door);
    }
    Door::Resting => {}
  }

  for (who, client) in &serving.clients {
    let flags = match ::core::primitive::bool::then_some(client.outbox.is_empty(), ()) {
      ::core::option::Option::Some(()) => PollFlags::IN,
      ::core::option::Option::None => PollFlags::IN | PollFlags::OUT,
    };

    watch.push(PollFd::from_borrowed_fd(client.connection.as_fd(), flags));
    which.push(Watched::Client(*who));
  }

  let Ok(breath) = breath(door);

  match poll(&mut watch, breath.as_ref()) {
    Ok(_) => {}
    Err(rustix::io::Errno::INTR) => return Ok(Vec::new()),
    Err(fault) => return Err(Unserved::Waiting(fault)),
  }

  Ok(
    which
      .into_iter()
      .zip(watch.iter().map(|fd| fd.revents()))
      .filter(|(_, flags)| !flags.is_empty())
      .collect(),
  )
}

fn breath(door: Door) -> Result<Option<Timespec>, Never> {
  Ok(match door {
    Door::Resting => {
      let Ok(seconds) = fitted::<u64, Secs>(BREATH.as_secs());
      let Ok(nanoseconds) = fitted::<u32, Nsecs>(BREATH.subsec_nanos());

      Some(Timespec {
        tv_sec: seconds,
        tv_nsec: nanoseconds,
      })
    }
    Door::Open | Door::Retrying => None,
  })
}

fn let_in(
  serving: &mut Serving<'_>,
  listening: &UnixListener,
  door: Door,
) -> Result<Door, Never> {
  for accepted in listening.incoming() {
    let fault = match accepted {
      Ok(stream) => {
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

  Ok(door)
}

fn arrived(serving: &mut Serving<'_>, stream: UnixStream) -> Result<(), Never> {
  match stream.set_nonblocking(true) {
    Ok(()) => {}
    Err(fault) => {
      eprintln!("console-events: a program connected and could not be read: {fault}");

      return Ok(());
    }
  }

  let Ok(who) = serving.pool.joined();

  let _ = serving.clients.insert(
    who,
    Client {
      connection: stream,
      heard: Vec::new(),
      outbox: Vec::new(),
    },
  );

  Ok(())
}

fn read_client(serving: &mut Serving<'_>, who: Who, flags: PollFlags) -> Result<(), Never> {
  match ::core::primitive::bool::then_some(flags.intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(()),
  }

  let Ok((lines, still)) = read(serving, who);

  for bytes in lines
    .split(|byte| *byte == b'\n')
    .filter(|line| !line.is_empty())
  {
    let line = match std::str::from_utf8(bytes) {
      Ok(line) => line,
      Err(fault) => {
        console_core_logging::warn!("console-events: {who} sent invalid UTF-8: {fault}");

        continue;
      }
    };
    let Ok(message) = wire::decoded(line);

    match message {
      Some(message_2) => {
        let Ok(()) = received(serving, who, message_2);
      }
      None => {
        console_core_logging::warn!("console-events: {who} said {line:?}, which is nothing")
      }
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

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "arrived sets every client socket nonblocking before storing it, and this read stops when it would block"
  )
)]
fn read(serving: &mut Serving<'_>, who: Who) -> Result<(Vec<u8>, Still), Never> {
  let client = match serving.clients.get_mut(&who) {
    Some(client) => client,
    None => return Ok((Vec::new(), Still::Gone)),
  };
  let buffer = Box::new([0_u8; 64 << 10]);

  let heard = iterate((client, buffer), |(client_2, mut buffer_2)| {
    let fault = match client_2.connection.read(buffer_2.as_mut_slice()) {
      Ok(0) => return Ok(Step::Halt((client_2, Still::Gone))),
      Ok(many) => {
        client_2.heard.extend(buffer_2.iter().take(many));

        return Ok(Step::Again((client_2, buffer_2)));
      }
      Err(fault) => fault,
    };
    let Ok(refused) = refused(&fault);

    Ok(match refused {
      Rejected::NotYet => Step::Halt((client_2, Still::Connected)),
      Rejected::Interrupted => Step::Again((client_2, buffer_2)),
      Rejected::Gone => Step::Halt((client_2, Still::Gone)),
    })
  });

  let (client_3, still) = match heard {
    Ok(heard_2) => heard_2,
    Err(Endless) => return Ok((Vec::new(), Still::Gone)),
  };

  let rest = match client_3.heard.iter().rposition(|byte| *byte == b'\n') {
    Some(end) => client_3.heard.split_off(end.saturating_add(1)),
    None => return Ok((Vec::new(), still)),
  };

  Ok((std::mem::replace(&mut client_3.heard, rest), still))
}

fn let_go(serving: &mut Serving<'_>, who: Who) -> Result<(), Never> {
  let Ok(()) = serving.pool.remove(who);
  let _ = serving.clients.remove(&who);

  Ok(())
}

fn received(serving: &mut Serving<'_>, who: Who, message: Message) -> Result<(), Never> {
  match message {
    Message::Subscribe(event_group) => {
      let Ok(replay) = serving.pool.subscribe(who, event_group.clone());
      let Ok(()) = hold(serving, &event_group);

      match replay {
        Some(change) => {
          let Ok(spelled) = wire::encoded(&Message::Publish(change));
          let Ok(()) = send(serving, who, format!("{spelled}\n").as_bytes());
        }
        None => {}
      }
    }
    Message::Unsubscribe(event_group) => {
      let Ok(()) = serving.pool.unsubscribe(who, &event_group);
    }
    Message::Publish(_) => eprintln!("console-events: {who} tried to tell the pool something"),
  }

  Ok(())
}

fn hold(serving: &mut Serving<'_>, event_group: &EventGroup) -> Result<(), Never> {
  match ::core::primitive::bool::then_some(serving.held.contains(event_group), ()) {
    ::core::option::Option::Some(()) => return Ok(()),
    ::core::option::Option::None => {}
  }

  let Ok(held_now) = (serving.holding)(event_group, serving.sources.clone());

  match held_now {
    Subscribed::Yes => serving.held.push(event_group.clone()),
    Subscribed::No => {
      let Ok(token) = wire::token(event_group);

      eprintln!(
        "console-events: nothing here watches {token}, so anyone listening to it will \
                 hear nothing"
      );
    }
  }

  Ok(())
}

fn publish_pending(serving: &mut Serving<'_>, published: &Receiver<Change>) -> Result<(), Never> {
  for change in published.try_iter() {
    let Ok(()) = publish(serving, &change);

    match ::core::primitive::bool::then_some(serving.carried > BATCH, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = flush(serving);
      }
      ::core::option::Option::None => {}
    }
  }

  Ok(())
}

fn publish(serving: &mut Serving<'_>, change: &Change) -> Result<(), Never> {
  let Ok(everyone) = serving.pool.publish(change);

  match everyone.first() {
    Some(_someone_asked_for_this) => {}
    None => return Ok(()),
  }

  let Ok(spelled) = wire::encoded(&Message::Publish(change.clone()));
  let line = format!("{spelled}\n");

  for who in everyone {
    let Ok(()) = send(serving, who, line.as_bytes());
  }

  Ok(())
}

fn send(serving: &mut Serving<'_>, who: Who, line: &[u8]) -> Result<(), Never> {
  let client = match serving.clients.get_mut(&who) {
    Some(client) => client,
    None => return Ok(()),
  };
  let Ok(held) = fitted::<_, u64>(client.outbox.len());

  match ::core::primitive::bool::then_some(held > OUTBOX, ()) {
    ::core::option::Option::Some(()) => {
      eprintln!(
        "console-events: {who} is holding {OUTBOX} bytes it has not read and is let go; \
                 it will connect again and be told what is true then"
      );

      return let_go(serving, who);
    }
    ::core::option::Option::None => {}
  }

  client.outbox.extend_from_slice(line);

  let Ok(long) = fitted::<_, u64>(line.len());

  serving.carried = serving.carried.saturating_add(long);

  Ok(())
}

fn flush(serving: &mut Serving<'_>) -> Result<(), Never> {
  serving.carried = 0;

  let mut gone: Vec<Who> = Vec::new();

  for (who, client) in &mut serving.clients {
    let Ok(wrote) = wrote(client);

    match wrote {
      Wrote::Retained => {}
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
  let written = iterate(client, |client_2| {
    match ::core::primitive::bool::then_some(client_2.outbox.is_empty(), ()) {
      ::core::option::Option::Some(()) => return Ok(Step::Halt(Wrote::Retained)),
      ::core::option::Option::None => {}
    }

    let fault = match client_2.connection.write(&client_2.outbox) {
      Ok(0) => return Ok(Step::Halt(Wrote::Gone)),
      Ok(many) => {
        let _ = client_2.outbox.drain(..many);

        return Ok(Step::Again(client_2));
      }
      Err(fault) => fault,
    };
    let Ok(refused) = refused(&fault);

    Ok(match refused {
      Rejected::NotYet => Step::Halt(Wrote::Retained),
      Rejected::Interrupted => Step::Again(client_2),
      Rejected::Gone => Step::Halt(Wrote::Gone),
    })
  });

  Ok(match written {
    Ok(written_2) => written_2,
    Err(Endless) => Wrote::Gone,
  })
}

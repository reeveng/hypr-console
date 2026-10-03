//! A machine on its own thread, pressed from several: what arrives, what comes
//! back to whom, what a fall costs, and what is still there after a restart.

use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

use console_actor::{BootError, Closed, KeyValueStore, SendError, boot, boot_serializable};
use console_core_state_machine::{
    Decoder, Encoder, Machine, Never, ParseError, Queue, Serializable, SerializableMachine, Version, restore,
};
use console_core_temporary_directories::fresh;
use console_waiting::{Schedule, until_some};

type Failure = Box<dyn std::error::Error>;

const PATIENCE: Duration = Duration::from_secs(5);
const KEY: &str = "walk";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Depth(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Request {
    Down,
    Up,
    Fall,
    Echo(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    StandingAt(Depth),
    Echoed(u64),
}

impl Serializable for Depth {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
        self.0.encode(encoder)
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
        u64::decode(decoder).map(Depth)
    }
}

struct Walk;

#[cfg_attr(dylint_lib = "explicit004_no_panic", allow(explicit004_no_panic, reason = "a machine that falls is what the actor's restart is for, and a panic is the only thing that presses it"))]
fn fall() -> ! {
    panic!("the machine fell where it was told to")
}

impl Machine for Walk {
    type Input = Depth;
    type State = Depth;
    type Request = Request;
    type Effect = Effect;

    fn initialize(input: &Depth, previous: Option<Depth>, effects: &mut Queue<Effect>) -> Result<Depth, Never> {
        let start = match previous {
            Some(kept) => kept,
            None => *input,
        };
        let Ok(()) = effects.offer(Effect::StandingAt(start));

        Ok(start)
    }

    fn handle(state: Depth, request: Request, effects: &mut Queue<Effect>) -> Result<Depth, Never> {
        match request {
            Request::Down => Ok(Depth(state.0.saturating_add(1))),
            Request::Up => Ok(Depth(state.0.saturating_sub(1))),
            Request::Fall => fall(),
            Request::Echo(said) => {
                let Ok(()) = effects.offer(Effect::Echoed(said));

                Ok(state)
            }
        }
    }
}

impl SerializableMachine for Walk {
    const VERSION: Version = Version(1);
}

fn within<T: Send + 'static>(patience: Duration, doing: impl FnOnce() -> T + Send + 'static) -> Result<T, RecvTimeoutError> {
    let (said, hear) = channel();
    let asking = std::thread::spawn(move || {
        let _ = said.send(doing());
    });
    let Ok(()) = console_program_lifetime::threads::let_go(asking);

    hear.recv_timeout(patience)
}

fn kept(store: &KeyValueStore, wanted: Depth) -> Result<Option<Depth>, Never> {
    Ok(match store.get(KEY) {
        Ok(Some(bytes)) => match restore::<Walk>(&bytes) {
            Ok(found) => Some(found).filter(|found| *found == wanted),
            Err(_not_yet_a_snapshot) => None,
        },
        Ok(None) => None,
        Err(_unreadable) => None,
    })
}

#[test]
fn it_holds_a_state_without_a_lock() -> Result<(), Failure> {
    let walk = boot::<Walk>(Depth(0))?;

    walk.actor.send(Request::Down)?;
    walk.actor.send(Request::Down)?;
    walk.actor.send(Request::Up)?;

    assert_eq!(walk.actor.get(), Ok(Depth(1)));

    let Ok(()) = walk.scope.close();

    Ok(())
}

#[test]
fn every_request_from_every_thread_arrives() -> Result<(), Failure> {
    let walk = boot::<Walk>(Depth(0))?;

    std::thread::scope(|senders| {
        for _ in 0..8 {
            let actor = walk.actor.clone();

            senders.spawn(move || {
                for _ in 0..1000 {
                    let _ = actor.send(Request::Down);
                }
            });
        }
    });

    assert_eq!(walk.actor.get(), Ok(Depth(8000)));

    let Ok(()) = walk.scope.close();

    Ok(())
}

#[test]
fn what_a_request_decided_goes_back_to_whoever_sent_it() -> Result<(), Failure> {
    let walk = boot::<Walk>(Depth(0))?;

    assert_eq!(walk.effects, vec![Effect::StandingAt(Depth(0))], "what initializing decided was not handed back");

    std::thread::scope(|senders| {
        for who in 0..16_u64 {
            let actor = walk.actor.clone();

            senders.spawn(move || {
                for round in 0..64_u64 {
                    let mine = who.saturating_mul(1000).saturating_add(round);

                    assert_eq!(actor.send(Request::Echo(mine)), Ok(vec![Effect::Echoed(mine)]), "an effect went to the wrong sender");
                }
            });
        }
    });

    let Ok(()) = walk.scope.close();

    Ok(())
}

#[test]
fn a_request_the_machine_falls_under_is_told_rather_than_left_waiting() -> Result<(), Failure> {
    let walk = boot::<Walk>(Depth(3))?;

    walk.actor.send(Request::Down)?;
    walk.actor.send(Request::Down)?;

    let actor = walk.actor.clone();
    let said = within(PATIENCE, move || actor.send(Request::Fall))?;

    assert_eq!(said, Err(SendError::MachineDefect(vec![Effect::StandingAt(Depth(3))])), "the sender of a fall was not told the machine started again");
    assert_eq!(walk.actor.get(), Ok(Depth(3)), "the machine did not start over after the fall");

    let Ok(()) = walk.scope.close();

    Ok(())
}

#[test]
fn what_was_sent_behind_a_fall_still_arrives() -> Result<(), Failure> {
    let walk = boot::<Walk>(Depth(0))?;

    walk.actor.send(Request::Down)?;

    let _ = walk.actor.send(Request::Fall);

    for _ in 0..7 {
        walk.actor.send(Request::Down)?;
    }

    assert_eq!(walk.actor.get(), Ok(Depth(7)), "the seven sent after the fall did not all arrive");

    let Ok(()) = walk.scope.close();

    Ok(())
}

#[test]
fn an_ended_actor_says_so_to_every_handle() -> Result<(), Failure> {
    let walk = boot::<Walk>(Depth(0))?;
    let one = walk.actor.clone();
    let other = walk.actor.clone();
    let Ok(()) = walk.scope.close();

    let sent = within(PATIENCE, move || one.send(Request::Down))?;
    let read = within(PATIENCE, move || other.get())?;

    assert_eq!(sent, Err(SendError::Closed));
    assert_eq!(read, Err(Closed));

    Ok(())
}

#[test]
fn a_kept_machine_comes_back_where_it_was() -> Result<(), Failure> {
    let directory = fresh("actor-comes-back")?;
    let Ok(store) = KeyValueStore::file_system(directory);
    let walk = boot_serializable::<Walk>(Depth(0), store.clone(), KEY)?;

    for _ in 0..5 {
        walk.actor.send(Request::Down)?;
    }

    let Ok(()) = walk.scope.close();
    let again = boot_serializable::<Walk>(Depth(0), store, KEY)?;

    assert_eq!(again.actor.get(), Ok(Depth(5)));
    assert_eq!(again.effects, vec![Effect::StandingAt(Depth(5))], "initializing was not handed the state it had");

    let Ok(()) = again.scope.close();

    Ok(())
}

#[test]
fn a_kept_machine_is_saved_once_nothing_is_waiting_for_it() -> Result<(), Failure> {
    let directory = fresh("actor-saved-idle")?;
    let Ok(store) = KeyValueStore::file_system(directory);
    let walk = boot_serializable::<Walk>(Depth(0), store.clone(), KEY)?;

    for _ in 0..3 {
        walk.actor.send(Request::Down)?;
    }

    let Ok(patience) = Schedule::of(PATIENCE);
    let Ok(saved) = until_some(patience, || kept(&store, Depth(3)));

    assert_eq!(saved, Some(Depth(3)), "the state was not written while the actor stood idle");

    let Ok(()) = walk.scope.close();

    Ok(())
}

#[test]
fn a_kept_machine_that_falls_starts_again_from_where_it_was_kept() -> Result<(), Failure> {
    let directory = fresh("actor-falls-kept")?;
    let Ok(store) = KeyValueStore::file_system(directory);
    let walk = boot_serializable::<Walk>(Depth(0), store.clone(), KEY)?;

    for _ in 0..4 {
        walk.actor.send(Request::Down)?;
    }

    let Ok(patience) = Schedule::of(PATIENCE);
    let Ok(saved) = until_some(patience, || kept(&store, Depth(4)));

    assert_eq!(saved, Some(Depth(4)));
    assert_eq!(walk.actor.send(Request::Fall), Err(SendError::MachineDefect(vec![Effect::StandingAt(Depth(4))])));
    assert_eq!(walk.actor.get(), Ok(Depth(4)), "the fall threw away what was kept");

    let Ok(()) = walk.scope.close();

    Ok(())
}

#[test]
fn a_saved_state_that_will_not_be_read_starts_fresh() -> Result<(), Failure> {
    let directory = fresh("actor-garbage")?;
    let Ok(store) = KeyValueStore::file_system(directory);

    store.set(KEY, b"not a snapshot")?;

    let walk = boot_serializable::<Walk>(Depth(2), store, KEY)?;

    assert_eq!(walk.actor.get(), Ok(Depth(2)));

    let Ok(()) = walk.scope.close();

    Ok(())
}

#[test]
fn a_key_that_will_not_be_read_is_not_started_over() -> Result<(), Failure> {
    let directory = fresh("actor-unreadable")?;

    std::fs::create_dir_all(directory.join(KEY))?;

    let Ok(store) = KeyValueStore::file_system(directory);

    assert!(matches!(boot_serializable::<Walk>(Depth(0), store, KEY), Err(BootError::Store(_))), "an unreadable save was booted over");

    Ok(())
}

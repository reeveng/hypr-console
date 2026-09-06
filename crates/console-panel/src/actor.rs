//! State with one owner, reached by sending it a message.
//!
//! A panel draws on the main thread and reads its rows on another. What a tab
//! is looking at, where a walk has got to, what has been typed into a line:
//! all of that is written by a thumb on the one thread and read by the reader
//! on the other, so it is the only mutable thing a panel has that two threads
//! can see at once.
//!
//! The plain answer is a lock around it, and every panel here has written that
//! answer out itself. A lock is not wrong, and it is not what it looks like
//! either: `.lock()` says a second thread might be in there, which is true
//! about a tenth of a second per open and false the rest of the time; the
//! `.expect()` under it says a thread could have panicked holding it, which is
//! a case nobody has ever seen and nobody has decided what to do about. Six
//! panels have six copies of that decision, which is six answers waiting to
//! disagree.
//!
//! So the state is given one owner instead:
//!
//! ```text
//! Machine::step(self, Msg) -> Self
//! ```
//!
//! State goes in, a message happens, state comes out. Nothing borrows it, so
//! there is nothing to lock; the type of the mailbox says exactly which
//! messages can reach it; and what the state does next is one function that
//! can be read in one place rather than a dozen scattered `standing(...)`
//! blocks. A message carries what it needs by value, which is what lets it
//! cross to the reader thread at all.
//!
//! The owner is held by a supervisor rather than by the caller. A message that
//! makes the state panic costs the state and nothing else: the mailbox and
//! every handle survive, and the next message meets a machine that has started
//! over. On a desktop where every piece already starts itself again when it
//! dies, that is the same promise one step smaller.

use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::mpsc::channel;
use std::thread::JoinHandle;

use console_never::Never;

#[derive(Debug)]
pub struct Gone;

impl std::fmt::Display for Gone {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("the state's owner is gone")
    }
}

impl std::error::Error for Gone {}

pub trait Machine: Send + 'static {
    type Msg: Send + 'static;

    fn step(self, message: Self::Msg) -> Self;
}

enum Post<M> {
    Message(M),
    Stop,
}

pub struct Addr<M> {
    outbox: Sender<Post<M>>,
}

impl<M> Clone for Addr<M> {
    fn clone(&self) -> Self {
        Self { outbox: self.outbox.clone() }
    }
}

impl<M: Send + 'static> Addr<M> {
    pub fn tell(&self, message: M) -> Result<(), Gone> {
        self.outbox.send(Post::Message(message)).map_err(|_| Gone)
    }

    pub fn ask<T: Send + 'static>(&self, build: impl FnOnce(Answer<T>) -> M) -> Result<T, Gone> {
        let (said, hear) = channel();
        self.tell(build(Answer { said }))?;
        hear.recv().map_err(|_| Gone)
    }
}

pub struct Answer<T> {
    said: Sender<T>,
}

impl<T> Answer<T> {
    pub fn say(self, value: T) -> Result<(), Gone> {
        self.said.send(value).map_err(|_| Gone)
    }
}

pub struct Running<M> {
    pub addr: Addr<M>,
    thread: Option<JoinHandle<()>>,
}

impl<M> Running<M> {
    pub fn shutdown(self) -> Result<(), Never> {
        let Self { addr, thread } = self;
        let _ = addr.outbox.send(Post::Stop);
        drop(addr);

        match thread {
            Some(thread) => {
                let _ = thread.join();
            }
            None => {},
        }

        Ok(())
    }
}

pub fn supervise<A: Machine>(
    start: impl Fn() -> A + Send + 'static,
) -> Result<Running<A::Msg>, Never> {
    let (outbox, inbox) = channel();
    let thread = std::thread::spawn(move || {
        let Ok(()) = own(start, inbox);
    });

    Ok(Running { addr: Addr { outbox }, thread: Some(thread) })
}

fn own<A: Machine>(start: impl Fn() -> A, inbox: Receiver<Post<A::Msg>>) -> Result<(), Never> {
    inbox
        .into_iter()
        .map_while(|post| match post {
            Post::Message(message) => Some(message),
            Post::Stop => None,
        })
        .fold(start(), |state, message| {
            match catch_unwind(AssertUnwindSafe(|| state.step(message))) {
                Ok(next) => next,
                Err(_) => {
                    eprintln!("console: the panel's state fell, starting it over");
                    start()
                },
            }
        });

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    struct Depth(u64);

    struct Walk {
        depth: Depth,
    }

    enum Msg {
        Down,
        Up,
        Fall,
        Where(Answer<Depth>),
        Ignore(Answer<Depth>),
        FallHolding(Answer<Depth>),
        Told(Sender<Depth>),
    }

    fn out_of_a_list(at: Depth) -> Depth {
        let rows: Vec<Depth> = Vec::new();
        rows[usize::try_from(at.0).unwrap_or_default()]
    }

    impl Machine for Walk {
        type Msg = Msg;

        fn step(self, message: Msg) -> Self {
            match message {
                Msg::Down => Walk { depth: Depth(self.depth.0 + 1) },
                Msg::Up => Walk { depth: Depth(self.depth.0.saturating_sub(1)) },
                Msg::Fall => Walk { depth: out_of_a_list(self.depth) },
                Msg::Where(answer) => {
                    let _ = answer.say(self.depth);
                    self
                },
                Msg::Ignore(answer) => {
                    drop(answer);
                    self
                },
                Msg::FallHolding(answer) => {
                    let _ = &answer;
                    Walk { depth: out_of_a_list(self.depth) }
                },
                Msg::Told(said) => {
                    let _ = said.send(self.depth);
                    self
                },
            }
        }
    }

    fn depth(walk: &Running<Msg>) -> Depth {
        walk.addr.ask(Msg::Where).expect("the machine answered")
    }

    fn within<T: Send + 'static>(patience: Duration, doing: impl FnOnce() -> T + Send + 'static)
    -> Option<T> {
        let (said, hear) = channel();
        std::thread::spawn(move || {
            let _ = said.send(doing());
        });
        hear.recv_timeout(patience).ok()
    }

    #[test]
    fn it_holds_a_state_without_a_lock() {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });
        let _ = walk.addr.tell(Msg::Down);
        let _ = walk.addr.tell(Msg::Down);
        let _ = walk.addr.tell(Msg::Up);
        assert_eq!(depth(&walk), Depth(1));
        let Ok(()) = walk.shutdown();
    }

    #[test]
    fn a_fall_costs_the_state_and_nothing_else() {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });
        let _ = walk.addr.tell(Msg::Down);
        let _ = walk.addr.tell(Msg::Down);
        let _ = walk.addr.tell(Msg::Fall);
        let _ = walk.addr.tell(Msg::Down);
        assert_eq!(depth(&walk), Depth(1));
        let Ok(()) = walk.shutdown();
    }

    #[test]
    fn two_threads_one_owner() {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });
        let readers: Vec<JoinHandle<()>> = (0..8)
            .map(|_| {
                let addr = walk.addr.clone();
                std::thread::spawn(move || {
                    (0..1000).for_each(|_| {
                        let _ = addr.tell(Msg::Down);
                    })
                })
            })
            .collect();
        readers.into_iter().for_each(|reader| {
            let _ = reader.join();
        });
        assert_eq!(depth(&walk), Depth(8000));
        let Ok(()) = walk.shutdown();
    }

    #[test]
    fn a_question_nobody_answers_comes_back_rather_than_hanging() {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });
        let addr = walk.addr.clone();
        let said = within(Duration::from_secs(5), move || addr.ask(Msg::Ignore));
        assert!(said.is_some(), "asking hung: an unanswered question never came back");
        assert!(said.is_some_and(|answer| answer.is_err()), "an unanswered question answered");
        assert_eq!(depth(&walk), Depth::default());
        let Ok(()) = walk.shutdown();
    }

    #[test]
    fn a_question_the_state_falls_under_is_told_rather_than_left_waiting() {
        let Ok(walk) = supervise(|| Walk { depth: Depth(3) });
        assert!(walk.addr.tell(Msg::Down).is_ok());
        assert!(walk.addr.tell(Msg::Down).is_ok());
        assert_eq!(depth(&walk), Depth(5), "the state did not move before the fall");

        let addr = walk.addr.clone();
        let said = within(Duration::from_secs(5), move || addr.ask(Msg::FallHolding));
        assert!(said.is_some(), "asking hung: the state fell and the asker was never told");
        assert!(said.is_some_and(|answer| answer.is_err()), "a state that fell still answered");

        assert_eq!(depth(&walk), Depth(3), "the machine did not start over after the fall");
        let Ok(()) = walk.shutdown();
    }

    #[test]
    fn an_answer_goes_back_to_whoever_asked_for_it() {
        struct Echo;
        enum Say {
            Back(u64, Answer<u64>),
        }
        impl Machine for Echo {
            type Msg = Say;
            fn step(self, message: Say) -> Self {
                match message {
                    Say::Back(mine, answer) => {
                        let _ = answer.say(mine);
                        self
                    },
                }
            }
        }

        let Ok(echo) = supervise(|| Echo);
        let asking: Vec<JoinHandle<()>> = (0..16_u64)
            .map(|who| {
                let addr = echo.addr.clone();
                std::thread::spawn(move || {
                    for round in 0..64_u64 {
                        let mine = who * 1000 + round;
                        let back = addr.ask(|answer| Say::Back(mine, answer));
                        assert_eq!(back.ok(), Some(mine), "an answer went to the wrong asker");
                    }
                })
            })
            .collect();
        for thread in asking {
            assert!(thread.join().is_ok(), "an asker fell");
        }
        let Ok(()) = echo.shutdown();
    }

    #[test]
    fn stopping_works_through_what_was_already_sent() {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });
        for _ in 0..500 {
            assert!(walk.addr.tell(Msg::Down).is_ok(), "the mailbox closed early");
        }
        let (said, hear) = channel();
        assert!(walk.addr.tell(Msg::Told(said)).is_ok(), "the mailbox closed early");
        let Ok(()) = walk.shutdown();
        assert_eq!(hear.recv().ok(), Some(Depth(500)), "messages were dropped on the way out");
    }

    #[test]
    fn a_clone_of_a_stopped_address_is_gone_too() {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });
        let one = walk.addr.clone();
        let other = walk.addr.clone();
        let Ok(()) = walk.shutdown();
        assert!(one.tell(Msg::Down).is_err(), "a clone still accepted a message");
        let asked = within(Duration::from_secs(5), move || other.ask(Msg::Where));
        assert!(asked.is_some(), "asking a stopped machine hung");
        assert!(asked.is_some_and(|answer| answer.is_err()), "a stopped machine answered");
    }

    #[test]
    fn what_was_sent_behind_a_fall_still_arrives() {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });
        assert!(walk.addr.tell(Msg::Down).is_ok());
        assert!(walk.addr.tell(Msg::Fall).is_ok());
        for _ in 0..7 {
            assert!(walk.addr.tell(Msg::Down).is_ok(), "the mailbox died with the state");
        }
        assert_eq!(depth(&walk), Depth(7), "the seven sent after the fall did not all arrive");
        let Ok(()) = walk.shutdown();
    }

    #[test]
    fn asking_something_that_has_gone_says_so() {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });
        let addr = walk.addr.clone();
        let Ok(()) = walk.shutdown();
        assert!(addr.ask(Msg::Where).is_err());
    }
}

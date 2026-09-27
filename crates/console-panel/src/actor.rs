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
//! a case no one has ever seen and no one has decided what to do about. Six
//! panels have six copies of that decision, which is six answers waiting to
//! disagree.
//!
//! So the state is given one owner instead:
//!
//! ```text
//! Machine::step(self, Message) -> Self
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

use console_core_never::Never;

#[derive(Debug)]
pub struct Closed;

impl std::fmt::Display for Closed {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("the state's owner is gone")
    }
}

impl std::error::Error for Closed {}

pub trait Machine: Send + 'static {
    type Message: Send + 'static;

    fn step(self, message: Self::Message) -> Self;
}

enum Post<M> {
    Message(M),
    Stop,
}

pub struct Address<M> {
    outbox: Sender<Post<M>>,
}

impl<M> Clone for Address<M> {
    fn clone(&self) -> Self {
        Self { outbox: self.outbox.clone() }
    }
}

impl<M: Send + 'static> Address<M> {
    pub fn tell(&self, message: M) -> Result<(), Closed> {
        self.outbox.send(Post::Message(message)).map_err(|_| Closed)
    }

    pub fn ask<T: Send + 'static>(&self, build: impl FnOnce(Answer<T>) -> M) -> Result<T, Closed> {
        let (said, hear) = channel();
        self.tell(build(Answer { said }))?;
        hear.recv().map_err(|_| Closed)
    }
}

pub struct Answer<T> {
    said: Sender<T>,
}

impl<T> Answer<T> {
    pub fn say(self, value: T) -> Result<(), Closed> {
        self.said.send(value).map_err(|_| Closed)
    }
}

pub struct Running<M> {
    pub address: Address<M>,
    thread: Option<JoinHandle<()>>,
}

impl<M> Running<M> {
    pub fn shutdown(self) -> Result<(), Never> {
        let Self { address, thread } = self;
        let _ = address.outbox.send(Post::Stop);
        drop(address);

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
) -> Result<Running<A::Message>, Never> {
    let (outbox, inbox) = channel();
    let thread = std::thread::spawn(move || {
        let Ok(()) = own(start, inbox);
    });

    Ok(Running { address: Address { outbox }, thread: Some(thread) })
}

fn own<A: Machine>(start: impl Fn() -> A, inbox: Receiver<Post<A::Message>>) -> Result<(), Never> {
    inbox
        .into_iter()
        .map_while(|post| match post {
            Post::Message(message) => Some(message),
            Post::Stop => None,
        })
        .fold(start(), |state, message| {
            match catch_unwind(AssertUnwindSafe(|| state.step(message))) {
                Ok(next) => next,
                Err(_the_step_panicked) => {
                    eprintln!("console: the panel's state fell, starting it over");
                    start()
                },
            }
        });

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;

    use super::*;

    type Failure = Box<dyn std::error::Error>;

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    struct Depth(u64);

    struct Walk {
        depth: Depth,
    }

    enum Message {
        Down,
        Up,
        Fall,
        Where(Answer<Depth>),
        Ignore(Answer<Depth>),
        FallHolding(Answer<Depth>),
        Subscribe(Sender<Depth>),
    }

    #[cfg_attr(dylint_lib = "explicit004_no_panic", allow(explicit004_no_panic, reason = "a state that falls is what the supervisor is for, and a panic is the only thing that presses it"))]
    fn fall() -> ! {
        panic!("the state fell where it was told to")
    }

    impl Machine for Walk {
        type Message = Message;

        fn step(self, message: Message) -> Self {
            match message {
                Message::Down => Walk { depth: Depth(self.depth.0.saturating_add(1)) },
                Message::Up => Walk { depth: Depth(self.depth.0.saturating_sub(1)) },
                Message::Fall => fall(),
                Message::Where(answer) => {
                    let _ = answer.say(self.depth);
                    self
                },
                Message::Ignore(answer) => {
                    drop(answer);
                    self
                },
                Message::FallHolding(answer) => {
                    let _ = &answer;
                    fall()
                },
                Message::Subscribe(said) => {
                    let _ = said.send(self.depth);
                    self
                },
            }
        }
    }

    fn within<T: Send + 'static>(patience: Duration, doing: impl FnOnce() -> T + Send + 'static)
    -> Result<T, RecvTimeoutError> {
        let (said, hear) = channel();
        let asking = std::thread::spawn(move || {
            let _ = said.send(doing());
        });
        let Ok(()) = console_program_lifetime::threads::let_go(asking);

        hear.recv_timeout(patience)
    }

    #[test]
    fn it_holds_a_state_without_a_lock() -> Result<(), Failure> {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });

        walk.address.tell(Message::Down)?;
        walk.address.tell(Message::Down)?;
        walk.address.tell(Message::Up)?;

        let depth = walk.address.ask(Message::Where)?;

        assert_eq!(depth, Depth(1));

        let Ok(()) = walk.shutdown();

        Ok(())
    }

    #[test]
    fn a_fall_costs_the_state_and_nothing_else() -> Result<(), Failure> {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });

        walk.address.tell(Message::Down)?;
        walk.address.tell(Message::Down)?;
        walk.address.tell(Message::Fall)?;
        walk.address.tell(Message::Down)?;

        let depth = walk.address.ask(Message::Where)?;

        assert_eq!(depth, Depth(1));

        let Ok(()) = walk.shutdown();

        Ok(())
    }

    #[test]
    fn two_threads_one_owner() -> Result<(), Failure> {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });

        std::thread::scope(|readers| {
            for _ in 0..8 {
                let address = walk.address.clone();

                readers.spawn(move || {
                    for _ in 0..1000 {
                        let _ = address.tell(Message::Down);
                    }
                });
            }
        });

        let depth = walk.address.ask(Message::Where)?;

        assert_eq!(depth, Depth(8000));

        let Ok(()) = walk.shutdown();

        Ok(())
    }

    #[test]
    fn a_question_no_one_answers_comes_back_rather_than_hanging() -> Result<(), Failure> {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });
        let address = walk.address.clone();
        let said = within(Duration::from_secs(5), move || address.ask(Message::Ignore));

        assert!(matches!(said, Ok(Err(Closed))), "an unanswered question answered, or never came back");

        let depth = walk.address.ask(Message::Where)?;

        assert_eq!(depth, Depth::default());

        let Ok(()) = walk.shutdown();

        Ok(())
    }

    #[test]
    fn a_question_the_state_falls_under_is_told_rather_than_left_waiting() -> Result<(), Failure> {
        let Ok(walk) = supervise(|| Walk { depth: Depth(3) });

        walk.address.tell(Message::Down)?;
        walk.address.tell(Message::Down)?;

        let before = walk.address.ask(Message::Where)?;

        assert_eq!(before, Depth(5), "the state did not move before the fall");

        let address = walk.address.clone();
        let said = within(Duration::from_secs(5), move || address.ask(Message::FallHolding));

        assert!(matches!(said, Ok(Err(Closed))), "a state that fell still answered, or the asker was never told");

        let after = walk.address.ask(Message::Where)?;

        assert_eq!(after, Depth(3), "the machine did not start over after the fall");

        let Ok(()) = walk.shutdown();

        Ok(())
    }

    struct Echo;

    enum Say {
        Back(u64, Answer<u64>),
    }

    impl Machine for Echo {
        type Message = Say;

        fn step(self, message: Say) -> Self {
            match message {
                Say::Back(mine, answer) => {
                    let _ = answer.say(mine);
                    self
                },
            }
        }
    }

    #[test]
    fn an_answer_goes_back_to_whoever_asked_for_it() {
        let Ok(echo) = supervise(|| Echo);

        std::thread::scope(|asking| {
            for who in 0..16_u64 {
                let address = echo.address.clone();

                asking.spawn(move || {
                    for round in 0..64_u64 {
                        let mine = who.saturating_mul(1000).saturating_add(round);
                        let back = address.ask(|answer| Say::Back(mine, answer));

                        assert_eq!(back.map_err(|closed| closed.to_string()), Ok(mine), "an answer went to the wrong asker");
                    }
                });
            }
        });

        let Ok(()) = echo.shutdown();
    }

    #[test]
    fn stopping_works_through_what_was_already_sent() -> Result<(), Failure> {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });

        for _ in 0..500 {
            walk.address.tell(Message::Down)?;
        }

        let (said, hear) = channel();

        walk.address.tell(Message::Subscribe(said))?;

        let Ok(()) = walk.shutdown();

        assert_eq!(hear.recv(), Ok(Depth(500)), "messages were dropped on the way out");

        Ok(())
    }

    #[test]
    fn a_clone_of_a_stopped_address_is_gone_too() {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });
        let one = walk.address.clone();
        let other = walk.address.clone();
        let Ok(()) = walk.shutdown();

        assert!(matches!(one.tell(Message::Down), Err(Closed)), "a clone still accepted a message");

        let asked = within(Duration::from_secs(5), move || other.ask(Message::Where));

        assert!(matches!(asked, Ok(Err(Closed))), "a stopped machine answered, or asking it hung");
    }

    #[test]
    fn what_was_sent_behind_a_fall_still_arrives() -> Result<(), Failure> {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });

        walk.address.tell(Message::Down)?;
        walk.address.tell(Message::Fall)?;

        for _ in 0..7 {
            walk.address.tell(Message::Down)?;
        }

        let depth = walk.address.ask(Message::Where)?;

        assert_eq!(depth, Depth(7), "the seven sent after the fall did not all arrive");

        let Ok(()) = walk.shutdown();

        Ok(())
    }

    #[test]
    fn asking_something_that_has_gone_says_so() {
        let Ok(walk) = supervise(|| Walk { depth: Depth::default() });
        let address = walk.address.clone();
        let Ok(()) = walk.shutdown();

        assert!(matches!(address.ask(Message::Where), Err(Closed)));
    }
}

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

/// The owner is not there to answer.
///
/// Only ever the panel coming down: the owner outlives every handle to it, so
/// this is the machine having panicked on the message before this one, or the
/// panel already on its way out. It is an error rather than an absence, which
/// is why it is not an `Option`.
#[derive(Debug)]
pub struct Gone;

impl std::fmt::Display for Gone {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("the state's owner is gone")
    }
}

impl std::error::Error for Gone {}

/// Owned state, and the messages allowed to change it.
pub trait Machine: Send + 'static {
    /// Everything that can happen to this state, and nothing else.
    type Msg: Send + 'static;

    /// Take the state, handle one message, hand the next state back.
    fn step(self, message: Self::Msg) -> Self;
}

/// What travels down the mailbox: a message, or the word to stop.
enum Post<M> {
    Message(M),
    Stop,
}

/// A handle to a machine's mailbox.
///
/// Cheap to clone, safe to send anywhere, and it can carry nothing except
/// `Msg`. A panel hands one of these to every closure that used to be handed
/// an `Arc<Mutex<_>>`.
pub struct Addr<M> {
    outbox: Sender<Post<M>>,
}

impl<M> Clone for Addr<M> {
    fn clone(&self) -> Self {
        Self { outbox: self.outbox.clone() }
    }
}

impl<M: Send + 'static> Addr<M> {
    /// Say something and carry on.
    ///
    /// For a message whose answer nobody is waiting for: a word typed into a
    /// line, a tab arrived at. The redraw that follows is what reads the
    /// result, and it asks in its own time.
    pub fn tell(&self, message: M) -> Result<(), Gone> {
        self.outbox.send(Post::Message(message)).map_err(|_| Gone)
    }

    /// Ask, and wait for the answer.
    ///
    /// The channel the answer comes back on is part of the message, so the
    /// type system knows what a given question is answered with.
    pub fn ask<T: Send + 'static>(&self, build: impl FnOnce(Answer<T>) -> M) -> Result<T, Gone> {
        let (said, hear) = channel();
        self.tell(build(Answer { said }))?;
        hear.recv().map_err(|_| Gone)
    }
}

/// The half of a question that the answer goes back down, handed to the
/// machine inside the message.
pub struct Answer<T> {
    said: Sender<T>,
}

impl<T> Answer<T> {
    /// Hand the answer back.
    ///
    /// The asker can have stopped waiting -- a redraw whose panel closed
    /// underneath it -- so this says whether anybody was still there, rather
    /// than pretending every question is heard.
    pub fn say(self, value: T) -> Result<(), Gone> {
        self.said.send(value).map_err(|_| Gone)
    }
}

/// A machine that is running: where to reach it, and the thread it lives on.
pub struct Running<M> {
    pub addr: Addr<M>,
    thread: Option<JoinHandle<()>>,
}

impl<M> Running<M> {
    /// Ask it to stop, and wait for it to work through what it was sent.
    ///
    /// The word to stop goes down the mailbox behind everything already in it,
    /// so this loses no message and waits on nobody else's copy of the
    /// address.
    pub fn shutdown(self) {
        let Self { addr, thread } = self;
        let _ = addr.outbox.send(Post::Stop);
        drop(addr);

        match thread {
            Some(thread) => {
                let _ = thread.join();
            }
            None => {},
        }
    }
}

/// Start a machine under a supervisor that starts it again if it falls.
///
/// `start` builds the first state, and builds it again after a fall. One child
/// and one rule, which is all a panel has ever needed.
pub fn supervise<A: Machine>(start: impl Fn() -> A + Send + 'static) -> Running<A::Msg> {
    let (outbox, inbox) = channel();
    let thread = std::thread::spawn(move || own(start, inbox));
    Running { addr: Addr { outbox }, thread: Some(thread) }
}

/// The owner's whole life: fold the mailbox into the state.
fn own<A: Machine>(start: impl Fn() -> A, inbox: Receiver<Post<A::Msg>>) {
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
                    // Said rather than swallowed. A panel quietly starting its
                    // state over is a panel that forgets which folder you were
                    // in and looks like it was never told.
                    eprintln!("console: the panel's state fell, starting it over");
                    start()
                },
            }
        });
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// How many folders down a walk has got.
    ///
    /// Named rather than a bare `u64`, so a signature that takes one says what
    /// it wants without a comment under it saying so.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    struct Depth(u64);

    /// A tab that can be walked into and out of, which is every panel here
    /// with the drawing taken away.
    struct Walk {
        depth: Depth,
    }

    enum Msg {
        Down,
        Up,
        Fall,
        Where(Answer<Depth>),
        /// A question the state takes and never answers, which is what a
        /// handler with an early return in it looks like from outside.
        Ignore(Answer<Depth>),
        /// A question the state falls under while holding the answer. This is
        /// the one that matters: every panel asks on the way to a redraw, and
        /// a redraw that waits for ever is a panel frozen on the screen.
        FallHolding(Answer<Depth>),
        /// Say where it is down a channel of the caller's, so what the state
        /// stood at can be read after the machine has stopped.
        Told(Sender<Depth>),
    }

    /// How a panel actually falls: a row number that outlived the list it was
    /// pointing into.
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

    /// What the machine says it is standing at.
    ///
    /// It insists on an answer rather than reading `Gone` as depth zero. A
    /// helper that turns "the machine is not there" into a plausible number
    /// is a helper that lets every test below pass against a machine that
    /// died.
    fn depth(walk: &Running<Msg>) -> Depth {
        walk.addr.ask(Msg::Where).expect("the machine answered")
    }

    /// Run something on a thread of its own and give up on it after a while.
    ///
    /// Every question below could be answered by hanging for ever, and a test
    /// that hangs is a test nobody can run. `None` is "it never came back",
    /// which is a failure with a name rather than a suite that has to be
    /// killed.
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
        let walk = supervise(|| Walk { depth: Depth::default() });
        let _ = walk.addr.tell(Msg::Down);
        let _ = walk.addr.tell(Msg::Down);
        let _ = walk.addr.tell(Msg::Up);
        assert_eq!(depth(&walk), Depth(1));
        walk.shutdown();
    }

    /// The whole of what the supervisor is for. A panel whose state falls is a
    /// panel back at the top of the walk, not a panel that has gone.
    #[test]
    fn a_fall_costs_the_state_and_nothing_else() {
        let walk = supervise(|| Walk { depth: Depth::default() });
        let _ = walk.addr.tell(Msg::Down);
        let _ = walk.addr.tell(Msg::Down);
        let _ = walk.addr.tell(Msg::Fall);
        let _ = walk.addr.tell(Msg::Down);
        assert_eq!(depth(&walk), Depth(1));
        walk.shutdown();
    }

    /// A panel draws on one thread and reads its rows on another. Both hold
    /// the same address, and only one of them ever holds the state.
    #[test]
    fn two_threads_one_owner() {
        let walk = supervise(|| Walk { depth: Depth::default() });
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
        walk.shutdown();
    }

    /// A question the state takes and never answers comes back as `Gone`,
    /// rather than as a wait with no end to it.
    ///
    /// Nothing stops a handler from returning early with the answer still in
    /// its hand. If that left the asker waiting, one early return anywhere in
    /// six panels would be a panel that freezes on a redraw and a bug nobody
    /// could find from the symptom.
    #[test]
    fn a_question_nobody_answers_comes_back_rather_than_hanging() {
        let walk = supervise(|| Walk { depth: Depth::default() });
        let addr = walk.addr.clone();
        let said = within(Duration::from_secs(5), move || addr.ask(Msg::Ignore));
        assert!(said.is_some(), "asking hung: an unanswered question never came back");
        assert!(said.is_some_and(|answer| answer.is_err()), "an unanswered question answered");
        // And the machine is still there for the next one.
        assert_eq!(depth(&walk), Depth::default());
        walk.shutdown();
    }

    /// The same, when the state falls while holding the answer.
    ///
    /// This is the one that would hurt. Every panel asks on its way to a
    /// redraw, and the supervisor's whole promise is that a fall costs the
    /// state and nothing else -- which is only true if whoever was waiting is
    /// told. The `Answer` is dropped by the unwind, and dropping it is what
    /// says so.
    #[test]
    fn a_question_the_state_falls_under_is_told_rather_than_left_waiting() {
        // Walked away from where it starts, so that starting over is a
        // different number from standing still. Asking a machine that begins
        // at three whether it is at three proves nothing.
        let walk = supervise(|| Walk { depth: Depth(3) });
        assert!(walk.addr.tell(Msg::Down).is_ok());
        assert!(walk.addr.tell(Msg::Down).is_ok());
        assert_eq!(depth(&walk), Depth(5), "the state did not move before the fall");

        let addr = walk.addr.clone();
        let said = within(Duration::from_secs(5), move || addr.ask(Msg::FallHolding));
        assert!(said.is_some(), "asking hung: the state fell and the asker was never told");
        assert!(said.is_some_and(|answer| answer.is_err()), "a state that fell still answered");

        assert_eq!(depth(&walk), Depth(3), "the machine did not start over after the fall");
        walk.shutdown();
    }

    /// Answers do not cross between askers.
    ///
    /// Every question carries the channel its own answer comes back on, so
    /// this should hold by construction. It is asked anyway because the thing
    /// it replaced -- one lock, read by whoever got there first -- had no such
    /// guarantee, and "by construction" is worth one test that leans on it.
    #[test]
    fn an_answer_goes_back_to_whoever_asked_for_it() {
        /// One counter, and questions that each carry their own number.
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

        let echo = supervise(|| Echo);
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
        echo.shutdown();
    }

    /// Stopping works through what was already sent.
    ///
    /// The word to stop goes down the mailbox behind everything in it, and
    /// this is what says so. A shutdown that jumped the queue would be a panel
    /// closing on a keypress it had already accepted, which is the kind of
    /// loss nobody reports because it looks like a missed press.
    #[test]
    fn stopping_works_through_what_was_already_sent() {
        let walk = supervise(|| Walk { depth: Depth::default() });
        for _ in 0..500 {
            assert!(walk.addr.tell(Msg::Down).is_ok(), "the mailbox closed early");
        }
        let (said, hear) = channel();
        assert!(walk.addr.tell(Msg::Told(said)).is_ok(), "the mailbox closed early");
        walk.shutdown();
        assert_eq!(hear.recv().ok(), Some(Depth(500)), "messages were dropped on the way out");
    }

    /// A machine that has stopped refuses every handle to it, not only the
    /// one that stopped it.
    ///
    /// The panels clone the address into every closure they hand to GTK, and
    /// those closures outlive the panel closing. A clone that still looked
    /// live would be a redraw waiting on a thread that had gone.
    #[test]
    fn a_clone_of_a_stopped_address_is_gone_too() {
        let walk = supervise(|| Walk { depth: Depth::default() });
        let one = walk.addr.clone();
        let other = walk.addr.clone();
        walk.shutdown();
        assert!(one.tell(Msg::Down).is_err(), "a clone still accepted a message");
        let asked = within(Duration::from_secs(5), move || other.ask(Msg::Where));
        assert!(asked.is_some(), "asking a stopped machine hung");
        assert!(asked.is_some_and(|answer| answer.is_err()), "a stopped machine answered");
    }

    /// A fall costs the state and not the mailbox.
    ///
    /// Everything sent behind the message that fell is still delivered, to a
    /// machine that has started over. The count is what says the mailbox
    /// survived: a supervisor that rebuilt the channel would lose them.
    #[test]
    fn what_was_sent_behind_a_fall_still_arrives() {
        let walk = supervise(|| Walk { depth: Depth::default() });
        assert!(walk.addr.tell(Msg::Down).is_ok());
        assert!(walk.addr.tell(Msg::Fall).is_ok());
        for _ in 0..7 {
            assert!(walk.addr.tell(Msg::Down).is_ok(), "the mailbox died with the state");
        }
        assert_eq!(depth(&walk), Depth(7), "the seven sent after the fall did not all arrive");
        walk.shutdown();
    }

    /// Asking a machine that has stopped is an error with a name on it, rather
    /// than a wait that never ends.
    #[test]
    fn asking_something_that_has_gone_says_so() {
        let walk = supervise(|| Walk { depth: Depth::default() });
        let addr = walk.addr.clone();
        walk.shutdown();
        assert!(addr.ask(Msg::Where).is_err());
    }
}

//! What a program on this device is: the same events in, the same effects out.
//!
//! `docs/programs.md` is the argument and this is the contract it arrives at.
//! A program here holds a state, is told one event at a time, and answers with
//! the state it holds now and a list of what it wants done. It never does any
//! of it. Something else -- the loop in the program's own `main`, which is
//! the only part of it that touches a machine -- carries the effects out.
//!
//! Everything the plan claims falls out of that one split. A test is a list of
//! events and a list of the effects that should come back, and it runs on a
//! laptop with no compositor, no controller and no network, because nothing in
//! here can reach one. [`transcript`] is that test, and it is the deliverable:
//! a trait with no way to press it is a shape rather than a contract.
//!
//! ## What is deliberately not here
//!
//! **The loop.** An earlier draft had a runtime in this crate that owned the
//! program's `main`. It cannot: GTK draws on one thread and will not be driven
//! from someone else's loop, so half the programs on this device have to keep
//! their own. What is shared is the shape of a decision, not the shape of a
//! loop, and a crate that promised the second would have been wrong for every
//! panel.
//!
//! **Drawing.** `docs/programs.md` listed "draw these pages" as an effect and
//! also gave the trait a `showing`, which is the same thing said twice. Only
//! one of them can be the truth about what is on the screen, and it is
//! `showing`: the runtime redraws from the state, which is the whole reason
//! [`Program::State`] is asked to be `PartialEq`. So there is no `Effect::Show`
//! here, and `showing` is not on [`Program`] either -- seven of the fourteen
//! programs never draw, and a `Vec<Page>` they all have to return empty is the
//! ceremony that document is against. A program that draws says so by
//! implementing a second trait, in the crate whose vocabulary a page is.
//!
//! **`offers()`.** Stage 6, and conditional on two programs wanting it. A
//! registry written before anything asks for one is the piece most likely to
//! be built bigger than anything needs.
//!
//! ## Its own events, and its own effects
//!
//! [`Event`] and [`Effect`] are closed sets shared by every program, which is
//! what makes them worth having and is also the way this can go wrong. A
//! variant no one else uses is one program's private business kept in a shared
//! type, and enough of those and this crate is the shelf `CLAUDE.md` forbids.
//!
//! The shape was settled against three real programs -- `controller-desktop`,
//! `console-wallpaper` and `settings-panel` -- and one of them broke it.
//! Everything `console-wallpaper` and `settings-panel` do is a kind of effect any
//! program might ask for: run this and tell me what it said, start this and forget
//! it, write this file, say this on the screen. `controller-desktop` emits pointer
//! motion on a virtual device it holds, and no other program on this machine will
//! ever want to, because the plan is that one program reads the input and the rest
//! are told.
//!
//! So the shared sets stay about kinds of effect, and a program names its own
//! in its own crate: [`Program::Event`] for the events only it can be told, and
//! [`Program::Effect`] for the effects only it can ask for. Both are
//! `console_core_never::Never` for a program that has neither, which is most of
//! them. What this does not give up is the transcript -- a private effect is
//! still a value that was decided rather than carried out, so it is still in
//! the list a test asserts on. What it gives up is that shared code can carry
//! it out, which is the honest cost: the program that asked for it is the one
//! that knows what it means.

pub mod arguments;
pub mod effect;
pub mod program;
pub mod transcript;
pub mod subscription;
pub mod event;

pub use arguments::{Arguments, Flag};
pub use effect::{Effect, Exit, Executable, Prompt, Command, Notification, FileWrite};
pub use program::{Initial, Program, Update};
pub use transcript::{Step, Trace, run, run_from};
pub use subscription::{Subscription, Timer};
pub use event::{Answer, Change, Choice, Elapsed, Topic, ExitStatus, Event};

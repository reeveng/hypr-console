//! Input, claimed by whatever is in front of you.
//!
//! One sentence is the whole of it: **while this surface is up, nothing else
//! acts on what somebody is pressing**. Three programs here want that and,
//! before this crate, all three took it a different way.
//!
//! The on-screen keyboard opened the pad and leaned on an InputPlumber profile
//! to hand it over untranslated. The card that asks which button you just
//! pressed loaded a profile that sent every button to a key nothing listens
//! for, and then read the keys instead of the pad. `console-buttons
//! --identify` stopped the controller daemon with `SIGSTOP` and started it
//! again afterwards. Two of those are a profile load, and a profile load
//! destroys the pad and builds another -- the fault half the comments in
//! `console-gamepad` are about. The third is worse: stopped is not deaf, so
//! every press made meanwhile arrived in one instant against a desktop that
//! had moved on.
//!
//! What all three wanted is the kernel's own answer. `EVIOCGRAB` says this
//! process is the one the device delivers to and nothing else receives a
//! thing, and it is a property no signal and no profile can be given: the
//! kernel holds it, and the kernel lets go when the process goes, however it
//! goes. There is nothing to restore, nothing to remember, and no window in
//! which a program that died left the machine wrong.
//!
//! ## Two layers, and one road between them
//!
//! [`devices`] is the layer that talks to the machine: which input devices this
//! desktop has, taking them, handing them back, and the raw events in between.
//! It is the only part here that opens anything.
//!
//! [`said`] is the layer above it: one event, in one vocabulary, whatever it
//! arrived on. A press is `South` whether it came off the pad, off the keyboard
//! InputPlumber publishes beside it, or off some input method nobody has
//! written yet -- because what a program wants to know is which button a person
//! pressed and never which device file said so.
//!
//! That is the road, and it runs one way: a device is taken, what arrives is
//! named, and the program decides. Nothing here decides anything, and nothing
//! here says who *should* have the input. It hands out one claim, refuses the
//! second, and that is all it does.
//!
//! `console-onscreen` is the precedent and the argument. It exists because the
//! panel and the daemon both needed to know what was in front of you, and a
//! daemon reading a pad twenty times a second should not carry a toolkit to
//! find out. Same shape here: three callers, one answer, where there were three
//! copies of it that were each wrong differently.

pub mod devices;
pub mod said;

pub use devices::{CONTROLLER, Claim, Heard, Refused, Spans, Which};
pub use said::{Said, Went, said};

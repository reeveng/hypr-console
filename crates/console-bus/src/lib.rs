//! What this desktop says on the session bus, and what it hears there.
//!
//! One name is owned on this machine that nothing here wrote the code for:
//! `org.freedesktop.Notifications`, which every program that raises a card
//! calls, and which was mako's. Taking it means being a bus service, and being
//! a bus service means speaking the wire format, because there is no way to
//! own a name through someone else's program: `busctl` can call and it can
//! watch, and it cannot answer.
//!
//! So this is the wire and nothing above it. What a notification *is* belongs
//! to `console-notifications`, which is the one crate here that calls this; the
//! two are kept apart for the reason everything here is kept apart, that a
//! format read in the crate that spells its vocabulary is a format read twice
//! the day something else speaks it.
//!
//! It is deliberately not a D-Bus library. There is no introspection built out
//! of a type and no fd passing. `properties` and `introspection` are the two
//! things every service on a bus is asked whether or not it cares, written once
//! so the music player and the notification daemon stop writing them by hand.
//!
//! The second caller was the Bluetooth agent, which has to be on the system bus
//! because bluez is, and has to make calls as well as answer them, because a
//! pairing is both at once. What it wanted was the system bus's address, an
//! error reply in a service's own name, and `replies`: a reply matched to the
//! call it answers by the serial the call went out under, so a program can have
//! a call outstanding and still hear everything else that arrives.
//!
//! Then the rest of Bluetooth came off the programs it had been reading
//! through -- the settings tab off `bluetoothctl`, the bar off `busctl`'s
//! JSON, the event pool off `gdbus monitor` -- and what they wanted was
//! smaller still: `call`, which waits for the one answer it asked for,
//! `subscribe`, which asks the bus for signals by a rule, a `Set` written for
//! somebody else's service, and two readings of a value, a truth and an entry
//! in a dictionary looked up by its key. Every other caller that only wants an
//! answer still goes out through `busctl`, which
//! `console-core-external-programs` already names, and moves here when its
//! crate is next opened rather than all at once.

pub mod connection;
pub mod introspection;
pub mod messages;
pub mod properties;
pub mod replies;

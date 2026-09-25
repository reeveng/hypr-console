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
//! of a type, no fd passing and no client side beyond what opening a connection
//! needs -- a call goes out through `busctl` here, and
//! `console-core-external-programs` already names it. `properties` and
//! `introspection` are the two things every service on a bus is asked whether
//! or not it cares, written once so the music player and the notification
//! daemon stop writing them by hand; what else a second caller wants is what it
//! will want rather than a second spelling of this.

pub mod messages;
pub mod connection;
pub mod introspection;
pub mod properties;

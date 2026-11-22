//! What bluez is asked on the system bus, and what its answers mean.
//!
//! bluez is the Bluetooth daemon on every Linux machine, and everything it
//! does for a program it does over D-Bus: a device is an object with a path,
//! pairing is a method on it, being trusted is a property, and the questions
//! a pairing raises -- a code to show, a device asking to be let in -- are
//! calls bluez makes back to an agent the program registered. This crate is
//! that vocabulary: the messages bluez is sent, built, and the messages it
//! sends, read.
//!
//! It began as BlueR, the Rust interface bluez publishes, carried under
//! `forks/` for a day. BlueR is the whole of bluez behind tokio and
//! the `dbus` crate -- GATT, mesh, L2CAP and RFCOMM sockets, advertising --
//! and this desktop needed a pairing. What was kept is what was bluez's
//! rather than BlueR's: the names of its errors, its interfaces, the methods
//! an agent answers and the capabilities it can claim. What went is the
//! runtime, two D-Bus libraries and a dozen crates that existed to hold them
//! up, because `console-bus` already speaks the wire.
//!
//! **Nothing here opens a socket.** Every function builds a message or reads
//! one, so a whole pairing can be pressed in a test with bluez nowhere near
//! it. In what order the messages go, and what is done while the answers are
//! awaited, is `console-bluetooth-agent`'s, which is a machine; the loop that
//! carries its messages to the bus is the program at the edge.

pub mod adapter;
pub mod agent;
pub mod device;
pub mod errors;
pub mod objects;

pub const SERVICE: &str = "org.bluez";

pub const ROOT: &str = "/org/bluez";

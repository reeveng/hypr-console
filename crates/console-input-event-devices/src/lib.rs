//! The kernel's input devices: read, taken, and made.
//!
//! Every event node is `input-event-codes.h` and a handful of ioctls, and
//! this is the one place in the tree that speaks them. It used to be the
//! `evdev` crate, which is a bitset library, a system-call library, an async
//! runtime's worth of streams and every event the kernel has, carried so that
//! a handheld's pad could be opened, grabbed, read and faked; what is here is
//! what that took, written against the C library std already links.
//!
//! `devices` and `presses` are the pad for the programs that run where
//! nothing else is reading it: the way in runs before the controller daemon is
//! up and recovery runs when it may have fallen, so they read every node
//! themselves and agree on what a press is. `touches` is the same for a
//! finger on the screen, which the greeter reads and recovery does not.

pub mod codes;
pub mod device;
pub mod devices;
pub mod event;
mod kernel;
pub mod keys;
pub mod presses;
pub mod touches;
pub mod virtual_device;

pub use codes::{
    AbsoluteAxisCode, BusType, EventType, ForceFeedbackCode, MiscCode, PropType, RelativeAxisCode,
    SynchronizationCode,
};
pub use device::Device;
pub use event::InputEvent;
pub use kernel::{AbsInfo, InputId};
pub use keys::KeyCode;
pub use virtual_device::{Setup, Step, Unmade, VirtualDevice};

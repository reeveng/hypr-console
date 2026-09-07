//! Build the public copy of this, with nobody's name in it.
//!
//! What comes out is the same desktop with four things changed, and it is
//! built rather than kept, so it can be built again when this one moves on.
//! Keeping a second copy by hand is how the two come to disagree.
//!
//!   * The person is called player rather than the name of the person whose
//!     machine this is.
//!   * The machine is called handheld rather than the name on the network.
//!   * The controller's serial number is gone from the captured devices.
//!   * The two compiled programs are not carried. Both are forks of somebody
//!     else's GPL project, and publishing a binary means offering the source
//!     with it, which is theirs to publish and not ours. What is published
//!     instead is where each came from and how to build it.
//!
//! Everything else is the same file. The tests run against the copy, because a
//! scrub that breaks the desktop is a scrub that has not been read.

pub mod names;
pub mod papers;
pub mod tree;

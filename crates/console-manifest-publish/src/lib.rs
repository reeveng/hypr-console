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
//!   * The one compiled program is not carried. It is a fork of somebody
//!     else's GPL project, and publishing a binary means offering the source
//!     with it, which is theirs to publish and not ours. What is published
//!     instead is where it came from and how to build it.
//!
//! There were two, and the second is the argument for why this is about
//! binaries rather than about forks. `console-resume` began as one -- a fork
//! held back at a path -- and became a crate, and a crate is carried. Most of
//! its lines are this tree's now, GPL-3.0 asks that it stay GPL-3.0 and not
//! that it stay unpublished, upstream is named in `papers/forks.md` and in the
//! crate's own `authors`, and a workspace that names a member the copy does not
//! carry is a copy nobody can build. What was being protected by holding it
//! back was somebody else's release, and there is no longer one here to
//! withhold.
//!
//! Everything else is the same file. The tests run against the copy, because a
//! scrub that breaks the desktop is a scrub that has not been read.

pub mod names;
pub mod papers;
pub mod tree;

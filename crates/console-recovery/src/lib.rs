//! What is left on the screen when the desktop did not come up.
//!
//! The way in is a ladder, and this is its middle rung. The top is the greeter,
//! which draws. The bottom is a plain login prompt on a text console, which
//! needs a keyboard this device does not have. This is the rung between: the
//! same choices the greeter offers when a session falls, set in the kernel's
//! own letters and driven by the pad, so it is still there when everything
//! that draws is what broke.
//!
//! **Each rung is its own program, and systemd is what steps down.** A rung
//! that is a mode of the one above it fails with it -- a library that will not
//! load takes the whole binary, not the part that used it. So this links
//! nothing that draws, says it is ready only once the menu is on the screen,
//! and a unit's `OnFailure=` is what reaches for the next rung when it does
//! not. A supervisor written here would be one more thing on the ladder that
//! can fall.
//!
//! **The kernel sets the letters.** A text console already draws text on
//! whatever screen the kernel found, before and without anything this tree
//! builds, so the menu is words and escapes written to it and there is no
//! font of ours to be missing.

pub mod menu;

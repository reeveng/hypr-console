//! A piece of the screen this desktop draws on itself.
//!
//! Every surface over the desktop is GTK's today, and GTK is the last thing on
//! this device deciding something this tree has already decided better. What a
//! toolkit decides is what fits, what order a press moves in, what a press
//! means and what color a thing is, and all four are answered here already, in
//! places a screen cannot reach. What is left underneath is a rectangle the
//! compositor is reading and the protocol to keep it there, which is this.
//!
//! **It is written rather than lifted.** `console-input-keyboard` has stood a
//! layer surface up since the wvkbd port landed and the obvious move was to
//! take that module out into a crate everything shares. It is not ours to take.
//! That crate is GPL-3.0-or-later because it is a derivative of someone else's
//! keyboard, this workspace is AGPL-3.0-or-later, and the AGPL is not a later
//! version of the GPL -- which is the argument `console_input_keyboard`'s own
//! head makes about itself. So the protocol is written again from the XML,
//! which no one owns, and the keyboard keeps its copy until it is the last
//! caller left and can be asked whether it wants to change license.
//!
//! **What it answers first is 2.5.** This panel is driven at a scale that is
//! not a whole number and `set_buffer_scale` takes an integer, so a surface of
//! our own was a surface that came out soft or oversized until something here
//! could say a fraction. [`scale::Scale`] is that, in hundred and twentieths,
//! and [`standing`] spends it in one place. Nothing above this crate stops
//! measuring in points.
//!
//! **What is not here is drawing.** This hands out a slice of bytes the width
//! and height of the frame and has no opinion about what goes in it. The shapes
//! this machine can draw are a closed list somewhere else, and keeping the two
//! apart is what lets the list be asserted against with no screen in the room.

pub mod fingers;
pub mod memory;
pub mod scale;
pub mod standing;

pub use scale::Scale;
pub use standing::{
    Anchor, Closed, Damage, Visible, Keyboard, KeyboardEvent, Keysym, Margin, Part, PointerEvent,
    Room, Surface, SurfaceError, Under, Wanted, on_the_device,
};

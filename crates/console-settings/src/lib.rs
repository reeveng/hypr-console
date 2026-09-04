//! What Legion right opens: the settings, each kind in its own place.
//!
//! Screen is how bright it is, what colour it goes in the evening and how big
//! everything on it is, Battery is how hard the machine is allowed to work and
//! what it says on the way down, Wi-Fi and Bluetooth are what it talks to,
//! Sound is what comes out of it, System is how it stops. Nothing that turns
//! the machine off shares a page with anything you would touch every day.
//!
//! Everything here is read from the machine each time the panel is drawn, so a
//! network that has just come into range appears and one that has gone is gone.
//! Reading the machine is one half and understanding what it said is the other,
//! and the second half is written here where it can be asked without a machine.

pub mod bluetooth;
pub mod defaults;
pub mod level;
pub mod rocker;
pub mod rows;
pub mod screen;
pub mod size;
pub mod sound;
pub mod stopping;
pub mod wallpaper;
pub mod warm;
pub mod wifi;
pub mod words;

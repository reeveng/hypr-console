//! What Legion right opens: the settings, each kind in its own place.
//!
//! Battery is how the machine runs, which is the screen as much as the profile,
//! Wi-Fi and Bluetooth are what it talks to, Sound is what comes out of it,
//! System is how it stops. Nothing that turns the machine off shares a page
//! with anything you would touch every day.
//!
//! Everything here is read from the machine each time the panel is drawn, so a
//! network that has just come into range appears and one that has gone is gone.
//! Reading the machine is one half and understanding what it said is the other,
//! and the second half is written here where it can be asked without a machine.

pub mod bluetooth;
pub mod defaults;
pub mod level;
pub mod rows;
pub mod sound;
pub mod wallpaper;
pub mod wifi;

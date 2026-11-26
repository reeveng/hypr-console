//! What Legion right opens: the settings, each kind in its own place.
//!
//! Screen is how bright it is, what color it goes in the evening and how big
//! everything on it is, Battery is how hard the machine is allowed to work and
//! what it says on the way down, Wi-Fi and Bluetooth are what it talks to,
//! Sound is what comes out of it, System is how it stops. Nothing that turns
//! the machine off shares a page with anything you would touch every day.
//!
//! Everything here is read from the machine each time the panel is drawn, so a
//! network that has just come into range appears and one that has gone is gone.
//! Reading the machine is one half and understanding what it said is the other,
//! and the second half is written here where it can be asked without a machine.

mod about;
pub mod bluetooth;
pub mod card;
pub mod choosing;
pub mod defaults;
pub mod display;
pub mod hours;
mod keyboard_layouts;
pub mod languages;
pub mod learned;
pub mod level;
pub mod light;
pub mod login;
pub mod machine;
pub mod named;
pub mod network_code;
mod pointer;
pub mod power_profile;
mod quick_workspace;
pub mod rocker;
pub mod rows;
pub mod screen;
pub mod screens;
pub mod size;
pub mod sound;
mod status_bar;
pub mod stopping;
mod theme;
pub mod turning;
pub mod wallpaper;
pub mod wifi;
mod window_placement;
pub mod words;

pub use card::{COMMAND, WHO, card, door};

pub mod warm {
  pub use console_night_light::*;
}

pub use window_placement::{TOGGLE_WINDOW_PLACEMENT, toggle_window_placement};

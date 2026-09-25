//! What this desktop opens things with.
//!
//! Which browser a link means and which engine a question is asked of. Both
//! used to be written into a program. A setting no one can reach is a setting
//! someone has to be asked to change, and there is no one to ask on a machine
//! with one person on it.
//!
//! The browser is xdg-settings', because every program on the machine asks
//! that and a second copy here would be a second answer. The engine has no
//! such place, so it is kept in `console-defaults` with the other settings no
//! one else owns.

pub mod browsers;
pub mod clock;
pub mod engines;
pub mod policies;

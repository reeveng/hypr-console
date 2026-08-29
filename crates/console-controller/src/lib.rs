//! The desktop half of the controller: scrolling, and the buttons that ask the
//! compositor for something.
//!
//! Both live here for the same reason. Sending these through the compositor as
//! key bindings never held: function keys did not resolve to a keysym Hyprland
//! would match, raw keycodes were stored as literal strings, and a modifier
//! plus a letter races, because InputPlumber emits the pair in one event frame
//! and the letter can reach the focused window before the modifier applies.
//! That is how pressing X typed a k into the terminal. Reading the pad has no
//! such ambiguity, and has worked first time every time.
//!
//! Nothing in this library opens a device. What arrives is handed in and what
//! to do about it is handed back, so every decision the daemon makes can be
//! asked of it twice and answered the same way.

pub mod buttons;
pub mod doing;
pub mod finding;
pub mod reading;
pub mod scroll;
pub mod touch;
pub mod turning;

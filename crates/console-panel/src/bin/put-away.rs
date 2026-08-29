//! Put away whatever is up.
//!
//! The right paddle closes, always. What closing means depends on what is on
//! screen rather than on which profile the pad happens to be in: a chooser if
//! one is up, the focused window if none is.
//!
//! It is decided here because the pad's profile changes a beat after the screen
//! does, and a button whose meaning is written into the profile means one thing
//! during that beat and another outside it. Pressed there it closed the window
//! behind a menu that had just opened.

use std::process::Command;

use console_panel::chooser;

fn main() {
    if chooser::put_away() {
        return;
    }
    let _ = Command::new("hyprctl").args(["dispatch", "hl.dsp.window.close()"]).status();
}

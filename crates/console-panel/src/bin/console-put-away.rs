//! Put away whatever is up.
//!
//! The right paddle closes, always. What closing means depends on what is on
//! screen rather than on which profile the pad happens to be in: a picker if
//! one is up, and the focused window if not. An app of ours is a window like
//! any other, so when it is the one in front it is the one that closes.
//!
//! It is decided here because the pad's profile changes a beat after the screen
//! does, and a button whose meaning is written into the profile means one thing
//! during that beat and another outside it. Pressed there it closed the window
//! behind a menu that had just opened.


use console_panel::picker;

fn main() {
    let Ok(away) = picker::console_put_away();

    match away == picker::Away::Notified {
        true => return,
        false => {},
    }

    let Ok(_done) = console_compositor::request(
        console_compositor::Request::Dispatch,
        console_compositor::CLOSE_WINDOW,
    );
}

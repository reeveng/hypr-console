//! Put away whatever is up.
//!
//! The right paddle closes, always. What closing means depends on what is on
//! screen rather than on which profile the pad happens to be in: a picker if
//! one is up, the app on top if one is, and the focused window if neither is.
//! An app is a layer over the windows, so the window it covers is not what a
//! person pressing the paddle is looking at.
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

    let Ok(away) = picker::app_put_away();

    match away == picker::Away::Notified {
        true => return,
        false => {},
    }

    let Ok(_done) = console_compositor::request(
        console_compositor::Request::Dispatch,
        console_compositor::CLOSE_WINDOW,
    );
}

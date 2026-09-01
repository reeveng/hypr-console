//! Leave Steam and come back to the desktop.
//!
//! Held on the left Legion button, which is the button the desktop left on. A
//! press stays Steam's, so its menu is where it always was.

use console_session::{GAME_TARGET, Session, here, run};

fn main() {
    if here(GAME_TARGET) == Session::Desktop {
        return;
    }
    run(Session::Game, Session::Desktop);
}

//! Leave Steam and come back to the desktop.
//!
//! Held on the left Legion button, which is the button the desktop left on. A
//! press stays Steam's, so its menu is where it always was.

use console_session::{GAME_TARGET, Session, here, run};

fn main() {
    let Ok(here) = here(GAME_TARGET);

    match here {
        Session::Desktop => {}
        Session::Game => {
            let Ok(()) = run(Session::Game, Session::Desktop);
        }
    }
}

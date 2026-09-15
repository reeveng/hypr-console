//! Leave the desktop and go to Steam. Bound to the left Legion button.

use console_session::{GAME_TARGET, Session, here, run};

fn main() {
    let Ok(here) = here(GAME_TARGET);

    match here {
        Session::Game => {}
        Session::Desktop => {
            let Ok(()) = run(Session::Desktop, Session::Game);
        }
    }
}

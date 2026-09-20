//! Leave the desktop and go to Steam. Bound to the left Legion button.
//!
//! What the radio was doing is written down on the way past, because Steam
//! powers the adapter down when its session comes up and nothing here would
//! otherwise know it had ever been on. `console_session::radio` is the rest.

use console_core_never::Never;
use console_session::{GAME_TARGET, Session, here, radio, run};

fn main() {
    let Ok(here) = here(GAME_TARGET);

    match here {
        Session::Game => {}
        Session::Desktop => {
            let Ok(()) = remembering();
            let Ok(()) = run(Session::Desktop, Session::Game);
        }
    }
}

fn remembering() -> Result<(), Never> {
    let Ok(runtime) = console_core_places::runtime_ours();

    let runtime = match runtime {
        Some(runtime) => runtime,
        None => return Ok(()),
    };

    let asked = match radio::asked() {
        Ok(asked) => asked,
        Err(fault) => {
            eprintln!("session-game: {fault}");

            return Ok(());
        }
    };

    match radio::remember(&runtime, asked) {
        Ok(()) => {},
        Err(fault) => eprintln!("session-game: {fault}"),
    }

    Ok(())
}

//! Leave the desktop and go to Steam. Bound to the left Legion button.

use console_session::{GAME_TARGET, Session, here, run};

fn main() {
    if here(GAME_TARGET) == Session::Game {
        return;
    }

    run(Session::Desktop, Session::Game);
}

//! Leave Steam and come back to the desktop.
//!
//! Held on the left Legion button, which is the button the desktop left on. A
//! press stays Steam's, so its menu is where it always was.

use console_session::{Session, SwitchBackend, run};

use console_session_cachyos::CachyOs;

fn main() {
  let Ok(here) = CachyOs.current();

  match here {
    Session::Desktop => {}
    Session::Game => {
      let Ok(()) = run(Session::Game, Session::Desktop, &CachyOs);
    }
  }
}

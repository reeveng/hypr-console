//! Leave the desktop and go to Steam. Bound to the left Legion button.
//!
//! What the radio was doing is written down on the way past, because Steam
//! powers the adapter down when its session comes up and nothing here would
//! otherwise know it had ever been on. `console_session::radio` is the rest.

use console_core_never::Never;
use console_session::{Session, SwitchBackend, radio, run};

use console_session_cachyos::CachyOs;

fn main() {
  let Ok(here) = CachyOs.current();

  match here {
    Session::Game => {}
    Session::Desktop => {
      let Ok(()) = remember_session();
      let Ok(()) = run(Session::Desktop, Session::Game, &CachyOs);
    }
  }
}

fn remember_session() -> Result<(), Never> {
  let Ok(runtime) = console_core_places::application_runtime();

  let runtime_3 = match runtime {
    Some(runtime_2) => runtime_2,
    None => return Ok(()),
  };

  let asked = match radio::query_radio() {
    Ok(asked) => asked,
    Err(fault) => {
      eprintln!("session-game: {fault}");

      return Ok(());
    }
  };

  match radio::remember(&runtime_3, asked) {
    Ok(()) => {}
    Err(fault) => eprintln!("session-game: {fault}"),
  }

  Ok(())
}

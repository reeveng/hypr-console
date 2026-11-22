//! Hand the session's environment to systemd, then start the desktop.
//!
//! Run by the compositor, once, as it comes up.
//!
//! The radio is put back last, after the desktop has been asked for, because
//! powering an adapter that Steam turned off is the better part of two seconds
//! and none of it is anything someone is waiting to see.

use console_core_never::Never;
use console_session::{radio, run_each, startup_commands};

fn main() {
  let Ok(starting) = startup_commands();
  let Ok(()) = run_each("starting", &starting);
  let Ok(()) = putting_back();
}

fn putting_back() -> Result<(), Never> {
  let Ok(runtime) = console_core_places::application_runtime();

  let runtime_3 = match runtime {
    Some(runtime_2) => runtime_2,
    None => return Ok(()),
  };

  let before = match radio::load_saved(&runtime_3) {
    Ok(before) => before,
    Err(fault) => {
      eprintln!("session-start: {fault}");

      return Ok(());
    }
  };

  match before {
    Some(_) => {}
    None => return Ok(()),
  }

  let asked = match radio::query_radio() {
    Ok(asked) => asked,
    Err(fault) => {
      eprintln!("session-start: {fault}");

      return Ok(());
    }
  };

  let Ok(owed) = radio::putting_back(before, asked);

  match owed {
    Some(power) => match radio::power_radio(power) {
      Ok(()) => {}
      Err(fault) => eprintln!("session-start: {fault}"),
    },
    None => {}
  }

  match radio::forget(&runtime_3) {
    Ok(()) => {}
    Err(fault) => eprintln!("session-start: {fault}"),
  }

  Ok(())
}

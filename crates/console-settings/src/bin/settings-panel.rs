//! The settings, asked for.
//!
//!     settings-panel
//!     settings-panel Sound
//!
//! What is on each tab and why they are apart is `console_settings::card`'s own
//! head. This is the program the bar's icons click and the Legion right button
//! opens: it takes the screen, asks the host to draw the settings on it, and
//! holds the screen until they are gone.

use std::process::ExitCode;

use console_panel::card::{Panel, opened};

fn main() -> ExitCode {
  let asked: Vec<String> = std::env::args().skip(1).collect();

  match asked.as_slice() {
    [action] => match action.as_str() {
      console_settings::TOGGLE_WINDOW_PLACEMENT => {
        let Ok(status) = console_settings::toggle_window_placement();
        return status;
      }
      _ => {}
    },
    _ => {}
  }

  let Ok(code) = opened(
    &asked,
    Panel {
      who: console_settings::WHO,
      command: console_settings::COMMAND,
      door: console_settings::door,
      card: console_settings::card,
    },
  );

  code
}

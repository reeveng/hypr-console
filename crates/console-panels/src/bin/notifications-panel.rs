//! What the desktop has said, asked for.
//!
//! Draw the notification inbox through the resident panel host.
//! This is the program someone types and the desktop opens: it takes the
//! screen, asks the host to draw the card on it, and holds the screen until
//! the card is gone.

use std::process::ExitCode;

use console_panel::card::{Panel, opened};
use console_panels::notification_panel;

fn main() -> ExitCode {
  let asked: Vec<String> = std::env::args().skip(1).collect();

  let Ok(code) = opened(
    &asked,
    Panel {
      who: notification_panel::WHO,
      command: notification_panel::COMMAND,
      door: notification_panel::door,
      card: notification_panel::card,
    },
  );

  code
}

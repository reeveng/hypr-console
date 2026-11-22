use std::process::ExitCode;

use console_panel::card::{Panel, opened};
use console_task_manager::{COMMAND, WHO, card, door};

fn main() -> ExitCode {
  let asked: Vec<String> = std::env::args().skip(1).collect();
  let Ok(code) = opened(
    &asked,
    Panel {
      who: WHO,
      command: COMMAND,
      door,
      card,
    },
  );

  code
}

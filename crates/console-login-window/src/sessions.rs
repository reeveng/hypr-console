//! Read the command from the desktop entry for a login session.

use console_applications::entry::DesktopEntry;
use console_applications::words;
use console_core_never::Never;

pub const SESSIONS: &str = "/usr/share/wayland-sessions";

pub fn command(entry: &str) -> Result<Option<Vec<String>>, Never> {
  let Ok(read) = DesktopEntry::read(entry);

  match read.command {
    Some(command) => words::split(command),
    None => Ok(None),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_session_runs_its_entrys_command() {
    let entry = "[Desktop Entry]\nType=Application\nName=Console\nExec=/usr/bin/start-hyprland -- --config /home/a/hyprland.lua\n";
    let Ok(command) = command(entry);

    assert_eq!(
      command,
      Some(vec![
        "/usr/bin/start-hyprland".to_string(),
        "--".to_string(),
        "--config".to_string(),
        "/home/a/hyprland.lua".to_string(),
      ])
    );
  }
}

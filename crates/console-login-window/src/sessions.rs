//! Which desktop a login starts, and what running it means.
//!
//! The choice is not this tree's. `steamos-session-select` is how Game Mode is
//! entered and left: it writes the next session's name under `[Autologin]` in
//! the file plasmalogin reads, and restarts the login. So that file is read
//! here the way plasmalogin read it, and a machine that has never been into
//! Game Mode, where the file names nothing, starts this desktop's own session.
//! A session is a desktop entry under `wayland-sessions`, and what it runs is
//! that entry's command, read by the crate that reads desktop entries.

use console_applications::entry::DesktopEntry;
use console_applications::words;
use console_core_ini_files::{Key, Under, field};
use console_core_never::Never;

pub const CHOSEN: &str = "/etc/plasmalogin.conf.d/zz-steamos-autologin.conf";

pub const SESSIONS: &str = "/usr/share/wayland-sessions";

pub const OURS: &str = "console.desktop";

const AUTOLOGIN: Under<'static> = Under("Autologin");

const SESSION: Key<'static> = Key("Session");

pub fn chosen(said: Option<&str>) -> Result<String, Never> {
    let named = match said {
        Some(said) => {
            let Ok(named) = field(said, AUTOLOGIN, SESSION);

            named
        }
        None => None,
    };

    Ok(match named.map(str::trim) {
        Some("") | None => OURS.to_string(),
        Some(named) => match named.ends_with(".desktop") {
            true => named.to_string(),
            false => format!("{named}.desktop"),
        },
    })
}

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
    fn a_machine_that_never_chose_starts_this_desktop() {
        let Ok(none) = chosen(None);
        let Ok(empty) = chosen(Some("[Autologin]\nSession=\n"));

        assert_eq!(none, OURS);
        assert_eq!(empty, OURS);
    }

    #[test]
    fn game_mode_is_the_session_the_switch_wrote() {
        let Ok(named) = chosen(Some("[Autologin]\nSession=gamescope-session-steam\n"));

        assert_eq!(named, "gamescope-session-steam.desktop");
    }

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

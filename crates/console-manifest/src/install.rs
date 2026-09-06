//! Putting one file where the manifest says it goes.

use std::path::{Path, PathBuf};

use console_never::Never;

use crate::settled::Settled;


pub const USER: &str = "@user@";

fn home_of(user: &str) -> Result<String, Never> {
    Ok(format!("/home/{user}/"))
}

pub fn on_machine(live: &str, user: &str) -> Result<String, Never> {
    let Ok(marked) = home_of(USER);
    let Ok(theirs) = home_of(user);

    Ok(match live.strip_prefix(&marked) {
        Some(rest) => format!("{theirs}{rest}"),
        None => live.to_string(),
    })
}

pub fn as_declared(live: &str, user: &str) -> Result<String, Never> {
    let Ok(theirs) = home_of(user);
    let Ok(marked) = home_of(USER);

    Ok(match live.strip_prefix(&theirs) {
        Some(rest) => format!("{marked}{rest}"),
        None => live.to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Ok,
    Differs,
    Missing,
    Unreadable,
    Unsourced,
}

impl State {
    pub fn name(self) -> Result<&'static str, Never> {
        Ok(match self {
            State::Ok => "ok",
            State::Differs => "differs",
            State::Missing => "missing",
            State::Unreadable => "cannot read",
            State::Unsourced => "unsourced",
        })
    }

    pub fn settled(self) -> Result<Settled, Never> {
        Ok(match self == State::Ok {
            true => Settled::Yes,
            false => Settled::No,
        })
    }
}

pub fn source_of(source: &Path, live: &str) -> Result<PathBuf, Never> {
    Ok(source.join(live.trim_start_matches('/')))
}

pub fn content_on_machine(held: &[u8], user: &str, _live: &str) -> Result<Vec<u8>, Never> {
    let Ok(text) = std::str::from_utf8(held) else { return Ok(held.to_vec()) };

    Ok(match text.contains(USER) {
        true => text.replace(USER, user).into_bytes(),
        false => held.to_vec(),
    })
}

pub fn content_as_declared(held: &[u8], user: &str) -> Result<Vec<u8>, Never> {
    Ok(match std::str::from_utf8(held) {
        Ok(text) if text.contains(user) => text.replace(user, USER).into_bytes(),
        Ok(_) | Err(_) => held.to_vec(),
    })
}

pub fn state(source: &Path, live: &str, user: &str) -> Result<State, Never> {
    let Ok(on) = on_machine(live, user);
    let Ok(from) = source_of(source, live);
    let to = Path::new(&on);

    Ok(match (std::fs::read(&from), std::fs::read(to)) {
        (Err(_), _) => State::Unsourced,
        (Ok(_), Err(fault)) if fault.kind() == std::io::ErrorKind::PermissionDenied => {
            State::Unreadable
        }
        (Ok(_), Err(_)) => State::Missing,
        (Ok(held), Ok(there)) => {
            let Ok(content) = content_on_machine(&held, user, live);

            match content == there {
                true => State::Ok,
                false => State::Differs,
            }
        }
    })
}

pub fn owner_of(live: &str, user: &str) -> Result<String, Never> {
    let Ok(marked) = home_of(USER);
    let Ok(theirs) = home_of(user);

    Ok(match live.starts_with(&marked) || live.starts_with(&theirs) {
        true => user.to_string(),
        false => "root".to_string(),
    })
}

pub fn holding(live: &str) -> Result<Vec<PathBuf>, Never> {
    let mut dirs: Vec<PathBuf> = Path::new(live)
        .parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .filter(|dir| !dir.as_os_str().is_empty() && *dir != Path::new("/"))
        .map(Path::to_path_buf)
        .collect();
    dirs.reverse();

    Ok(dirs)
}

pub fn mode_of(live: &str, head: &[u8]) -> Result<u32, Never> {
    Ok(match live {
        path if path.contains("/bin/") || path.contains("/sbin/") => 0o755,
        _ => match head {
            [b'#', b'!', ..] => 0o755,
            [0x7f, b'E', b'L', b'F', ..] => 0o755,
            _ => 0o644,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_of(source: &Path, live: &str) -> PathBuf {
        let Ok(at) = super::source_of(source, live);

        at
    }

    fn state(source: &Path, live: &str, user: &str) -> State {
        let Ok(state) = super::state(source, live, user);

        state
    }

    fn owner_of(live: &str, user: &str) -> String {
        let Ok(owner) = super::owner_of(live, user);

        owner
    }

    fn on_machine(live: &str, user: &str) -> String {
        let Ok(on) = super::on_machine(live, user);

        on
    }

    fn as_declared(live: &str, user: &str) -> String {
        let Ok(declared) = super::as_declared(live, user);

        declared
    }

    fn content_on_machine(held: &[u8], user: &str, live: &str) -> Vec<u8> {
        let Ok(content) = super::content_on_machine(held, user, live);

        content
    }

    fn content_as_declared(held: &[u8], user: &str) -> Vec<u8> {
        let Ok(content) = super::content_as_declared(held, user);

        content
    }

    fn holding(live: &str) -> Vec<PathBuf> {
        let Ok(holding) = super::holding(live);

        holding
    }

    fn mode_of(live: &str, head: &[u8]) -> u32 {
        let Ok(mode) = super::mode_of(live, head);

        mode
    }

    #[test]
    fn a_source_path_is_the_live_path_under_the_tree() {
        let source = Path::new("/etc/console/files");
        assert_eq!(
            source_of(source, "/usr/local/bin/launcher"),
            Path::new("/etc/console/files/usr/local/bin/launcher")
        );
    }

    const SOMEBODY: &str = "ada";

    #[test]
    fn a_file_nobody_here_may_read_is_not_a_file_that_is_missing() {
        use std::os::unix::fs::PermissionsExt;

        let here = std::env::temp_dir().join(format!("console-shut-{}", std::process::id()));
        let live = here.join("live/console");
        let source = here.join("files");
        std::fs::create_dir_all(source_of(&source, &live.to_string_lossy()).parent().expect("a parent"))
            .expect("the source");
        std::fs::write(source_of(&source, &live.to_string_lossy()), b"what it should be\n")
            .expect("the source");
        std::fs::create_dir_all(here.join("live")).expect("somewhere live");
        std::fs::write(&live, b"what it should be\n").expect("the live file");

        let said = state(&source, &live.to_string_lossy(), SOMEBODY);
        assert_eq!(said, State::Ok, "the same file, while it can be read");

        std::fs::set_permissions(here.join("live"), std::fs::Permissions::from_mode(0o000))
            .expect("shut");
        let shut = std::fs::read(&live).is_err();
        let said = state(&source, &live.to_string_lossy(), SOMEBODY);
        std::fs::set_permissions(here.join("live"), std::fs::Permissions::from_mode(0o755)).ok();
        std::fs::remove_dir_all(&here).ok();

        match !shut {
            true => return,
            false => {},
        }

        assert_eq!(said, State::Unreadable, "a file that cannot be read is not a file that is gone");
        assert_ne!(said, State::Missing);
    }

    #[test]
    fn a_file_in_a_home_belongs_to_whoever_lives_there() {
        assert_eq!(owner_of("/home/@user@/.config/hypr/hyprland.lua", SOMEBODY), SOMEBODY);
        assert_eq!(owner_of("/home/ada/.config/hypr/hyprland.lua", SOMEBODY), SOMEBODY);
        assert_eq!(owner_of("/etc/systemd/user/console.target", SOMEBODY), "root");
        assert_eq!(owner_of("/home/adam/.bashrc", SOMEBODY), "root");
        assert_eq!(owner_of("/home/someone/.bashrc", SOMEBODY), "root");
    }

    #[test]
    fn the_mark_is_filled_in_when_a_path_reaches_the_machine() {
        assert_eq!(
            on_machine("/home/@user@/.config/hypr/hyprland.lua", SOMEBODY),
            "/home/ada/.config/hypr/hyprland.lua"
        );
        assert_eq!(on_machine("/etc/pamac.conf", SOMEBODY), "/etc/pamac.conf");
    }

    #[test]
    fn a_path_somebody_typed_is_taken_back_to_the_mark() {
        assert_eq!(
            as_declared("/home/ada/.config/hypr/hyprland.lua", SOMEBODY),
            "/home/@user@/.config/hypr/hyprland.lua"
        );
        assert_eq!(as_declared("/etc/pamac.conf", SOMEBODY), "/etc/pamac.conf");
    }

    #[test]
    fn a_path_taken_to_the_machine_and_back_is_the_path_it_was() {
        let declared = "/home/@user@/.librewolf/console/user.js";
        assert_eq!(as_declared(&on_machine(declared, SOMEBODY), SOMEBODY), declared);
    }

    #[test]
    fn the_mark_is_filled_in_inside_a_file_as_well_as_in_its_name() {
        let held = b"@user@ ALL=(root) NOPASSWD: /usr/local/bin/console-engine\n";
        assert_eq!(
            content_on_machine(held, SOMEBODY, "/etc/sudoers.d/console"),
            b"ada ALL=(root) NOPASSWD: /usr/local/bin/console-engine\n".to_vec()
        );
    }

    #[test]
    fn nothing_but_the_mark_is_filled_in() {
        let held = b"    source_event:\n      gamepad:\n        button: LeftPaddle1\n";
        for live in ["/etc/inputplumber/profiles/game.yaml", "/usr/local/bin/keyboard-toggle"] {
            assert_eq!(content_on_machine(held, SOMEBODY, live), held.to_vec());
        }
    }

    #[test]
    fn a_file_saved_off_the_machine_carries_the_mark_and_not_a_name() {
        let held = b"ada ALL=(root) NOPASSWD: /usr/local/bin/console-engine\n";
        assert_eq!(
            content_as_declared(held, SOMEBODY),
            b"@user@ ALL=(root) NOPASSWD: /usr/local/bin/console-engine\n".to_vec()
        );
    }

    #[test]
    fn what_is_not_text_is_carried_through_untouched() {
        let held = [0x7f, b'E', b'L', b'F', 0xff, 0xfe];
        assert_eq!(
            content_on_machine(&held, SOMEBODY, "/usr/local/bin/launcher"),
            held.to_vec()
        );
        assert_eq!(content_as_declared(&held, SOMEBODY), held.to_vec());
    }

    #[test]
    fn a_file_names_every_directory_it_sits_inside() {
        assert_eq!(
            holding("/home/ada/.librewolf/console/chrome/userChrome.css"),
            [
                Path::new("/home"),
                Path::new("/home/ada"),
                Path::new("/home/ada/.librewolf"),
                Path::new("/home/ada/.librewolf/console"),
                Path::new("/home/ada/.librewolf/console/chrome"),
            ]
        );
        assert_eq!(holding("/etc/pamac.conf"), [Path::new("/etc")]);
    }

    #[test]
    fn a_directory_made_inside_a_home_belongs_to_whoever_lives_there() {
        let made = holding("/home/ada/.librewolf/console/chrome/userChrome.css");
        let owners: Vec<String> =
            made.iter().map(|dir| owner_of(&dir.to_string_lossy(), SOMEBODY)).collect();
        assert_eq!(owners, ["root", "root", SOMEBODY, SOMEBODY, SOMEBODY]);
    }

    #[test]
    fn anything_in_a_bin_directory_is_meant_to_be_run() {
        assert_eq!(mode_of("/usr/local/bin/console", b"any"), 0o755);
        assert_eq!(mode_of("/usr/sbin/thing", b""), 0o755);
    }

    #[test]
    fn a_script_and_a_compiled_program_are_both_meant_to_be_run() {
        assert_eq!(mode_of("/etc/thing", b"#!/bin/sh"), 0o755);
        assert_eq!(mode_of("/etc/thing", b"\x7fELF\x02"), 0o755);
    }

    #[test]
    fn a_compiled_program_is_read_past_its_first_two_bytes() {
        assert_eq!(mode_of("/etc/thing", b"\x7fELF"), 0o755);
        assert_eq!(mode_of("/etc/thing", b"\x7fEL"), 0o644);
    }

    #[test]
    fn everything_else_is_only_read() {
        assert_eq!(mode_of("/etc/systemd/user/console.target", b"[Uni"), 0o644);
        assert_eq!(mode_of("/home/@user@/.config/kdeglobals", b"[Col"), 0o644);
    }
}

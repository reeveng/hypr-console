//! Putting one file where the manifest says it goes.
//!
//! What the manifest says about a file is usually the whole of it: this content,
//! at this path, and anything else there is drift. `Written::Once` is the other
//! answer, and `manifest` argues for it -- the tree ships what the file starts
//! as, and something on the machine writes it afterwards and is supposed to. So
//! the file is asked whether it is there rather than what is in it, and
//! `State::WrittenOnce` is that answer said out loud: `console check` prints the word
//! beside the path, which is the difference between a file no one compares and a
//! file that happens to match today.

use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_words::Words;

use crate::manifest::Written;
use crate::up_to_date::UpToDate;


pub use console_manifest_migrations::USER;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct User<'a>(pub &'a str);

fn home_of(user: User<'_>) -> Result<String, Never> {
    Ok(format!("/home/{}/", user.0))
}

pub fn on_machine(live: &str, user: User<'_>) -> Result<String, Never> {
    let Ok(marked) = home_of(User(USER));
    let Ok(theirs) = home_of(user);

    moved(live, Homes { from: &marked, to: &theirs })
}

pub fn as_declared(live: &str, user: User<'_>) -> Result<String, Never> {
    let Ok(theirs) = home_of(user);
    let Ok(marked) = home_of(User(USER));

    moved(live, Homes { from: &theirs, to: &marked })
}

struct Homes<'a> {
    from: &'a str,
    to: &'a str,
}

fn moved(live: &str, homes: Homes<'_>) -> Result<String, Never> {
    Ok(match live.strip_prefix(homes.from) {
        Some(rest) => format!("{}{rest}", homes.to),
        None => live.to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum State {
    #[words(name = "ok")]
    Ok,
    #[words(name = "written once")]
    WrittenOnce,
    #[words(name = "differs")]
    Differs,
    #[words(name = "missing")]
    Missing,
    #[words(name = "cannot read")]
    Unreadable,
    #[words(name = "unsourced")]
    Unsourced,
}

impl State {
    pub fn up_to_date(self) -> Result<UpToDate, Never> {
        Ok(match self {
            State::Ok | State::WrittenOnce => UpToDate::Yes,
            State::Differs | State::Missing | State::Unreadable | State::Unsourced => UpToDate::No,
        })
    }
}

pub fn source_of(source: &Path, live: &str) -> Result<PathBuf, Never> {
    Ok(source.join(live.trim_start_matches('/')))
}

pub fn content_on_machine(held: &[u8], user: User<'_>, _live: &str) -> Result<Vec<u8>, Never> {
    let text = match std::str::from_utf8(held) {
        Ok(text) => text,
        Err(_not_text) => return Ok(held.to_vec()),
    };

    Ok(match text.contains(USER) {
        true => text.replace(USER, user.0).into_bytes(),
        false => held.to_vec(),
    })
}

pub fn content_as_declared(held: &[u8], user: User<'_>) -> Result<Vec<u8>, Never> {
    Ok(match std::str::from_utf8(held) {
        Ok(text) => match text.contains(user.0) {
            true => text.replace(user.0, USER).into_bytes(),
            false => held.to_vec(),
        },
        Err(_not_text) => held.to_vec(),
    })
}

pub fn state(source: &Path, live: &str, user: User<'_>, written: Written) -> Result<State, Never> {
    let Ok(on) = on_machine(live, user);
    let Ok(from) = source_of(source, live);
    let to = Path::new(&on);

    Ok(match (std::fs::read(&from), std::fs::read(to)) {
        (Err(_no_source), _) => State::Unsourced,
        (Ok(_), Err(fault)) => match fault.kind() == std::io::ErrorKind::PermissionDenied {
            true => match written {
                Written::Once => State::WrittenOnce,
                Written::Always => State::Unreadable,
            },
            false => State::Missing,
        },
        (Ok(held), Ok(there)) => match written {
            Written::Once => State::WrittenOnce,
            Written::Always => {
                let Ok(content) = content_on_machine(&held, user, live);

                match content == there {
                    true => State::Ok,
                    false => State::Differs,
                }
            }
        },
    })
}

pub fn owner_of(live: &str, user: User<'_>) -> Result<String, Never> {
    let Ok(marked) = home_of(User(USER));
    let Ok(theirs) = home_of(user);

    Ok(match live.starts_with(&marked) || live.starts_with(&theirs) {
        true => user.0.to_string(),
        false => "root".to_string(),
    })
}

pub fn parent_directories(live: &str) -> Result<Vec<PathBuf>, Never> {
    let mut directories: Vec<PathBuf> = Path::new(live)
        .parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .filter(|directory| !directory.as_os_str().is_empty() && *directory != Path::new("/"))
        .map(Path::to_path_buf)
        .collect();
    directories.reverse();

    Ok(directories)
}

#[cfg(test)]
mod tests {
    use super::*;

    type Failure = Box<dyn std::error::Error>;

    const SOMEONE: &str = "ada";

    const LIVING_HERE: User<'static> = User(SOMEONE);

    #[test]
    fn a_source_path_is_the_live_path_under_the_tree() {
        let source = Path::new("/etc/console/files");

        assert_eq!(
            source_of(source, "/usr/local/bin/launcher"),
            Ok(PathBuf::from("/etc/console/files/usr/local/bin/launcher"))
        );
    }

    fn laid_out((here, live): (&Path, &Path), held: (&[u8], &[u8])) -> Result<PathBuf, Failure> {
        let source = here.join("files");
        let Ok(at) = source_of(&source, &live.to_string_lossy());
        let above = at.parent().ok_or("a source with a directory above it")?;
        let (should_be, is) = held;

        std::fs::create_dir_all(above)?;
        console_core_atomic_writes::whole(&at, should_be)?;
        std::fs::create_dir_all(here.join("live"))?;
        console_core_atomic_writes::whole(live, is)?;

        Ok(source)
    }

    #[test]
    fn a_file_no_one_here_may_read_is_not_a_file_that_is_missing() -> Result<(), Failure> {
        use std::os::unix::fs::PermissionsExt;

        let here = console_core_temporary_directories::fresh("shut")?;
        let live = here.join("live/console");
        let source = laid_out((&here, &live), (b"what it should be\n", b"what it should be\n"))?;
        let at = live.to_string_lossy().to_string();

        assert_eq!(state(&source, &at, LIVING_HERE, Written::Always), Ok(State::Ok), "the same file, while it can be read");

        std::fs::set_permissions(here.join("live"), std::fs::Permissions::from_mode(0o000))?;

        let shut = std::fs::read(&live).map_err(|fault| fault.kind());
        let said = state(&source, &at, LIVING_HERE, Written::Always);
        let _ = std::fs::set_permissions(here.join("live"), std::fs::Permissions::from_mode(0o755));
        let _ = std::fs::remove_dir_all(&here);

        match shut {
            Ok(_read_anyway_as_root) => return Ok(()),
            Err(_refused) => {},
        }

        assert_eq!(said, Ok(State::Unreadable), "a file that cannot be read is not a file that is gone");
        assert_ne!(said, Ok(State::Missing));

        Ok(())
    }

    #[test]
    fn a_file_written_once_is_installed_when_it_is_gone_and_never_compared() -> Result<(), Failure> {
        let here = console_core_temporary_directories::fresh("once")?;
        let live = here.join("live/bar.css");
        let source = laid_out((&here, &live), (b"what it starts as\n", b"what the login wrote instead\n"))?;
        let at = live.to_string_lossy().to_string();

        assert_eq!(state(&source, &at, LIVING_HERE, Written::Always), Ok(State::Differs));
        assert_eq!(
            state(&source, &at, LIVING_HERE, Written::Once),
            Ok(State::WrittenOnce),
            "a file something else on the machine writes was read as drift"
        );

        std::fs::remove_file(&live)?;

        assert_eq!(
            state(&source, &at, LIVING_HERE, Written::Once),
            Ok(State::Missing),
            "nothing would have put it back"
        );

        let _ = std::fs::remove_dir_all(&here);

        Ok(())
    }

    #[test]
    fn a_file_no_one_compares_is_a_file_nothing_has_to_be_done_about() {
        let Ok(settled) = State::WrittenOnce.up_to_date();
        let Ok(name) = State::WrittenOnce.name();

        assert_eq!(settled, UpToDate::Yes);
        assert_eq!(name, "written once");
    }

    #[test]
    fn a_file_in_a_home_belongs_to_whoever_lives_there() {
        let whoever = Ok(SOMEONE.to_string());
        let root = Ok("root".to_string());

        assert_eq!(owner_of("/home/@user@/.config/console/hypr/hyprland.lua", LIVING_HERE), whoever);
        assert_eq!(owner_of("/home/ada/.config/console/hypr/hyprland.lua", LIVING_HERE), whoever);
        assert_eq!(owner_of("/etc/systemd/user/console.target", LIVING_HERE), root);
        assert_eq!(owner_of("/home/adam/.bashrc", LIVING_HERE), root);
        assert_eq!(owner_of("/home/someone/.bashrc", LIVING_HERE), root);
    }

    #[test]
    fn the_mark_is_filled_in_when_a_path_reaches_the_machine() {
        assert_eq!(
            on_machine("/home/@user@/.config/console/hypr/hyprland.lua", LIVING_HERE),
            Ok("/home/ada/.config/console/hypr/hyprland.lua".to_string())
        );
        assert_eq!(on_machine("/etc/pamac.conf", LIVING_HERE), Ok("/etc/pamac.conf".to_string()));
    }

    #[test]
    fn a_path_someone_typed_is_taken_back_to_the_mark() {
        assert_eq!(
            as_declared("/home/ada/.config/console/hypr/hyprland.lua", LIVING_HERE),
            Ok("/home/@user@/.config/console/hypr/hyprland.lua".to_string())
        );
        assert_eq!(as_declared("/etc/pamac.conf", LIVING_HERE), Ok("/etc/pamac.conf".to_string()));
    }

    #[test]
    fn a_path_taken_to_the_machine_and_back_is_the_path_it_was() {
        let declared = "/home/@user@/.librewolf/console/user.js";
        let Ok(there) = on_machine(declared, LIVING_HERE);

        assert_eq!(as_declared(&there, LIVING_HERE), Ok(declared.to_string()));
    }

    #[test]
    fn the_mark_is_filled_in_inside_a_file_as_well_as_in_its_name() {
        let held = b"@user@ ALL=(root) NOPASSWD: /usr/local/bin/console-engine\n";

        assert_eq!(
            content_on_machine(held, LIVING_HERE, "/etc/sudoers.d/console"),
            Ok(b"ada ALL=(root) NOPASSWD: /usr/local/bin/console-engine\n".to_vec())
        );
    }

    #[test]
    fn nothing_but_the_mark_is_filled_in() {
        let held = b"    source_event:\n      gamepad:\n        button: LeftPaddle1\n";

        for live in ["/etc/inputplumber/profiles/game.yaml", "/usr/local/bin/keyboard-toggle"] {
            assert_eq!(content_on_machine(held, LIVING_HERE, live), Ok(held.to_vec()));
        }
    }

    #[test]
    fn a_file_saved_off_the_machine_carries_the_mark_and_not_a_name() {
        let held = b"ada ALL=(root) NOPASSWD: /usr/local/bin/console-engine\n";

        assert_eq!(
            content_as_declared(held, LIVING_HERE),
            Ok(b"@user@ ALL=(root) NOPASSWD: /usr/local/bin/console-engine\n".to_vec())
        );
    }

    #[test]
    fn what_is_not_text_is_carried_through_untouched() {
        let held = [0x7f, b'E', b'L', b'F', 0xff, 0xfe];

        assert_eq!(content_on_machine(&held, LIVING_HERE, "/usr/local/bin/launcher"), Ok(held.to_vec()));
        assert_eq!(content_as_declared(&held, LIVING_HERE), Ok(held.to_vec()));
    }

    #[test]
    fn a_file_names_every_directory_it_sits_inside() {
        assert_eq!(
            parent_directories("/home/ada/.librewolf/console/chrome/userChrome.css"),
            Ok(vec![
                PathBuf::from("/home"),
                PathBuf::from("/home/ada"),
                PathBuf::from("/home/ada/.librewolf"),
                PathBuf::from("/home/ada/.librewolf/console"),
                PathBuf::from("/home/ada/.librewolf/console/chrome"),
            ])
        );
        assert_eq!(parent_directories("/etc/pamac.conf"), Ok(vec![PathBuf::from("/etc")]));
    }

    #[test]
    fn a_directory_made_inside_a_home_belongs_to_whoever_lives_there() {
        let Ok(made) = parent_directories("/home/ada/.librewolf/console/chrome/userChrome.css");
        let owners: Vec<String> = made
            .iter()
            .map(|directory| {
                let Ok(owner) = owner_of(&directory.to_string_lossy(), LIVING_HERE);

                owner
            })
            .collect();

        assert_eq!(owners, ["root", "root", SOMEONE, SOMEONE, SOMEONE]);
    }
}

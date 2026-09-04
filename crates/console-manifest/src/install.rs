//! Putting one file where the manifest says it goes.

use std::path::{Path, PathBuf};

use crate::settled::Settled;


/// The mark that stands for whoever this desktop belongs to.
///
/// The manifest does not name them. This source is published, and a stranger
/// who installs it should get their own home rather than a second person's;
/// the person whose machine this is should not be named in a repository
/// either. So the manifest and the tree both write the mark, and the machine
/// says who it stands for at the moment a file is actually laid down.
pub const USER: &str = "@user@";

/// A home directory, as a prefix to match a path against.
fn home_of(user: &str) -> String {
    format!("/home/{user}/")
}

/// A declared path, as the machine has it.
///
/// The manifest and the tree both say `/home/@user@/...`; the machine has a
/// home with somebody's name on it. This is the one place the two are joined,
/// and it is asked for whenever a path is about to be read, written or owned.
pub fn on_machine(live: &str, user: &str) -> String {
    match live.strip_prefix(&home_of(USER)) {
        Some(rest) => format!("{}{rest}", home_of(user)),
        None => live.to_string(),
    }
}

/// A path on the machine, as the manifest declares it.
///
/// The other way round, for a path a person has typed: somebody chasing a
/// fault says the file they were just editing, which is the one with their own
/// name in it, and the tree it has to be saved into is under the mark.
pub fn as_declared(live: &str, user: &str) -> String {
    match live.strip_prefix(&home_of(user)) {
        Some(rest) => format!("{}{rest}", home_of(USER)),
        None => live.to_string(),
    }
}

/// How a file on the machine stands against the source it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The machine has it and it is what the source says.
    Ok,
    /// The machine has it and it is not what the source says.
    Differs,
    /// The source has it and the machine does not.
    Missing,
    /// The machine has it and whoever is asking may not read it.
    ///
    /// Not a fault and not drift: it is a question this process cannot answer.
    /// `/etc/sudoers.d` is `drwxr-x---` and belongs to root, which is exactly
    /// right and means a check running as the person using the machine cannot
    /// open the one file in it this desktop writes. Read as `Missing`, that
    /// was a card at every boot and every hour after saying the machine had
    /// drifted, on a machine that was perfectly well -- which is the failure
    /// `well` exists to avoid rather than to cause.
    Unreadable,
    /// The manifest names it and nothing holds its content.
    Unsourced,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            State::Ok => "ok",
            State::Differs => "differs",
            State::Missing => "missing",
            State::Unreadable => "cannot read",
            State::Unsourced => "unsourced",
        }
    }

    pub fn settled(self) -> Settled {
        match self == State::Ok {
            true => Settled::Yes,
            false => Settled::No,
        }
    }
}

/// Where under the source tree the content of a live path is kept.
///
/// `/usr/local/bin/launcher` is `files/usr/local/bin/launcher`. There is
/// nothing to keep in step, because the one path is the other.
pub fn source_of(source: &Path, live: &str) -> PathBuf {
    source.join(live.trim_start_matches('/'))
}

/// A source file's content, as the machine should hold it.
///
/// Two of the files here name the desktop user inside themselves rather than
/// only in their path: the sudoers line saying what they may run as root, and
/// the udev rule handing them the touchpad. A mark is not a user to either of
/// those programs, so it is filled in on the way to the machine exactly as it
/// is filled in in a path.
///
/// Only text is rewritten. The rest of what this lays down is compiled
/// programs, which are carried byte for byte and never looked inside.
///
/// Nothing else is filled in. A profile used to be, because it was written in
/// the buttons of the device this desktop grew on and a device with its buttons
/// elsewhere needed them moved on the way past. The profile this desktop is
/// driven by is not in the tree at all now -- it is made out of the machine it
/// is for -- so there is nothing here to move.
pub fn content_on_machine(held: &[u8], user: &str, _live: &str) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(held) else { return held.to_vec() };

    match text.contains(USER) {
        true => text.replace(USER, user).into_bytes(),
        false => held.to_vec(),
    }
}

/// The other way, for a file edited on the machine and saved back.
///
/// Without this a `console save` of the sudoers file would carry a name into
/// the tree, which is the one thing the mark is for.
pub fn content_as_declared(held: &[u8], user: &str) -> Vec<u8> {
    match std::str::from_utf8(held) {
        Ok(text) if text.contains(user) => text.replace(user, USER).into_bytes(),
        _ => held.to_vec(),
    }
}

/// One of missing, differs, ok, or unsourced.
///
/// The source is compared as the machine should hold it, so a file whose mark
/// has been filled in is settled rather than for ever drifting.
pub fn state(source: &Path, live: &str, user: &str) -> State {
    let on = on_machine(live, user);
    let (from, to) = (source_of(source, live), Path::new(&on));

    match (std::fs::read(&from), std::fs::read(to)) {
        (Err(_), _) => State::Unsourced,
        // Not allowed to look is not the same answer as not there, and it used
        // to be. Both come back as an error from one call, and taking either
        // for the other means a root-only file is reported as gone from a
        // machine that has it.
        (Ok(_), Err(fault)) if fault.kind() == std::io::ErrorKind::PermissionDenied => {
            State::Unreadable
        }
        (Ok(_), Err(_)) => State::Missing,
        (Ok(held), Ok(there)) if content_on_machine(&held, user, live) == there => State::Ok,
        (Ok(_), Ok(_)) => State::Differs,
    }
}

/// Files under the desktop user's home belong to them. Everything else is
/// root's.
///
/// Asked of a path in either spelling, because the directories a file sits
/// inside are worked out from the path the machine has and the file itself is
/// named the way the manifest declares it.
pub fn owner_of(live: &str, user: &str) -> String {
    match live.starts_with(&home_of(USER)) || live.starts_with(&home_of(user)) {
        true => user.to_string(),
        false => "root".to_string(),
    }
}

/// Every directory a file sits inside, outermost first.
///
/// A directory made for somebody's file has to belong to them as well.
/// `create_dir_all` leaves what it makes owned by whoever ran it, which is
/// root, and a profile directory the browser cannot write into is a browser
/// that cannot start: it asks which profile to use and then cannot save the
/// answer. Naming the directories one at a time means each can be given the
/// owner `owner_of` already decides for any path.
pub fn holding(live: &str) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Path::new(live)
        .parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .filter(|dir| !dir.as_os_str().is_empty() && *dir != Path::new("/"))
        .map(Path::to_path_buf)
        .collect();
    dirs.reverse();
    dirs
}

/// Whether a file is meant to be run.
///
/// A script says so with a shebang and a compiled program with its own magic
/// number, and anything kept in a bin directory is there to be run whatever it
/// is made of. Reading only the first two bytes got this wrong for a compiled
/// program, which was then installed unreadable to the kernel and refused to
/// start with nothing but "permission denied" to say why.
pub fn mode_of(live: &str, head: &[u8]) -> u32 {
    match live {
        path if path.contains("/bin/") || path.contains("/sbin/") => 0o755,
        _ => match head {
            [b'#', b'!', ..] => 0o755,
            [0x7f, b'E', b'L', b'F', ..] => 0o755,
            _ => 0o644,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_path_is_the_live_path_under_the_tree() {
        let source = Path::new("/etc/console/files");
        assert_eq!(
            source_of(source, "/usr/local/bin/launcher"),
            Path::new("/etc/console/files/usr/local/bin/launcher")
        );
    }

    /// Whoever the machine running the tests belongs to, which is not the
    /// machine this describes and does not need to be.
    const SOMEBODY: &str = "ada";

    /// The one that put a card on somebody's screen every hour: a file the
    /// machine has and the person checking is not allowed to open.
    ///
    /// This is not an edge case on this desktop. `/etc/sudoers.d` is
    /// `drwxr-x---` and root's, `console well` is a user unit, and the one
    /// file this tree keeps in there could never be read from where the check
    /// runs. Read as `Missing`, it said the machine had drifted at every boot
    /// and every hour after, for ever, on a machine that was perfectly well.
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

        // Shut, the way /etc/sudoers.d is. Root can read it anyway and would
        // rightly get the honest answer, so the test says nothing on a run
        // that turns out to be root's.
        std::fs::set_permissions(here.join("live"), std::fs::Permissions::from_mode(0o000))
            .expect("shut");
        let shut = std::fs::read(&live).is_err();
        let said = state(&source, &live.to_string_lossy(), SOMEBODY);
        std::fs::set_permissions(here.join("live"), std::fs::Permissions::from_mode(0o755)).ok();
        std::fs::remove_dir_all(&here).ok();

        if !shut {
            return;
        }

        assert_eq!(said, State::Unreadable, "a file that cannot be read is not a file that is gone");
        assert_ne!(said, State::Missing);
    }

    #[test]
    fn a_file_in_a_home_belongs_to_whoever_lives_there() {
        assert_eq!(owner_of("/home/@user@/.config/hypr/hyprland.lua", SOMEBODY), SOMEBODY);
        assert_eq!(owner_of("/home/ada/.config/hypr/hyprland.lua", SOMEBODY), SOMEBODY);
        assert_eq!(owner_of("/etc/systemd/user/console.target", SOMEBODY), "root");
        // Not any home, and not a name that merely starts the same way.
        assert_eq!(owner_of("/home/adam/.bashrc", SOMEBODY), "root");
        assert_eq!(owner_of("/home/someone/.bashrc", SOMEBODY), "root");
    }

    #[test]
    fn the_mark_is_filled_in_when_a_path_reaches_the_machine() {
        assert_eq!(
            on_machine("/home/@user@/.config/hypr/hyprland.lua", SOMEBODY),
            "/home/ada/.config/hypr/hyprland.lua"
        );
        // Everything outside a home is the same path on both sides.
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

    /// The two are the same journey in opposite directions, and a path that
    /// does not come back is a file saved into the wrong place in the tree.
    #[test]
    fn a_path_taken_to_the_machine_and_back_is_the_path_it_was() {
        let declared = "/home/@user@/.librewolf/console/user.js";
        assert_eq!(as_declared(&on_machine(declared, SOMEBODY), SOMEBODY), declared);
    }

    /// The sudoers line is the reason this exists: a mark is not a user to
    /// sudo, and a file naming one that does not exist is a file it refuses.
    #[test]
    fn the_mark_is_filled_in_inside_a_file_as_well_as_in_its_name() {
        let held = b"@user@ ALL=(root) NOPASSWD: /usr/local/bin/console-engine\n";
        assert_eq!(
            content_on_machine(held, SOMEBODY, "/etc/sudoers.d/console"),
            b"ada ALL=(root) NOPASSWD: /usr/local/bin/console-engine\n".to_vec()
        );
    }

    /// The mark, and nothing else. A profile used to be rewritten on the way
    /// past, because it was written in the buttons of the device this desktop
    /// grew on; the profile is made out of the machine it is for now, and no
    /// file in the tree is changed by whose machine it is going to beyond the
    /// name of the person who owns it.
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

    /// A compiled program holds bytes that are not text, and looking inside one
    /// for a mark would be looking inside every program this installs.
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
        // The fault this answers: the browser's profile directory was made by
        // root, so the browser could not write the profile it was given.
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
        // Reading only two bytes installed a binary unreadable to the kernel,
        // which then refused to start with nothing but "permission denied".
        assert_eq!(mode_of("/etc/thing", b"\x7fELF"), 0o755);
        assert_eq!(mode_of("/etc/thing", b"\x7fEL"), 0o644);
    }

    #[test]
    fn everything_else_is_only_read() {
        assert_eq!(mode_of("/etc/systemd/user/console.target", b"[Uni"), 0o644);
        assert_eq!(mode_of("/home/@user@/.config/kdeglobals", b"[Col"), 0o644);
    }
}

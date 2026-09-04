//! What the crates shell out to, held against what the manifest names.
//!
//! Three packages went missing in one evening -- pipewire-audio for
//! `pw-record`, libnotify for `notify-send`, libpulse for `pactl` -- and every
//! one of them worked anyway, because something else on the machine had
//! dragged it in. A dependency that is only true by accident is true until the
//! day somebody removes the thing it came with, and then a button does nothing
//! and there is no terminal in front of the person holding it.
//!
//! The units and the scripts were already held to this: `the_tree` reads a
//! unit's ExecStart and a script's words. Nothing read a crate's. So this is
//! the same rule for the third kind of thing that runs a program.
//!
//! `PROGRAMS` is the answer and the scan below is the net under it. The net
//! cannot see everything: an argv built a word at a time is not a literal, and
//! a program nobody has installed on the machine running this cannot be told
//! from an ordinary word. What it does catch is the common shape -- a literal
//! at the front of an argv -- which is how all three of those were written.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Where a program comes from, and so whether the manifest has to say it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum From_ {
    /// A package, which `[packages]` has to name.
    Package(&'static str),
    /// Any Arch has it: coreutils, bash, systemd, util-linux, pacman.
    /// Naming these would be naming the operating system.
    Base,
    /// Only a machine that develops this runs it. The device does not.
    Here,
}

/// Every program the crates run, and where it comes from.
///
/// Adding a `Command::new` or an argv to a crate means adding a line here. The
/// scan below fails until you do, which is the point of it.
const PROGRAMS: &[(&str, From_)] = &[
    ("Hyprland", From_::Here),
    ("alacritty", From_::Package("alacritty")),
    ("awww", From_::Package("awww")),
    ("awww-daemon", From_::Package("awww")),
    ("bluetoothctl", From_::Package("bluez-utils")),
    ("busctl", From_::Base),
    ("cargo", From_::Package("rust")),
    // coreutils, and it is here for what it does not do rather than for what
    // it does. `systemd-inhibit` holds a lock for as long as what it runs is
    // running, so an apply needs it to run something that waits and ends when
    // this process does. `cat` on a pipe this process holds is that: it reads
    // end-of-file when the pipe closes, however the apply ended.
    ("cat", From_::Base),
    ("cp", From_::Base),
    ("cmake", From_::Package("cmake")),
    ("curl", From_::Package("curl")),
    // coreutils. A moment in local time, written most-significant first, is a
    // timezone database and a calendar, and neither is worth carrying here to
    // name a screenshot.
    ("date", From_::Base),
    ("echo", From_::Base),
    ("env", From_::Base),
    ("ffmpeg", From_::Package("ffmpeg")),
    ("ffprobe", From_::Package("ffmpeg")),
    ("gio", From_::Package("glib2")),
    ("git", From_::Package("git")),
    ("grim", From_::Package("grim")),
    ("hostname", From_::Here),
    ("hyprctl", From_::Package("hyprland")),
    // The colour of the screen through the evening. console-warm.service is
    // what starts it; nothing in the crates runs it, and it is named here
    // because the words this desktop must never put on a screen are a list
    // that has to say its name to keep it off them.
    ("hyprsunset", From_::Package("hyprsunset")),
    ("id", From_::Base),
    // util-linux, and so the operating system. It reaches the journal with a
    // tag on it, which is what `journalctl -t console` is asking after.
    ("logger", From_::Base),
    ("makoctl", From_::Package("mako")),
    ("mkdir", From_::Base),
    ("mv", From_::Base),
    ("nmcli", From_::Package("networkmanager")),
    ("notify-send", From_::Package("libnotify")),
    ("pacman", From_::Base),
    ("pactl", From_::Package("libpulse")),
    // procps-ng, and so the operating system. It is how the engine wakes the
    // desktop's bar when an apply moves on: waybar is the user's and the apply
    // is root's, and a signal is the only thing that crosses that.
    ("pkill", From_::Base),
    ("powerprofilesctl", From_::Package("power-profiles-daemon")),
    ("pw-record", From_::Package("pipewire-audio")),
    // util-linux. `console apply` is root, and the one thing it does inside the
    // browser's profile has to be done as her or the directory it makes is
    // root's and the browser cannot write its own add-ons into it.
    ("runuser", From_::Base),
    ("scp", From_::Here),
    ("sh", From_::Base),
    ("ssh", From_::Here),
    ("stdbuf", From_::Base),
    ("su", From_::Base),
    ("sudo", From_::Package("sudo")),
    ("systemctl", From_::Base),
    // The apply's promise from the manager that nothing will stop the machine
    // partway through. Held for the length of one apply, beside the lock in
    // `alone` and for the same span.
    ("systemd-inhibit", From_::Base),
    ("systemd-run", From_::Base),
    ("true", From_::Base),
    ("whisper-cli", From_::Package("whisper-cpp")),
    ("wpctl", From_::Package("wireplumber")),
    ("wtype", From_::Package("wtype")),
    ("xdg-mime", From_::Package("xdg-utils")),
    ("xdg-open", From_::Package("xdg-utils")),
    ("xdg-settings", From_::Package("xdg-utils")),
    ("yt-dlp", From_::Package("yt-dlp")),
];

/// Words that are a program's name somewhere else and not one here.
///
/// Each of these is a subcommand or a fixture that happens to collide with
/// something in PATH: `awww clear`, `systemctl start`, the package name
/// `hyprland` in a manifest fixture, and `test` in a leak check's example.
/// The scan cannot tell them from a program by looking, so they are named.
const NOT_PROGRAMS: &[&str] = &["clear", "hyprland", "start", "test"];

fn root() -> PathBuf {
    {
    // Tidied by `canonicalize` where that works and left as it stands where it
    // does not. What `CARGO_MANIFEST_DIR` gives is already absolute and already
    // right; canonicalizing only takes the `../..` out of the middle. It fails
    // under a sandbox that will not let a process resolve a path it can
    // otherwise read, and a test that stops there reports the sandbox as a
    // missing repository.
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

fn manifest() -> String {
    std::fs::read_to_string(root().join("desktop.conf")).expect("desktop.conf")
}

fn section(held: &str, wanted: &str) -> Vec<String> {
    held.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .fold((Vec::new(), None), |(mut out, at), line| {
            match line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
                Some(name) => (out, Some(name.to_string())),
                None => {
                    if at.as_deref() == Some(wanted) {
                        out.push(line.to_string());
                    }
                    (out, at)
                }
            }
        })
        .0
}

/// Every `.rs` file the workspace's own crates are made of.
fn sources() -> Vec<PathBuf> {
    fn walk(at: &Path, into: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(at) else { return };
        for path in entries.flatten().map(|entry| entry.path()) {
            match path {
                path if path.is_dir() => walk(&path, into),
                path if path.extension().is_some_and(|end| end == "rs") => into.push(path),
                _ => {}
            }
        }
    }
    let mut found = Vec::new();
    let Ok(crates) = std::fs::read_dir(root().join("crates")) else { return found };
    for crate_ in crates.flatten().map(|entry| entry.path()) {
        walk(&crate_.join("src"), &mut found);
    }
    found.sort();
    found
}

/// The string literals that could be the front of an argv.
///
/// `Command::new("x")`, and the first literal in any array or vec, which is
/// how every argv in this workspace is written.
fn front_words(said: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut push = |rest: &str| {
        if let Some(word) = rest.strip_prefix('"').and_then(|rest| rest.split('"').next()) {
            found.insert(word.to_string());
        }
    };
    for (at, _) in said.match_indices("Command::new(") {
        push(&said[at + "Command::new(".len()..]);
    }
    for (at, _) in said.match_indices('[') {
        push(said[at + 1..].trim_start());
    }
    found
}

/// Whether this machine has a program by that name, which is how a word is
/// told from a program without a list of every word in the language.
fn on_the_path(name: &str) -> bool {
    if name.contains('/') {
        return false; // a path, and so already its own answer about where it is
    }
    let Ok(path) = std::env::var("PATH") else { return false };
    path.split(':').any(|at| Path::new(at).join(name).is_file())
}

/// What the crates run, as far as looking at them can say.
fn reached_for() -> BTreeSet<String> {
    sources()
        .iter()
        .filter_map(|at| std::fs::read_to_string(at).ok())
        .flat_map(|said| front_words(&said))
        .filter(|word| !NOT_PROGRAMS.contains(&word.as_str()))
        .filter(|word| on_the_path(word))
        .collect()
}

/// Ours: built from this workspace, or installed by it as a script.
fn ours() -> BTreeSet<String> {
    let built = section(&manifest(), "build").into_iter();
    let carried = std::fs::read_dir(root().join("files/usr/local/bin"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok());
    built.chain(carried).collect()
}

/// The rule. A package a crate reaches for is a package the manifest names, or
/// it is on the machine by luck and goes the day that luck runs out.
#[test]
fn every_package_a_crate_reaches_for_is_in_the_manifest() {
    let held = manifest();
    let packages: BTreeSet<String> = section(&held, "packages").into_iter().collect();
    let missing: Vec<&str> = PROGRAMS
        .iter()
        .filter_map(|(_, from)| match from {
            From_::Package(named) => Some(*named),
            _ => None,
        })
        .filter(|named| !packages.contains(*named))
        .collect();
    assert!(missing.is_empty(), "[packages] does not name: {missing:?}");
}

/// The net. Anything that looks like a program and is not written down is
/// either a new dependency nobody declared or a word that needs naming in
/// NOT_PROGRAMS -- and both of those are things to decide rather than to miss.
#[test]
fn every_program_the_crates_run_is_written_down() {
    let known: BTreeSet<&str> = PROGRAMS.iter().map(|(name, _)| *name).collect();
    let ours = ours();
    let strange: Vec<String> = reached_for()
        .into_iter()
        .filter(|name| !known.contains(name.as_str()))
        .filter(|name| !ours.contains(name))
        .collect();
    assert!(
        strange.is_empty(),
        "these are run and not written down in PROGRAMS: {strange:?}"
    );
}

/// And the other way, so the table does not rot into a list of things this
/// stopped doing years ago.
#[test]
fn nothing_written_down_has_stopped_being_used() {
    let said: String = sources()
        .iter()
        .filter_map(|at| std::fs::read_to_string(at).ok())
        .collect::<Vec<_>>()
        .join("\n");
    let gone: Vec<&str> = PROGRAMS
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !said.contains(&format!("\"{name}\"")))
        .collect();
    assert!(gone.is_empty(), "PROGRAMS names what nothing runs: {gone:?}");
}

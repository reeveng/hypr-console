//! Everything that touches the machine itself.
//!
//! Kept apart from the rest so that what decides and what does are two files.
//! Nothing here can be tested without a machine, which is exactly why nothing
//! here decides anything.

use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use crate::install::{self, USER};

/// What a command said.
pub struct Said {
    pub out: String,
}

/// A command whose answer is wanted and whose failure is an answer too.
///
/// `systemctl is-active` says "inactive" and fails, and both halves of that
/// are the same fact, so what it said is kept and how it exited is not.
pub fn run(argv: &[&str]) -> Said {
    match Command::new(argv[0]).args(&argv[1..]).output() {
        Ok(done) => Said { out: String::from_utf8_lossy(&done.stdout).trim().to_owned() },
        Err(_) => Said { out: String::new() },
    }
}

/// A command whose output the person running this should see.
pub fn run_seen(argv: &[&str]) -> bool {
    Command::new(argv[0])
        .args(&argv[1..])
        .status()
        .map(|done| done.success())
        .unwrap_or(false)
}

/// Whoever this desktop belongs to, asked of the machine rather than named.
///
/// The manifest writes `@user@` and this is the answer to it. There is one
/// person on this device and their home is the one directory in `/home`, which
/// is the plainest thing there is to ask and stays true on a machine that is
/// somebody else's. A machine with several homes is asked for the first
/// account it made instead, because that is the one a desktop is installed for.
///
/// Asked once and kept. Every file laid down wants the answer, none of them can
/// change it, and a process for each would be a process for each.
///
/// A machine that will not say hands back the mark itself, so the fault a file
/// then fails with names the thing that is actually wrong: there is nobody for
/// `@user@` to stand for.
pub fn whoever() -> &'static str {
    static KNOWN: OnceLock<String> = OnceLock::new();
    KNOWN.get_or_init(|| {
        the_one_home().or_else(the_first_account).unwrap_or_else(|| USER.to_string())
    })
}

/// The one home in `/home`, if there is exactly one and somebody answers to it.
fn the_one_home() -> Option<String> {
    let mut homes: Vec<String> = std::fs::read_dir("/home")
        .ok()?
        .flatten()
        .filter(|found| found.path().is_dir())
        .filter_map(|found| found.file_name().into_string().ok())
        .filter(|name| who(name).is_some())
        .collect();
    match homes.len() {
        1 => homes.pop(),
        _ => None,
    }
}

/// The first account the machine made, which is the one a desktop is for.
fn the_first_account() -> Option<String> {
    let said = run(&["id", "-nu", "1000"]).out;
    match said.is_empty() {
        true => None,
        false => Some(said),
    }
}

/// systemctl for the desktop user's own manager, from a root shell.
pub fn user_systemctl(args: &[&str]) -> Said {
    let owned = format!("{}@", whoever());
    let argv: Vec<&str> = ["systemctl", "--user", "-M", &owned]
        .into_iter()
        .chain(args.iter().copied())
        .collect();
    run(&argv)
}

/// Run something in the desktop's own session, from root.
///
/// `su` hands over the account and leaves the environment where it was, so a
/// notification sent by a command started that way goes to root's bus, where
/// nothing is listening and nobody is looking. The session is named here out
/// of the account's own uid, which is the one thing root has to be told to
/// speak into somebody else's desktop.
///
/// What it says is thrown away. Everything this is used for is worth saying
/// and not worth stopping for, and the journal has the rest.
pub fn in_the_session(command: &str) {
    let owner = whoever();
    let Some((uid, _)) = who(owner) else { return };
    // `env` rather than a `VAR=value` prefix, and a shell named rather than
    // taken: `su` runs the account's login shell, which on this device is
    // fish, and what a shell makes of a line written for sh is the shell's
    // own business. Neither of those is worth finding out about through a
    // notification that silently never appeared.
    let line = format!(
        "env XDG_RUNTIME_DIR=/run/user/{uid} \
         DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{uid}/bus {command}"
    );
    run(&["su", owner, "-s", "/bin/sh", "-c", &line]);
}

pub fn unit_state(unit: &str) -> (String, String) {
    (
        user_systemctl(&["is-enabled", unit]).out,
        user_systemctl(&["is-active", unit]).out,
    )
}

pub fn installed_packages() -> Vec<String> {
    named(&["pacman", "-Qq"])
}

/// The packages somebody asked for, as against the ones that came in behind
/// them. pacman keeps only these when the orphans are swept.
pub fn wanted_packages() -> Vec<String> {
    named(&["pacman", "-Qeq"])
}

fn named(argv: &[&str]) -> Vec<String> {
    run(argv).out.split_whitespace().map(str::to_owned).collect()
}

/// Write beside the destination, then move it into place.
///
/// Writing over the destination fails outright for a program that is running:
/// the kernel refuses to alter a file being executed. A rename replaces the
/// name rather than the file, which the running copy holds on to until it
/// exits. It is also all-or-nothing, so an interrupted apply cannot leave half
/// a file behind.
pub fn install_file(from: &Path, live: &str) -> Result<(), String> {
    let on = install::on_machine(live, whoever());
    let to = Path::new(&on);
    let complain = |what: &str, fault: std::io::Error| format!("{live}: {what}: {fault}");

    for holding in install::holding(&on) {
        if holding.is_dir() {
            continue;
        }
        std::fs::create_dir(&holding).map_err(|fault| complain("its directory", fault))?;
        let owner = install::owner_of(&holding.to_string_lossy(), whoever());
        let (uid, gid) = who(&owner).ok_or_else(|| format!("{live}: no user called {owner}"))?;
        std::os::unix::fs::chown(&holding, Some(uid), Some(gid))
            .map_err(|fault| complain("its directory's owner", fault))?;
    }
    let staged = to.with_file_name(format!(
        "{}.console-new",
        to.file_name().and_then(|name| name.to_str()).unwrap_or("file")
    ));
    // Read and written rather than copied, because the mark standing for
    // whoever this desktop belongs to is filled in on the way past. A file
    // that holds no mark comes out the same bytes it went in as.
    let held = std::fs::read(from).map_err(|fault| complain("reading it", fault))?;
    std::fs::write(&staged, install::content_on_machine(&held, whoever()))
        .map_err(|fault| complain("writing", fault))?;

    let mode = install::mode_of(live, &install::head_of(from));
    std::fs::set_permissions(&staged, permissions(mode))
        .map_err(|fault| complain("its mode", fault))?;

    let owner = install::owner_of(live, whoever());
    let (uid, gid) = who(&owner).ok_or_else(|| format!("{live}: no user called {owner}"))?;
    std::os::unix::fs::chown(&staged, Some(uid), Some(gid))
        .map_err(|fault| complain("its owner", fault))?;

    std::fs::rename(&staged, to).map_err(|fault| complain("moving it into place", fault))
}

fn permissions(mode: u32) -> std::fs::Permissions {
    use std::os::unix::fs::PermissionsExt;
    std::fs::Permissions::from_mode(mode)
}

/// A user's numbers, asked of the system rather than read out of a file, so
/// that a machine which keeps its users somewhere else still answers.
fn who(user: &str) -> Option<(u32, u32)> {
    let number = |flag: &str| run(&["id", flag, user]).out.parse::<u32>().ok();
    Some((number("-u")?, number("-g")?))
}

/// Keep a history, so a desktop that breaks can be walked back.
pub fn commit(root: &Path, what: &str) {
    if !root.join(".git").exists() {
        return;
    }
    let root = root.display().to_string();
    run(&["git", "-C", &root, "add", "-A"]);
    if !run(&["git", "-C", &root, "status", "--porcelain"]).out.is_empty() {
        run(&["git", "-C", &root, "commit", "-m", what]);
    }
}

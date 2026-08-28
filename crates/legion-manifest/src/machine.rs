//! Everything that touches the machine itself.
//!
//! Kept apart from the rest so that what decides and what does are two files.
//! Nothing here can be tested without a machine, which is exactly why nothing
//! here decides anything.

use std::path::Path;
use std::process::Command;

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

/// systemctl for the desktop user's own manager, from a root shell.
pub fn user_systemctl(args: &[&str]) -> Said {
    let owned = format!("{USER}@");
    let argv: Vec<&str> = ["systemctl", "--user", "-M", &owned]
        .into_iter()
        .chain(args.iter().copied())
        .collect();
    run(&argv)
}

pub fn unit_state(unit: &str) -> (String, String) {
    (
        user_systemctl(&["is-enabled", unit]).out,
        user_systemctl(&["is-active", unit]).out,
    )
}

pub fn installed_packages() -> Vec<String> {
    run(&["pacman", "-Qq"]).out.split_whitespace().map(str::to_owned).collect()
}

/// Write beside the destination, then move it into place.
///
/// Writing over the destination fails outright for a program that is running:
/// the kernel refuses to alter a file being executed. A rename replaces the
/// name rather than the file, which the running copy holds on to until it
/// exits. It is also all-or-nothing, so an interrupted apply cannot leave half
/// a file behind.
pub fn install_file(from: &Path, live: &str) -> Result<(), String> {
    let to = Path::new(live);
    let complain = |what: &str, fault: std::io::Error| format!("{live}: {what}: {fault}");

    if let Some(holding) = to.parent() {
        std::fs::create_dir_all(holding).map_err(|fault| complain("its directory", fault))?;
    }
    let staged = to.with_file_name(format!(
        "{}.legion-new",
        to.file_name().and_then(|name| name.to_str()).unwrap_or("file")
    ));
    std::fs::copy(from, &staged).map_err(|fault| complain("copying", fault))?;

    let mode = install::mode_of(live, &install::head_of(from));
    std::fs::set_permissions(&staged, permissions(mode))
        .map_err(|fault| complain("its mode", fault))?;

    let owner = install::owner_of(live);
    let (uid, gid) = who(owner).ok_or_else(|| format!("{live}: no user called {owner}"))?;
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

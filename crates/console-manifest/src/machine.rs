//! Everything that touches the machine itself.
//!
//! Kept apart from the rest so that what decides and what does are two files.
//! Nothing here can be tested without a machine, which is exactly why nothing
//! here decides anything.

use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use crate::install::{self, USER};
use crate::laying::{self, Back, Laid};

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

/// Whether a command run in front of somebody came back clean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ran {
    /// It ran and it succeeded.
    Fine,
    /// It failed, or it would not start at all.
    Badly,
}

/// A command whose output the person running this should see.
pub fn run_seen(argv: &[&str]) -> Ran {
    // `Badly` is written to mean both of these: a command that ran and failed
    // and one that would not start are the same news to whoever asked for it.
    let done = match Command::new(argv[0]).args(&argv[1..]).status() {
        Ok(done) => done.success(),
        Err(_) => false,
    };

    match done {
        true => Ran::Fine,
        false => Ran::Badly,
    }
}

/// This desktop's own directory in whoever's home it is.
///
/// What tells one home from another on a machine with more than one. Named
/// here and laid down by the manifest, and a test holds the two together so
/// they cannot drift into different answers.
const OURS: &str = ".config/console";

/// Whoever this desktop belongs to, asked of the machine rather than named.
///
/// The manifest writes `@user@` and this is the answer to it, so this decides
/// the path of every file laid down, the content of the two that name the user
/// inside themselves, who owns each one, and the account every
/// `machinectl shell --uid=` speaks as. It is as load-bearing as anything here.
///
/// Three questions, in the order they deserve to be trusted.
///
/// One home in `/home` with an account behind it is the plainest thing there is
/// to ask, and it is the whole answer on a machine with one person on it.
///
/// Otherwise, the home this desktop is already in. A handheld gets a second
/// account the ordinary way -- somebody who sets it up keeps a login on it --
/// and the two are told apart by which one the desktop was installed into,
/// which is a fact about this desktop rather than about how the machine was
/// numbered.
///
/// Only then the account numbered 1000, and only saying so. That is the first
/// apply on a machine where nothing has been laid down yet, so there is no
/// desktop to find and no better question to ask. It is a guess, it is usually
/// right, and what it must not be is quietly wrong: a device set up for
/// somebody after a technician's own account would get files under the wrong
/// home, owned by the wrong person, with every check speaking as them.
///
/// This was the first and the third with the middle one missing, and the third
/// was written down as "the first account the machine made" while being
/// `id -nu 1000`. Those are the same on almost every machine and they are not
/// the same fact. The device has had two homes for as long as it has had a
/// second account, so the plain rule stopped applying and the guess has been
/// answering ever since, without saying.
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
        the_one_home()
            .or_else(the_home_it_is_already_in)
            .or_else(the_account_numbered_1000)
            .unwrap_or_else(|| USER.to_string())
    })
}

/// Every directory in `/home` with an account of that name behind it.
fn homes() -> Vec<String> {
    let Ok(reading) = std::fs::read_dir("/home") else { return Vec::new() };

    reading
        .flatten()
        .filter(|found| found.path().is_dir())
        // A directory whose name is not text is not a user this desktop can
        // act on: every account it knows arrives as a string, from `id` and
        // from the manifest. Passed over rather than guessed at.
        .filter_map(|found| match found.file_name().into_string() {
            Ok(name) => Some(name),

            Err(name) => {
                eprintln!("console: /home/{}: not a name this desktop can act on", name.to_string_lossy());
                None
            }
        })
        .filter(|name| who(name).is_some())
        .collect()
}

/// The one home in `/home`, if there is exactly one and somebody answers to it.
fn the_one_home() -> Option<String> {
    let mut homes = homes();

    match homes.len() {
        1 => homes.pop(),
        _ => None,
    }
}

/// The home this desktop has already been laid down in.
///
/// Only where exactly one of them has it. Two homes both holding a console
/// directory is a machine this cannot answer for, and answering anyway would
/// pick by the order `/home` happened to be read in, which is the same kind of
/// wrong as picking by the order the accounts were numbered.
fn the_home_it_is_already_in() -> Option<String> {
    let mut theirs: Vec<String> = homes()
        .into_iter()
        .filter(|name| Path::new("/home").join(name).join(OURS).is_dir())
        .collect();

    match theirs.len() {
        1 => theirs.pop(),
        _ => None,
    }
}

/// The account numbered 1000, said out loud.
///
/// The last thing asked and the only one that is a guess. Nothing has been laid
/// down yet, so there is no desktop to find; what is left is the number almost
/// every machine gives the account it was set up for. Said rather than assumed,
/// because a wrong answer here is a whole desktop installed into a stranger's
/// home, and the moment it is printed is the only moment anybody could catch
/// it.
fn the_account_numbered_1000() -> Option<String> {
    let said = run(&["id", "-nu", "1000"]).out;

    if said.is_empty() {
        return None;
    }

    println!(
        "no desktop in anybody's home yet, so this is being installed for {said}, who is the \
         account numbered 1000"
    );
    Some(said)
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

/// systemctl for the manager this process is already in.
///
/// The other one, `user_systemctl`, is root reaching into somebody else's
/// manager, which is what an apply needs and what nothing else should use. This
/// is for the questions asked by a program the desktop user is running as
/// themselves -- `console well`, out of a unit in that same manager -- where
/// reaching in from outside would need root the program has not got and would
/// answer about the same manager anyway.
pub fn mine(args: &[&str]) -> Said {
    let argv: Vec<&str> =
        ["systemctl", "--user"].into_iter().chain(args.iter().copied()).collect();
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
pub fn stage_file(from: &Path, live: &str) -> Result<(), String> {
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

    let staged = laying::staged(to);
    // Read and written rather than copied, because the mark standing for
    // whoever this desktop belongs to is filled in on the way past. A file
    // that holds no mark comes out the same bytes it went in as.
    let held = std::fs::read(from).map_err(|fault| complain("reading it", fault))?;
    // Synced, not merely written. Everything staged here is renamed into place
    // in one run at the end of the apply; bytes that were promised rather than
    // written are bytes a machine that loses power during that run comes back
    // without, under a name that says the release is installed.
    console_writing::settled(&staged, &install::content_on_machine(&held, whoever(), live))
        .map_err(|fault| format!("{live}: {fault}"))?;

    // The bytes that are about to be written, rather than a second read of
    // the same file: what decides whether this is a program is what is going
    // on the machine, and a file that changed between the two reads would
    // otherwise be installed under the other one's mode.
    let mode = install::mode_of(live, &held);
    std::fs::set_permissions(&staged, permissions(mode))
        .map_err(|fault| complain("its mode", fault))?;

    let owner = install::owner_of(live, whoever());
    let (uid, gid) = who(&owner).ok_or_else(|| format!("{live}: no user called {owner}"))?;
    std::os::unix::fs::chown(&staged, Some(uid), Some(gid))
        .map_err(|fault| complain("its owner", fault))
}

/// Put a staged file in place, keeping whatever was there.
///
/// The rename is the whole of it, and it is what makes this safe twice over. A
/// rename either happened or did not, so nothing ever reads half a file. And it
/// replaces a name rather than a file, so a service still executing the program
/// that was there goes on executing it -- the inode outlives the name, and a
/// deploy does not reach inside a running process.
///
/// What was there is linked aside first, not copied. A link is the same inode
/// under a second name, so putting it back is another rename and the thing put
/// back is the thing that was there rather than something that resembles it.
pub fn swap_file(live: &str) -> Result<Back, String> {
    let on = install::on_machine(live, whoever());
    let to = Path::new(&on);
    let complain = |what: &str, fault: std::io::Error| format!("{live}: {what}: {fault}");

    let back = match to.exists() {
        false => Back::Gone,
        true => {
            let kept = laying::kept(to);
            let _ = std::fs::remove_file(&kept);
            std::fs::hard_link(to, &kept).map_err(|fault| complain("keeping what was there", fault))?;
            Back::Kept
        }
    };
    std::fs::rename(laying::staged(to), to)
        .map_err(|fault| complain("moving it into place", fault))?;

    // The name, made durable as well as the bytes. Without this a machine that
    // stops inside the swap can come back holding the new file under the
    // staging name with the old one still live -- which is the half-laid
    // release this whole arrangement exists to make impossible, arriving after
    // the renames rather than during them.
    if let Err(fault) = console_writing::named(to) {
        eprintln!("console apply: {fault}");
    }

    Ok(back)
}

/// Put one file back the way it was.
pub fn put_back(laid: &Laid) -> Result<(), String> {
    let on = install::on_machine(&laid.at, whoever());
    let to = Path::new(&on);
    let complain = |what: &str, fault: std::io::Error| format!("{}: {what}: {fault}", laid.at);

    match laid.back {
        Back::Kept => std::fs::rename(laying::kept(to), to)
            .map_err(|fault| complain("putting back what was there", fault)),
        Back::Gone => std::fs::remove_file(to)
            .map_err(|fault| complain("taking away what this put there", fault)),
    }
}

/// Throw away a staged file that is not going to be used.
pub fn drop_staged(live: &str) {
    let on = install::on_machine(live, whoever());
    let _ = std::fs::remove_file(laying::staged(Path::new(&on)));
}

/// Throw away the copy kept in case a file had to go back.
pub fn drop_kept(live: &str) {
    let on = install::on_machine(live, whoever());
    let _ = std::fs::remove_file(laying::kept(Path::new(&on)));
}

fn permissions(mode: u32) -> std::fs::Permissions {
    use std::os::unix::fs::PermissionsExt;
    std::fs::Permissions::from_mode(mode)
}

/// A user's numbers, asked of the system rather than read out of a file, so
/// that a machine which keeps its users somewhere else still answers.
fn who(user: &str) -> Option<(u32, u32)> {
    // `id` says nothing at all about a user that does not exist, and an answer
    // that is not a number is that silence: there is no third thing it prints.
    let number = |flag: &str| {
        let Ok(number) = run(&["id", flag, user]).out.parse::<u32>() else { return None };

        Some(number)
    };
    Some((number("-u")?, number("-g")?))
}

/// Keep a history of what was taken into the tree, so a desktop that breaks
/// can be walked back.
///
/// Only the files this wrote, named one at a time. It was `git add -A`, and
/// that is a different thing entirely: in a checkout somebody is working in,
/// it sweeps whatever else was open into a commit named after an operation
/// that never touched it. On the device it is worse than untidy. `/etc/console`
/// is pushed into with `receive.denyCurrentBranch updateInstead`, so a commit
/// nobody made there is a branch that has diverged, and the way that is found
/// out is the next deploy being refused.
///
/// An operation that wrote nothing into the tree passes nothing and commits
/// nothing. `console apply` is that operation: everything it writes is on the
/// machine, not here.
pub fn commit(root: &Path, what: &str, wrote: &[std::path::PathBuf]) {
    if !root.join(".git").exists() || wrote.is_empty() {
        return;
    }

    let root = root.display().to_string();
    let named: Vec<String> = wrote.iter().map(|at| at.display().to_string()).collect();
    let argv = |verb: &[&str]| -> Vec<String> {
        let mut argv: Vec<String> = ["git", "-C", &root].iter().map(|word| word.to_string()).collect();
        argv.extend(verb.iter().map(|word| (*word).to_string()));
        argv.push("--".to_string());
        argv.extend(named.iter().cloned());
        argv
    };
    let said = |argv: &[String]| run(&argv.iter().map(String::as_str).collect::<Vec<_>>());
    // Added first, because a file being taken into the tree for the first time
    // is not a path `git commit` will take on its own. Committed by path after
    // that, so an index somebody else has already staged into is not what gets
    // written down.
    said(&argv(&["add"]));

    if !said(&argv(&["status", "--porcelain"])).out.is_empty() {
        said(&argv(&["commit", "-m", what]));
    }
}

/// What in a tree has not been committed, where it is a checkout at all.
///
/// Asked so that an apply can say the machine it just built matches no commit.
/// Empty where git will not answer, which includes a root running this in
/// somebody else's checkout: silence is not a promise that the tree is clean,
/// and nothing here decides anything on the strength of it.
pub fn uncommitted(root: &Path) -> Vec<String> {
    if !root.join(".git").exists() {
        return Vec::new();
    }

    let root = root.display().to_string();
    run(&["git", "-C", &root, "status", "--porcelain"])
        .out
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

/// The machine itself, as the thing a release is laid down on.
///
/// A unit struct rather than nothing, because `laying::Deploy` is written
/// against a trait so its order can be held to without a machine, and this is
/// the machine. Everything it does is the four functions above.
pub struct Here;

impl laying::Lays for Here {
    fn stage(&mut self, from: &Path, live: &str) -> Result<(), String> {
        stage_file(from, live)
    }

    fn swap(&mut self, live: &str) -> Result<Back, String> {
        swap_file(live)
    }

    fn put_back(&mut self, laid: &Laid) -> Result<(), String> {
        put_back(laid)
    }

    fn drop_staged(&mut self, live: &str) {
        drop_staged(live);
    }

    fn drop_kept(&mut self, live: &str) {
        drop_kept(live);
    }

    fn standing(&self, live: &str) -> Back {
        let on = install::on_machine(live, whoever());

        match Path::new(&on).exists() {
            true => Back::Kept,
            false => Back::Gone,
        }
    }

    fn note(&mut self, laid: &[Laid]) -> Result<(), String> {
        wrote_plan(Path::new(PLAN), laid)
    }

    fn forget_note(&mut self) {
        forget_plan(Path::new(PLAN));
    }
}

/// Where an apply writes down what it is in the middle of doing.
///
/// Under `/var/lib` rather than `/run`, and that is the whole point of it. A
/// note in the runtime directory would be swept by the reboot, and a reboot is
/// exactly what happens between a machine stopping inside a swap and anybody
/// finding out. This has to outlive the thing it is a record of.
pub const PLAN: &str = "/var/lib/console/laying";

/// What one file's line in the plan says.
///
/// The word first and the path second, because a path can hold anything and the
/// word is one of two. Split the other way round, a file with a space in its
/// name would be read as a word nobody wrote.
fn line_of(laid: &Laid) -> String {
    let back = match laid.back {
        Back::Kept => "kept",
        Back::Gone => "gone",
    };
    format!("{back} {}
", laid.at)
}

/// Write the plan down, whole and synced, before a single file moves.
fn wrote_plan(at: &Path, laid: &[Laid]) -> Result<(), String> {
    if let Some(holding) = at.parent() {
        std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{}: its directory: {fault}", at.display()))?;
    }

    let written: String = laid.iter().map(line_of).collect();
    console_writing::whole(at, written.as_bytes())
}

/// Take the plan away, there being no half-laid release to describe.
fn forget_plan(at: &Path) {
    match std::fs::remove_file(at) {
        Ok(()) => {}
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => {}
        // Left behind, this reads as an apply that never finished, and `console
        // well` will raise a card about it at every boot until somebody looks.
        // Better that than silence, and better still to say so now.
        Err(fault) => eprintln!("console apply: {} will not go away: {fault}", at.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The directory that tells this desktop's home from anybody else's is one
    /// the manifest actually lays down.
    ///
    /// Held together because they are two statements of one fact written in two
    /// files. A rename of the desktop's own config directory that moved the
    /// manifest and not this would leave `whoever` looking for something no
    /// home has, falling through to the account numbered 1000, and answering
    /// the way it answered before any of this was fixed -- on a machine where
    /// the desktop was sitting in plain sight in somebody's home.
    #[test]
    fn the_directory_that_marks_a_home_is_one_the_manifest_puts_there() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let held = std::fs::read_to_string(root.join("desktop.conf")).expect("the manifest");
        let under = format!("/home/{USER}/{OURS}/");
        assert!(
            held.lines().any(|line| line.trim().starts_with(&under)),
            "nothing the manifest lays down is under {under}, so no home can be told by it"
        );
    }
}

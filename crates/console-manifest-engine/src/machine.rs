//! Everything that touches the machine itself.
//!
//! Kept apart from the rest so that what decides and what does are two files.
//! Nothing here can be tested without a machine, which is exactly why nothing
//! here decides anything.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_places::Base;

use crate::install::{USER, User, self};
use crate::laying::{self, Back, Laid};
use crate::modes;
use crate::unapplied::Unapplied;

pub struct Output {
    pub out: String,
}

pub fn run(arguments: &[&str]) -> Result<Output, Never> {
    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(Output { out: String::new() }),
    };

    Ok(match Command::new(program).args(rest).output() {
        Ok(done) => Output { out: String::from_utf8_lossy(&done.stdout).trim().to_owned() },
        Err(_would_not_start) => Output { out: String::new() },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ran {
    Fine,
    Badly,
}

pub struct CommandOutput {
    pub out: String,
    pub said: String,
    pub ran: Ran,
}

pub fn answered(arguments: &[&str]) -> Result<CommandOutput, Never> {
    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(CommandOutput { out: String::new(), said: String::new(), ran: Ran::Badly }),
    };

    Ok(match Command::new(program).args(rest).output() {
        Ok(done) => CommandOutput {
            out: String::from_utf8_lossy(&done.stdout).trim().to_owned(),
            said: String::from_utf8_lossy(&done.stderr).trim().to_owned(),
            ran: match done.status.success() {
                true => Ran::Fine,
                false => Ran::Badly,
            },
        },
        Err(fault) => CommandOutput {
            out: String::new(),
            said: format!("{program}: {fault}"),
            ran: Ran::Badly,
        },
    })
}

pub fn run_seen(arguments: &[&str]) -> Result<Ran, Never> {
    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(Ran::Badly),
    };

    let done = match Command::new(program).args(rest).status() {
        Ok(done) => done.success(),
        Err(_would_not_start) => false,
    };

    Ok(match done {
        true => Ran::Fine,
        false => Ran::Badly,
    })
}

pub fn run_watched<M>(
    arguments: &[&str],
    handed: &mut M,
    heard: impl Fn(&mut M, &str),
) -> Result<Ran, Never> {
    use std::io::{BufRead, BufReader};

    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(Ran::Badly),
    };

    let mut starting = Command::new(program);

    starting.args(rest).stdout(std::process::Stdio::piped());

    let mut child = match console_program_lifetime::alongside(&mut starting) {
        Ok(child) => child,

        Err(fault) => {
            eprintln!("console: {program}: {fault}");

            return Ok(Ran::Badly);
        }
    };
    let Ok(reading) = child.reading();

    match reading {
        Some(said) => {
            for line in BufReader::new(said).lines().map_while(Result::ok) {
                heard(handed, &line);
            }
        }
        None => {},
    }

    let done = match child.waiting() {
        Ok(done) => done.success(),
        Err(_would_not_wait) => false,
    };

    Ok(match done {
        true => Ran::Fine,
        false => Ran::Badly,
    })
}

pub fn whoever() -> Result<&'static str, Never> {
    #[cfg_attr(
        dylint_lib = "explicit044_no_ambient_value",
        allow(
            explicit044_no_ambient_value,
            reason = "who the desktop belongs to is what `@user@` is filled in with, settled by the machine before anything is laid down and the same for every path an apply writes; `docs/crates.md` is why this question stays here rather than going to `console_core_places`"
        )
    )]
    static KNOWN: OnceLock<String> = OnceLock::new();

    Ok(KNOWN.get_or_init(|| {
        let Ok(one) = the_one_home();
        let Ok(already) = the_home_it_is_already_in();
        let Ok(numbered) = the_account_numbered_1000();

        match one.or(already).or(numbered) {
            Some(whose) => whose,
            None => USER.to_string(),
        }
    }))
}

fn homes() -> Result<Vec<String>, Never> {
    let reading = match std::fs::read_dir("/home") {
        Ok(reading) => reading,
        Err(_unreadable) => return Ok(Vec::new()),
    };

    Ok(reading
        .flatten()
        .filter(|found| found.path().is_dir())
        .filter_map(|found| match found.file_name().into_string() {
            Ok(name) => Some(name),

            Err(name) => {
                eprintln!("console: /home/{}: not a name this desktop can act on", name.to_string_lossy());
                None
            }
        })
        .filter(|name| {
            let Ok(who) = who(name);

            who.is_some()
        })
        .collect())
}

fn the_one_home() -> Result<Option<String>, Never> {
    let Ok(mut homes) = homes();

    Ok(match homes.len() {
        1 => homes.pop(),
        _ => None,
    })
}

fn the_home_it_is_already_in() -> Result<Option<String>, Never> {
    let Ok(homes) = homes();
    let mut theirs: Vec<String> = homes
        .into_iter()
        .filter(|name| {
            let Ok(ours) = Base::Configuration.ours_under(&Path::new("/home").join(name));

            ours.is_dir()
        })
        .collect();

    Ok(match theirs.len() {
        1 => theirs.pop(),
        _ => None,
    })
}

fn the_account_numbered_1000() -> Result<Option<String>, Never> {
    let Ok(id) = Program::Id.name();
    let Ok(asked) = run(&[id, "-nu", "1000"]);
    let said = asked.out;

    match said.is_empty() {
        true => return Ok(None),
        false => {},
    }

    println!(
        "no desktop in anyone's home yet, so this is being installed for {said}, who is the \
         account numbered 1000"
    );

    Ok(Some(said))
}

pub fn user_systemctl(words: &[&str]) -> Result<CommandOutput, Never> {
    let Ok(whoever) = whoever();
    let Ok(systemctl) = Program::Systemctl.name();

    let owned = format!("{whoever}@");
    let arguments: Vec<&str> = [systemctl, "--user", "-M", &owned]
        .into_iter()
        .chain(words.iter().copied())
        .collect();

    answered(&arguments)
}

pub fn mine(words: &[&str]) -> Result<Output, Never> {
    let Ok(systemctl) = Program::Systemctl.name();

    let arguments: Vec<&str> =
        [systemctl, "--user"].into_iter().chain(words.iter().copied()).collect();

    run(&arguments)
}

pub fn in_the_session(command: &str) -> Result<(), Never> {
    let Ok(owner) = whoever();
    let Ok(who) = who(owner);

    let (uid, _taken_1) = match who {
        Some((uid, _taken_1)) => (uid, _taken_1),
        None => return Ok(()),
    };

    let line = format!(
        "env XDG_RUNTIME_DIR=/run/user/{uid} \
         DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{uid}/bus {command}"
    );
    let Ok(su) = Program::Su.name();
    let Ok(_) = run(&[su, owner, "-s", "/bin/sh", "-c", &line]);

    Ok(())
}

pub fn unit_state(unit: &str) -> Result<(String, String), Never> {
    let Ok(enabled) = user_systemctl(&["is-enabled", unit]);
    let Ok(active) = user_systemctl(&["is-active", unit]);

    Ok((enabled.out, active.out))
}

pub fn disk_free(at: &Path) -> Result<Output, Never> {
    let Ok(df) = Program::Df.name();
    let where_it_is = at.display().to_string();

    run(&[df, "--block-size=1", "--output=avail", &where_it_is])
}

pub fn sizes_under(roots: &[&str]) -> Result<Output, Never> {
    let Ok(du) = Program::Du.name();

    let arguments: Vec<&str> = [du, "--block-size=1", "--one-file-system", "--max-depth=1"]
        .into_iter()
        .chain(roots.iter().copied())
        .collect();

    run(&arguments)
}

pub fn installed_packages() -> Result<Vec<String>, Never> {
    let Ok(pacman) = Program::Pacman.name();

    named(&[pacman, "-Qq"])
}

pub fn wanted_packages() -> Result<Vec<String>, Never> {
    let Ok(pacman) = Program::Pacman.name();

    named(&[pacman, "-Qeq"])
}

fn named(arguments: &[&str]) -> Result<Vec<String>, Never> {
    let Ok(said) = run(arguments);

    Ok(said.out.split_whitespace().map(str::to_owned).collect())
}

pub fn stage_file(from: &Path, live: &str) -> Result<(), Unapplied> {
    let Ok(whoever) = whoever();
    let Ok(on) = install::on_machine(live, User(whoever));
    let to = Path::new(&on);
    let complain =
        |what: &'static str, fault: std::io::Error| Unapplied::Staging(live.to_string(), what, fault);
    let Ok(holds) = install::holding(&on);

    for holding in holds {
        match holding.is_dir() {
            true => continue,
            false => {},
        }

        std::fs::create_dir(&holding).map_err(|fault| complain("its directory", fault))?;

        let Ok(owner) = install::owner_of(&holding.to_string_lossy(), User(whoever));
        let Ok(who) = who(&owner);
        let (uid, gid) =
            who.ok_or_else(|| Unapplied::NoUserFor(live.to_string(), owner.to_string()))?;

        std::os::unix::fs::chown(&holding, Some(uid), Some(gid))
            .map_err(|fault| complain("its directory's owner", fault))?;
    }

    let Ok(beside) = laying::staged(to);

    let staged = match beside {
        Some(staged) => staged,
        None => return Err(Unapplied::NothingBeside(live.to_string(), "stage one beside")),
    };

    let held = std::fs::read(from).map_err(|fault| complain("reading it", fault))?;
    let Ok(content) = install::content_on_machine(&held, User(whoever), live);

    console_core_atomic_writes::settled(&staged, &content)
        .map_err(|fault| Unapplied::Unwritten(live.to_string(), fault))?;

    let Ok(mode) = modes::of(live, &held);
    let Ok(permissions) = permissions(mode);

    std::fs::set_permissions(&staged, permissions).map_err(|fault| complain("its mode", fault))?;

    let Ok(owner) = install::owner_of(live, User(whoever));
    let Ok(who) = who(&owner);
    let (uid, gid) = who.ok_or_else(|| Unapplied::NoUserFor(live.to_string(), owner.to_string()))?;

    std::os::unix::fs::chown(&staged, Some(uid), Some(gid))
        .map_err(|fault| complain("its owner", fault))
}

pub fn swap_file(live: &str) -> Result<Back, Unapplied> {
    let Ok(whoever) = whoever();
    let Ok(on) = install::on_machine(live, User(whoever));
    let to = Path::new(&on);
    let complain =
        |what: &'static str, fault: std::io::Error| Unapplied::Staging(live.to_string(), what, fault);

    let back = match to.exists() {
        false => Back::Closed,
        true => {
            let Ok(beside) = laying::kept(to);

            let kept = match beside {
                Some(kept) => kept,
                None => {
                    return Err(Unapplied::NothingBeside(live.to_string(), "keep one beside"));
                }
            };

            let _ = std::fs::remove_file(&kept);

            std::fs::hard_link(to, &kept).map_err(|fault| complain("keeping what was there", fault))?;

            Back::Retained
        }
    };

    let Ok(beside) = laying::staged(to);

    let staged = match beside {
        Some(staged) => staged,
        None => {
            return Err(Unapplied::NothingBeside(
                live.to_string(),
                "move one into place as",
            ));
        }
    };

    std::fs::rename(staged, to).map_err(|fault| complain("moving it into place", fault))?;

    match console_core_atomic_writes::named(to) {
        Ok(()) => {},
        Err(fault) => eprintln!("console apply: {fault}"),
    }

    Ok(back)
}

pub fn put_back(laid: &Laid) -> Result<(), Unapplied> {
    let Ok(whoever) = whoever();
    let Ok(on) = install::on_machine(&laid.at, User(whoever));
    let to = Path::new(&on);
    let complain = |what: &'static str, fault: std::io::Error| {
        Unapplied::Staging(laid.at.clone(), what, fault)
    };

    match laid.back {
        Back::Retained => {
            let Ok(beside) = laying::kept(to);

            let kept = match beside {
                Some(kept) => kept,
                None => {
                    return Err(Unapplied::NothingKept(laid.at.clone()));
                }
            };

            std::fs::rename(kept, to).map_err(|fault| complain("putting back what was there", fault))
        }
        Back::Closed => std::fs::remove_file(to)
            .map_err(|fault| complain("taking away what this put there", fault)),
    }
}

fn drop_beside(live: &str, beside: fn(&Path) -> Result<Option<PathBuf>, Never>) -> Result<(), Never> {
    let Ok(whoever) = whoever();
    let Ok(on) = install::on_machine(live, User(whoever));
    let Ok(found) = beside(Path::new(&on));

    match found {
        Some(beside) => {
            let _ = std::fs::remove_file(beside);
        }
        None => {}
    }

    Ok(())
}

pub fn drop_staged(live: &str) -> Result<(), Never> {
    drop_beside(live, laying::staged)
}

pub fn drop_kept(live: &str) -> Result<(), Never> {
    drop_beside(live, laying::kept)
}

fn permissions(mode: u32) -> Result<std::fs::Permissions, Never> {
    use std::os::unix::fs::PermissionsExt;

    Ok(std::fs::Permissions::from_mode(mode))
}

pub fn handed_over(at: &Path) -> Result<(), Unapplied> {
    let Ok(whoever) = whoever();
    let Ok(who) = who(whoever);
    let (uid, gid) = who.ok_or_else(|| Unapplied::NoUser(whoever.to_string()))?;

    std::os::unix::fs::chown(at, Some(uid), Some(gid))
        .map_err(|fault| Unapplied::Owner(at.to_path_buf(), fault))
}

fn who(user: &str) -> Result<Option<(u32, u32)>, Never> {
    let number = |flag: &str| {
        let Ok(id) = Program::Id.name();
        let Ok(said) = run(&[id, flag, user]);

        let number = match said.out.parse::<u32>() {
            Ok(number) => number,
            Err(_not_a_number) => return None,
        };

        Some(number)
    };

    let uid = match number("-u") {
        Some(uid) => uid,
        None => return Ok(None),
    };

    let gid = match number("-g") {
        Some(gid) => gid,
        None => return Ok(None),
    };

    Ok(Some((uid, gid)))
}

pub fn commit(root: &Path, what: &str, wrote: &[std::path::PathBuf]) -> Result<(), Never> {
    match !root.join(".git").exists() || wrote.is_empty() {
        true => return Ok(()),
        false => {},
    }

    let root = root.display().to_string();
    let named: Vec<String> = wrote.iter().map(|at| at.display().to_string()).collect();
    let arguments = |verb: &[&str]| -> Vec<String> {
        let Ok(mut arguments) = Program::Git.arguments(&["-C", &root]);

        arguments.extend(verb.iter().map(|word| (*word).to_string()));
        arguments.push("--".to_string());
        arguments.extend(named.iter().cloned());
        arguments
    };
    let said = |arguments: &[String]| {
        let Ok(said) = run(&arguments.iter().map(String::as_str).collect::<Vec<_>>());

        said
    };
    let _ = said(&arguments(&["add"]));

    match !said(&arguments(&["status", "--porcelain"])).out.is_empty() {
        true => {
            let _ = said(&arguments(&["commit", "-m", what]));
        }
        false => {},
    }

    Ok(())
}

pub fn uncommitted(root: &Path) -> Result<Vec<String>, Never> {
    match !root.join(".git").exists() {
        true => return Ok(Vec::new()),
        false => {},
    }

    let root = root.display().to_string();
    let Ok(git) = Program::Git.name();
    let Ok(said) = run(&[git, "-C", &root, "status", "--porcelain"]);

    Ok(said
        .out
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect())
}

pub struct Here;

impl laying::Lays for Here {
    fn stage(&mut self, from: &Path, live: &str) -> Result<(), Unapplied> {
        stage_file(from, live)
    }

    fn swap(&mut self, live: &str) -> Result<Back, Unapplied> {
        swap_file(live)
    }

    fn put_back(&mut self, laid: &Laid) -> Result<(), Unapplied> {
        put_back(laid)
    }

    fn drop_staged(&mut self, live: &str) {
        let Ok(()) = drop_staged(live);
    }

    fn drop_kept(&mut self, live: &str) {
        let Ok(()) = drop_kept(live);
    }

    fn standing(&self, live: &str) -> Back {
        let Ok(whoever) = whoever();
        let Ok(on) = install::on_machine(live, User(whoever));

        match Path::new(&on).exists() {
            true => Back::Retained,
            false => Back::Closed,
        }
    }

    fn note(&mut self, laid: &[Laid]) -> Result<(), Unapplied> {
        wrote_plan(Path::new(PLAN), laid)
    }

    fn forget_note(&mut self) {
        let Ok(()) = forget_plan(Path::new(PLAN));
    }
}

pub const PLAN: &str = "/var/lib/console/laying";

fn line_of(laid: &Laid) -> Result<String, Never> {
    let back = match laid.back {
        Back::Retained => "kept",
        Back::Closed => "gone",
    };

    Ok(format!("{back} {}
", laid.at))
}

fn wrote_plan(at: &Path, laid: &[Laid]) -> Result<(), Unapplied> {
    match at.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| Unapplied::Directory(at.to_path_buf(), fault))?,
        None => {},
    }

    let written: String = laid
        .iter()
        .map(|laid| {
            let Ok(line) = line_of(laid);

            line
        })
        .collect();
    console_core_atomic_writes::whole(at, written.as_bytes()).map_err(Unapplied::Wrote)
}

fn forget_plan(at: &Path) -> Result<(), Never> {
    match console_core_atomic_writes::gone(at) {
        Ok(()) => {}
        Err(fault) => eprintln!("console apply: {fault}"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_directory_that_marks_a_home_is_one_the_manifest_puts_there() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let held = std::fs::read_to_string(root.join("desktop.conf")).expect("the manifest");
        let Ok(ours) = Base::Configuration.ours_under(&Path::new("/home").join(USER));

        let under = format!("{}/", ours.display());
        assert!(
            held.lines().any(|line| line.trim().starts_with(&under)),
            "nothing the manifest lays down is under {under}, so no home can be told by it"
        );
    }
}

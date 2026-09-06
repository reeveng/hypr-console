//! Everything that touches the machine itself.
//!
//! Kept apart from the rest so that what decides and what does are two files.
//! Nothing here can be tested without a machine, which is exactly why nothing
//! here decides anything.

use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use console_external_programs::Program;
use console_never::Never;

use crate::install::{self, USER};
use crate::laying::{self, Back, Laid};

pub struct Said {
    pub out: String,
}

pub fn run(argv: &[&str]) -> Result<Said, Never> {
    let Some((program, rest)) = argv.split_first() else {
        return Ok(Said { out: String::new() });
    };

    Ok(match Command::new(program).args(rest).output() {
        Ok(done) => Said { out: String::from_utf8_lossy(&done.stdout).trim().to_owned() },
        Err(_) => Said { out: String::new() },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ran {
    Fine,
    Badly,
}

pub struct Answered {
    pub out: String,
    pub said: String,
    pub ran: Ran,
}

pub fn answered(argv: &[&str]) -> Result<Answered, Never> {
    let Some((program, rest)) = argv.split_first() else {
        return Ok(Answered { out: String::new(), said: String::new(), ran: Ran::Badly });
    };

    Ok(match Command::new(program).args(rest).output() {
        Ok(done) => Answered {
            out: String::from_utf8_lossy(&done.stdout).trim().to_owned(),
            said: String::from_utf8_lossy(&done.stderr).trim().to_owned(),
            ran: match done.status.success() {
                true => Ran::Fine,
                false => Ran::Badly,
            },
        },
        Err(fault) => Answered {
            out: String::new(),
            said: format!("{program}: {fault}"),
            ran: Ran::Badly,
        },
    })
}

pub fn run_seen(argv: &[&str]) -> Result<Ran, Never> {
    let Some((program, rest)) = argv.split_first() else {
        return Ok(Ran::Badly);
    };

    let done = match Command::new(program).args(rest).status() {
        Ok(done) => done.success(),
        Err(_) => false,
    };

    Ok(match done {
        true => Ran::Fine,
        false => Ran::Badly,
    })
}

const OURS: &str = ".config/console";

pub fn whoever() -> Result<&'static str, Never> {
    static KNOWN: OnceLock<String> = OnceLock::new();

    Ok(KNOWN.get_or_init(|| {
        let Ok(one) = the_one_home();
        let Ok(already) = the_home_it_is_already_in();
        let Ok(numbered) = the_account_numbered_1000();

        one.or(already).or(numbered).unwrap_or_else(|| USER.to_string())
    }))
}

fn homes() -> Result<Vec<String>, Never> {
    let Ok(reading) = std::fs::read_dir("/home") else { return Ok(Vec::new()) };

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
        .filter(|name| Path::new("/home").join(name).join(OURS).is_dir())
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
        "no desktop in anybody's home yet, so this is being installed for {said}, who is the \
         account numbered 1000"
    );

    Ok(Some(said))
}

pub fn user_systemctl(args: &[&str]) -> Result<Said, Never> {
    let Ok(whoever) = whoever();
    let Ok(systemctl) = Program::Systemctl.name();

    let owned = format!("{whoever}@");
    let argv: Vec<&str> = [systemctl, "--user", "-M", &owned]
        .into_iter()
        .chain(args.iter().copied())
        .collect();

    run(&argv)
}

pub fn mine(args: &[&str]) -> Result<Said, Never> {
    let Ok(systemctl) = Program::Systemctl.name();

    let argv: Vec<&str> =
        [systemctl, "--user"].into_iter().chain(args.iter().copied()).collect();

    run(&argv)
}

pub fn in_the_session(command: &str) -> Result<(), Never> {
    let Ok(owner) = whoever();
    let Ok(who) = who(owner);

    let Some((uid, _)) = who else { return Ok(()) };

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

pub fn installed_packages() -> Result<Vec<String>, Never> {
    let Ok(pacman) = Program::Pacman.name();

    named(&[pacman, "-Qq"])
}

pub fn wanted_packages() -> Result<Vec<String>, Never> {
    let Ok(pacman) = Program::Pacman.name();

    named(&[pacman, "-Qeq"])
}

fn named(argv: &[&str]) -> Result<Vec<String>, Never> {
    let Ok(said) = run(argv);

    Ok(said.out.split_whitespace().map(str::to_owned).collect())
}

pub fn stage_file(from: &Path, live: &str) -> Result<(), String> {
    let Ok(whoever) = whoever();
    let Ok(on) = install::on_machine(live, whoever);
    let to = Path::new(&on);
    let complain = |what: &str, fault: std::io::Error| format!("{live}: {what}: {fault}");
    let Ok(holds) = install::holding(&on);

    for holding in holds {
        match holding.is_dir() {
            true => continue,
            false => {},
        }

        std::fs::create_dir(&holding).map_err(|fault| complain("its directory", fault))?;

        let Ok(owner) = install::owner_of(&holding.to_string_lossy(), whoever);
        let Ok(who) = who(&owner);
        let (uid, gid) = who.ok_or_else(|| format!("{live}: no user called {owner}"))?;

        std::os::unix::fs::chown(&holding, Some(uid), Some(gid))
            .map_err(|fault| complain("its directory's owner", fault))?;
    }

    let Ok(staged) = laying::staged(to);
    let held = std::fs::read(from).map_err(|fault| complain("reading it", fault))?;
    let Ok(content) = install::content_on_machine(&held, whoever, live);

    console_file_writing::settled(&staged, &content).map_err(|fault| format!("{live}: {fault}"))?;

    let Ok(mode) = install::mode_of(live, &held);
    let Ok(permissions) = permissions(mode);

    std::fs::set_permissions(&staged, permissions).map_err(|fault| complain("its mode", fault))?;

    let Ok(owner) = install::owner_of(live, whoever);
    let Ok(who) = who(&owner);
    let (uid, gid) = who.ok_or_else(|| format!("{live}: no user called {owner}"))?;

    std::os::unix::fs::chown(&staged, Some(uid), Some(gid))
        .map_err(|fault| complain("its owner", fault))
}

pub fn swap_file(live: &str) -> Result<Back, String> {
    let Ok(whoever) = whoever();
    let Ok(on) = install::on_machine(live, whoever);
    let to = Path::new(&on);
    let complain = |what: &str, fault: std::io::Error| format!("{live}: {what}: {fault}");

    let back = match to.exists() {
        false => Back::Gone,
        true => {
            let Ok(kept) = laying::kept(to);
            let _ = std::fs::remove_file(&kept);

            std::fs::hard_link(to, &kept).map_err(|fault| complain("keeping what was there", fault))?;

            Back::Kept
        }
    };

    let Ok(staged) = laying::staged(to);

    std::fs::rename(staged, to).map_err(|fault| complain("moving it into place", fault))?;

    match console_file_writing::named(to) {
        Ok(()) => {},
        Err(fault) => eprintln!("console apply: {fault}"),
    }

    Ok(back)
}

pub fn put_back(laid: &Laid) -> Result<(), String> {
    let Ok(whoever) = whoever();
    let Ok(on) = install::on_machine(&laid.at, whoever);
    let to = Path::new(&on);
    let complain = |what: &str, fault: std::io::Error| format!("{}: {what}: {fault}", laid.at);

    match laid.back {
        Back::Kept => {
            let Ok(kept) = laying::kept(to);

            std::fs::rename(kept, to).map_err(|fault| complain("putting back what was there", fault))
        }
        Back::Gone => std::fs::remove_file(to)
            .map_err(|fault| complain("taking away what this put there", fault)),
    }
}

pub fn drop_staged(live: &str) -> Result<(), Never> {
    let Ok(whoever) = whoever();
    let Ok(on) = install::on_machine(live, whoever);
    let Ok(staged) = laying::staged(Path::new(&on));
    let _ = std::fs::remove_file(staged);

    Ok(())
}

pub fn drop_kept(live: &str) -> Result<(), Never> {
    let Ok(whoever) = whoever();
    let Ok(on) = install::on_machine(live, whoever);
    let Ok(kept) = laying::kept(Path::new(&on));
    let _ = std::fs::remove_file(kept);

    Ok(())
}

fn permissions(mode: u32) -> Result<std::fs::Permissions, Never> {
    use std::os::unix::fs::PermissionsExt;

    Ok(std::fs::Permissions::from_mode(mode))
}

fn who(user: &str) -> Result<Option<(u32, u32)>, Never> {
    let number = |flag: &str| {
        let Ok(id) = Program::Id.name();
        let Ok(said) = run(&[id, flag, user]);

        let Ok(number) = said.out.parse::<u32>() else { return None };

        Some(number)
    };

    let Some(uid) = number("-u") else { return Ok(None) };

    let Some(gid) = number("-g") else { return Ok(None) };

    Ok(Some((uid, gid)))
}

pub fn commit(root: &Path, what: &str, wrote: &[std::path::PathBuf]) -> Result<(), Never> {
    match !root.join(".git").exists() || wrote.is_empty() {
        true => return Ok(()),
        false => {},
    }

    let root = root.display().to_string();
    let named: Vec<String> = wrote.iter().map(|at| at.display().to_string()).collect();
    let argv = |verb: &[&str]| -> Vec<String> {
        let Ok(mut argv) = Program::Git.argv(&["-C", &root]);

        argv.extend(verb.iter().map(|word| (*word).to_string()));
        argv.push("--".to_string());
        argv.extend(named.iter().cloned());
        argv
    };
    let said = |argv: &[String]| {
        let Ok(said) = run(&argv.iter().map(String::as_str).collect::<Vec<_>>());

        said
    };
    let _ = said(&argv(&["add"]));

    match !said(&argv(&["status", "--porcelain"])).out.is_empty() {
        true => {
            let _ = said(&argv(&["commit", "-m", what]));
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
        let Ok(()) = drop_staged(live);
    }

    fn drop_kept(&mut self, live: &str) {
        let Ok(()) = drop_kept(live);
    }

    fn standing(&self, live: &str) -> Back {
        let Ok(whoever) = whoever();
        let Ok(on) = install::on_machine(live, whoever);

        match Path::new(&on).exists() {
            true => Back::Kept,
            false => Back::Gone,
        }
    }

    fn note(&mut self, laid: &[Laid]) -> Result<(), String> {
        wrote_plan(Path::new(PLAN), laid)
    }

    fn forget_note(&mut self) {
        let Ok(()) = forget_plan(Path::new(PLAN));
    }
}

pub const PLAN: &str = "/var/lib/console/laying";

fn line_of(laid: &Laid) -> Result<String, Never> {
    let back = match laid.back {
        Back::Kept => "kept",
        Back::Gone => "gone",
    };

    Ok(format!("{back} {}
", laid.at))
}

fn wrote_plan(at: &Path, laid: &[Laid]) -> Result<(), String> {
    match at.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{}: its directory: {fault}", at.display()))?,
        None => {},
    }

    let written: String = laid
        .iter()
        .map(|laid| {
            let Ok(line) = line_of(laid);

            line
        })
        .collect();
    console_file_writing::whole(at, written.as_bytes())
}

fn forget_plan(at: &Path) -> Result<(), Never> {
    match std::fs::remove_file(at) {
        Ok(()) => {}
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => {}
        Err(fault) => eprintln!("console apply: {} will not go away: {fault}", at.display()),
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
        let under = format!("/home/{USER}/{OURS}/");
        assert!(
            held.lines().any(|line| line.trim().starts_with(&under)),
            "nothing the manifest lays down is under {under}, so no home can be told by it"
        );
    }
}

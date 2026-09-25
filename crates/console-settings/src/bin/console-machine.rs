//! The three things about this machine that a person chooses and root owns.
//!
//! ```text
//! console-machine language nl_NL.UTF-8 UTF-8   make it if it is not made, then use it
//! console-machine hour Europe/Amsterdam        where the hour is kept
//! console-machine name legion                  what it answers to
//! ```
//!
//! One program rather than three, because it is one class of thing: what /etc
//! says about who this machine belongs to and where it is. `/etc/locale.recipes`,
//! `/etc/locale.conf`, `/etc/localtime` and `/etc/hostname` are all root's, the
//! panel runs as the person holding the device, and something has to carry it
//! across. `/etc/sudoers.d/console` lets her run this without a password,
//! because a password box on a machine whose only keyboard is drawn by the
//! session is a setting that cannot be changed.
//!
//! What makes that safe is that every word it takes is checked against a list
//! the machine itself holds: a language has to be a line in
//! `/usr/share/i18n/SUPPORTED`, an hour has to be a zone `timedatectl` names,
//! and a name has to be a name a network could look up. A word it does not
//! know is refused here rather than passed on, so there is nothing to hand it
//! that makes it write anything else.
//!
//! Making a language is slow -- glibc compiles it -- and it is the one thing
//! here that is. It says what it is doing on the way through, because the
//! panel is drawing a row that says the same.

use std::path::Path;
use std::process::ExitCode;

use console_core_atomic_writes::Stored;
use console_core_external_programs::Program;
use console_settings::named::{self, Allowed};
use console_settings::languages::{self, LOCALE_CONF, LOCALE_GEN, SUPPORTED};

const USAGE: &str = "usage: console-machine [language <name> <charset>|hour <zone>|name <name>]";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();

    let done = match words.as_slice() {
        ["language", name, charset] => language(Locale { name, charset }),
        ["hour", zone] => hour(zone),
        ["name", name] => called(name),
        _ => {
            eprintln!("{USAGE}");

            return ExitCode::from(2);
        }
    };

    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("console-machine: {why}");

            ExitCode::FAILURE
        }
    }
}

#[derive(Debug)]
enum Unset {
    NoProgram(&'static str),
    WouldNotRun(&'static str, std::io::Error),
    SaidNo(&'static str, String),
    Read(String, String),
    NotSupported(String),
    NoSuchZone(String),
    NotAName(String),
    Writing(console_core_atomic_writes::Unwritten),
}

impl std::fmt::Display for Unset {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unset::NoProgram(name) => write!(to, "there is no {name} on this machine"),
            Unset::WouldNotRun(name, fault) => write!(to, "{name} would not run: {fault}"),
            Unset::SaidNo(name, said) => write!(to, "{name} said no: {said}"),
            Unset::Read(at, fault) => write!(to, "{at} will not be read: {fault}"),
            Unset::NotSupported(line) => {
                write!(to, "{line} is not a language {SUPPORTED} names")
            }
            Unset::NoSuchZone(zone) => {
                write!(to, "{zone} is not a zone this machine has heard of")
            }
            Unset::NotAName(name) => write!(
                to,
                "{name} is not a name a network can look up: letters, digits and hyphens, not \
                 starting or ending with one"
            ),
            Unset::Writing(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for Unset {}

impl From<console_core_atomic_writes::Unwritten> for Unset {
    fn from(fault: console_core_atomic_writes::Unwritten) -> Self {
        Unset::Writing(fault)
    }
}

fn said(program: Program, arguments: &[&str]) -> Result<String, Unset> {
    let Ok(name) = program.name();
    let mut command = match program.command() {
        Ok(command) => command,
        Err(_) => return Err(Unset::NoProgram(name)),
    };

    let out = command
        .args(arguments)
        .output()
        .map_err(|fault| Unset::WouldNotRun(name, fault))?;

    match out.status.success() {
        true => Ok(String::from_utf8_lossy(&out.stdout).to_string()),
        false => Err(Unset::SaidNo(
            name,
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        )),
    }
}

fn held(at: &Path) -> Result<String, Unset> {
    let Ok(read) = console_core_atomic_writes::read(at);

    match read {
        Stored::Text(said) => Ok(said),
        Stored::Absent => Ok(String::new()),
        Stored::Failed(fault) => Err(Unset::Read(at.display().to_string(), fault)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Locale<'a> {
    name: &'a str,
    charset: &'a str,
}

fn language(locale: Locale<'_>) -> Result<(), Unset> {
    let Locale { name, charset } = locale;
    let line = format!("{name} {charset}");
    let listed = std::fs::read_to_string(SUPPORTED)
        .map_err(|fault| Unset::Read(SUPPORTED.to_string(), fault.to_string()))?;

    let Ok(supported) = languages::supported(&listed);

    let known = supported.iter().any(|locale| {
        let Ok(said) = locale.line();

        said == line
    });

    match known {
        true => {},
        false => return Err(Unset::NotSupported(line)),
    }

    let recipes = Path::new(LOCALE_GEN);
    let was = held(recipes)?;

    let Ok(written) = languages::generating(&was, languages::Line(&line));

    match written == was {
        true => {},
        false => {
            console_core_atomic_writes::whole(recipes, written.as_bytes())?;

            println!("making {name}");

            said(Program::LocaleGen, &[])?;
        }
    }

    let read = languages::read(languages::Named { name, charset });

    let lang = match read {
        Ok(Some(locale)) => {
            let Ok(lang) = locale.lang();

            lang
        }
        Ok(None) | Err(_) => name.to_string(),
    };

    console_core_atomic_writes::whole(
        Path::new(LOCALE_CONF),
        format!("LANG={lang}\n").as_bytes(),
    )?;

    println!("{lang}");

    Ok(())
}

fn hour(zone: &str) -> Result<(), Unset> {
    let listed = said(Program::Timedatectl, &["list-timezones"])?;

    let Ok(zones) = console_settings::hours::zones(&listed);

    let known = zones.iter().any(|known| known == zone);

    match known {
        true => {},
        false => return Err(Unset::NoSuchZone(zone.to_string())),
    }

    said(Program::Timedatectl, &["set-timezone", zone])?;

    println!("{zone}");

    Ok(())
}

fn called(name: &str) -> Result<(), Unset> {
    let Ok(allowed) = named::allowed(name);

    match allowed {
        Allowed::Yes => {},
        Allowed::No => {
            return Err(Unset::NotAName(name.to_string()));
        }
    }

    said(Program::Hostnamectl, &["set-hostname", name])?;

    println!("{name}");

    Ok(())
}

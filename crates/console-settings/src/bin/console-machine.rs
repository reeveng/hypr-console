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

use console_core_atomic_writes::Held;
use console_core_external_programs::Program;
use console_settings::named::{self, Allowed};
use console_settings::tongues::{self, LOCALE_CONF, LOCALE_GEN, SUPPORTED};

const USAGE: &str = "usage: console-machine [language <name> <charset>|hour <zone>|name <name>]";

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();

    let done = match words.as_slice() {
        ["language", name, charset] => language(name, charset),
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

fn said(program: Program, argv: &[&str]) -> Result<String, String> {
    let Ok(name) = program.name();
    let mut command = match program.command() {
        Ok(command) => command,
        Err(_) => return Err(format!("there is no {name} on this machine")),
    };

    let out = command
        .args(argv)
        .output()
        .map_err(|fault| format!("{name} would not run: {fault}"))?;

    match out.status.success() {
        true => Ok(String::from_utf8_lossy(&out.stdout).to_string()),
        false => Err(format!(
            "{name} said no: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

fn held(at: &Path) -> Result<String, String> {
    let Ok(read) = console_core_atomic_writes::read(at);

    match read {
        Held::Said(said) => Ok(said),
        Held::Nothing => Ok(String::new()),
        Held::Unreadable(fault) => Err(format!("{} will not be read: {fault}", at.display())),
    }
}

fn language(name: &str, charset: &str) -> Result<(), String> {
    let line = format!("{name} {charset}");
    let listed = std::fs::read_to_string(SUPPORTED)
        .map_err(|fault| format!("{SUPPORTED} will not be read: {fault}"))?;

    let Ok(supported) = tongues::supported(&listed);

    let known = supported.iter().any(|locale| {
        let Ok(said) = locale.line();

        said == line
    });

    match known {
        true => {},
        false => return Err(format!("{line} is not a language {SUPPORTED} names")),
    }

    let recipes = Path::new(LOCALE_GEN);
    let was = held(recipes)?;

    let Ok(written) = tongues::generating(&was, &line);

    match written == was {
        true => {},
        false => {
            console_core_atomic_writes::whole(recipes, written.as_bytes())?;

            println!("making {name}");

            said(Program::LocaleGen, &[])?;
        }
    }

    let read = tongues::read(name, charset);

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

fn hour(zone: &str) -> Result<(), String> {
    let listed = said(Program::Timedatectl, &["list-timezones"])?;

    let Ok(zones) = console_settings::hours::zones(&listed);

    let known = zones.iter().any(|known| known == zone);

    match known {
        true => {},
        false => return Err(format!("{zone} is not a zone this machine has heard of")),
    }

    said(Program::Timedatectl, &["set-timezone", zone])?;

    println!("{zone}");

    Ok(())
}

fn called(name: &str) -> Result<(), String> {
    let Ok(allowed) = named::allowed(name);

    match allowed {
        Allowed::Yes => {},
        Allowed::No => {
            return Err(format!(
                "{name} is not a name a network can look up: letters, digits and hyphens, not \
                 starting or ending with one"
            ));
        }
    }

    said(Program::Hostnamectl, &["set-hostname", name])?;

    println!("{name}");

    Ok(())
}

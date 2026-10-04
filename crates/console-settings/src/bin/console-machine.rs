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

use console_core_arguments::{Operands, ValidationError, read_with};
use console_core_atomic_writes::Stored;
use console_core_external_programs::Program;
use console_settings::machine::MachineSetting;
use console_settings::named::{self, Allowed};
use console_settings::languages::{self, LOCALE_CONF, LOCALE_GEN, SUPPORTED};

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
    name: "console-machine",
    about: "the three things about this machine that a person chooses and root owns",
    flags: &[],
    operands: Operands::Any("VALUE"),
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Setting {
    Language { name: String, charset: String },
    Hour(String),
    Name(String),
}

fn setting(words: &[String]) -> Result<Setting, ValidationError> {
    let read = read_with::<MachineSetting, String>(&COMMAND, words);
    let line = read?;
    let required = line.require_subcommand();
    let chosen = required?;

    match chosen {
        MachineSetting::Language => {
            let operands = line.exactly(["NAME", "CHARSET"]);
            let [name, charset] = operands?;

            Ok(Setting::Language { name: name.clone(), charset: charset.clone() })
        }
        MachineSetting::Hour => {
            let operands = line.exactly(["ZONE"]);
            let [zone] = operands?;

            Ok(Setting::Hour(zone.clone()))
        }
        MachineSetting::Name => {
            let operands = line.exactly(["NAME"]);
            let [name] = operands?;

            Ok(Setting::Name(name.clone()))
        }
    }
}

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let chosen = match setting(&words) {
        Ok(chosen) => chosen,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };

    let done = match &chosen {
        Setting::Language { name, charset } => language(Locale { name, charset }),
        Setting::Hour(zone) => hour(zone),
        Setting::Name(name) => called(name),
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

fn run_output(program: Program, arguments: &[&str]) -> Result<String, Unset> {
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

fn read_file(at: &Path) -> Result<String, Unset> {
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
    let was = read_file(recipes)?;

    let Ok(written) = languages::generating(&was, languages::Line(&line));

    match written == was {
        true => {},
        false => {
            console_core_atomic_writes::whole(recipes, written.as_bytes())?;

            println!("making {name}");

            run_output(Program::LocaleGen, &[])?;
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
    let listed = run_output(Program::Timedatectl, &["list-timezones"])?;

    let Ok(zones) = console_settings::hours::zones(&listed);

    let known = zones.iter().any(|known| known == zone);

    match known {
        true => {},
        false => return Err(Unset::NoSuchZone(zone.to_string())),
    }

    run_output(Program::Timedatectl, &["set-timezone", zone])?;

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

    run_output(Program::Hostnamectl, &["set-hostname", name])?;

    println!("{name}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_arguments::Reason;
    use console_core_never::Never;

    fn words(said: &[&str]) -> Result<Vec<String>, Never> {
        Ok(said.iter().map(|word| (*word).to_string()).collect())
    }

    #[test]
    fn each_setting_takes_exactly_the_words_it_names() {
        let Ok(language) = words(&["language", "nl_NL.UTF-8", "UTF-8"]);
        let Ok(hour) = words(&["hour", "Europe/Amsterdam"]);
        let Ok(half) = words(&["language", "nl_NL.UTF-8"]);
        let Ok(spare) = words(&["name", "legion", "go"]);

        assert_eq!(setting(&language), Ok(Setting::Language { name: "nl_NL.UTF-8".to_string(), charset: "UTF-8".to_string() }));
        assert_eq!(setting(&hour), Ok(Setting::Hour("Europe/Amsterdam".to_string())));
        assert_eq!(setting(&half).map_err(|refusal| refusal.reason), Err(Reason::MissingOperands(vec!["CHARSET"])));
        assert_eq!(setting(&spare).map_err(|refusal| refusal.reason), Err(Reason::ExtraArgument("go".to_string())));
    }

    #[test]
    fn it_refuses_a_setting_it_does_not_own_before_it_writes_anything() {
        let Ok(owner) = words(&["owner", "root"]);

        assert_eq!(setting(&owner).map_err(|refusal| refusal.reason), Err(Reason::NoSuchSubcommand("owner".to_string())));
    }
}

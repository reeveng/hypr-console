//! Press the buttons of a Legion Go on a machine that is not one.
//!
//!     console-emulate                  make the devices and take commands
//!     console-emulate press a b        press those and stop
//!     console-emulate run scenario     play a file of the same commands
//!     console-emulate what x           what that button does, in every profile
//!     console-emulate devices          what the emulator publishes
//!
//! The devices exist for as long as the command runs and are gone when it
//! stops. While they exist they are real input devices, which means the
//! desktop in front of you is reading them: `press a` clicks whatever the
//! pointer is on.

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_pad::capture::captured;
use console_pad::devices::Devices;
use console_pad::go::{LegionGo, Passing};
use console_pad::profile::Profile;
use console_pad::router::every_profile;
use console_pad::script::{self, VERBS};
use console_pad::uinput::Uinput;

/// What the run was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Doing {
    /// Make the devices and take commands, a line at a time.
    Interactive,
    Press(Vec<String>),
    Run(PathBuf),
    /// What a button does, in every profile. Needs no devices.
    What(Vec<String>),
    /// What the emulator publishes. Needs no devices.
    Devices,
}

struct Asked {
    doing: Doing,
    profile: String,
    root: PathBuf,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(fault) => {
            eprintln!("{fault}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let asked = match read(std::env::args().skip(1).collect())? {
        None => {
            println!("{HELP}");
            return Ok(ExitCode::SUCCESS);
        }
        Some(asked) => asked,
    };

    match &asked.doing {
        Doing::What(buttons) => {
            what(buttons, &every_profile(&asked.root)?);
            return Ok(ExitCode::SUCCESS);
        }
        Doing::Devices => {
            devices();
            return Ok(ExitCode::SUCCESS);
        }
        _ => (),
    }

    let descriptors = captured().map_err(|why| format!("console-emulate: {why}"))?;
    let sink = Uinput::of(&descriptors).map_err(|fault| {
        format!(
            "console-emulate: {fault}. Tests that need no devices at all are \
             `just test`; see docs/emulator.md for the one rule that grants this."
        )
    })?;
    let mut go = LegionGo::new(
        every_profile(&asked.root)?,
        Devices::new(descriptors, sink),
        Passing,
        &asked.profile,
    )?;

    match &asked.doing {
        Doing::Press(buttons) => buttons.iter().try_for_each(|button| go.press(button))?,
        Doing::Run(scenario) => {
            let text = std::fs::read_to_string(scenario)
                .map_err(|fault| format!("{} could not be read: {fault}", scenario.display()))?;
            script::play(&mut go, &text)?;
        }
        _ => interactive(&mut go),
    }

    go.close();
    Ok(ExitCode::SUCCESS)
}

/// The arguments, read. Nothing at all is the help.
fn read(args: Vec<String>) -> Result<Option<Asked>, String> {
    let mut profile = console_pad::router::NAME.to_string();
    let mut root = PathBuf::from(".");
    let mut rest: Vec<String> = Vec::new();
    let mut waiting = args.into_iter();

    while let Some(word) = waiting.next() {
        match word.as_str() {
            "--help" | "-h" => return Ok(None),
            "--profile" => profile = waiting.next().ok_or("--profile takes a name")?,
            "--root" => root = waiting.next().ok_or("--root takes a path")?.into(),
            _ => rest.push(word),
        }
    }

    let root = match root == Path::new(".") {
        true => repository()?,
        false => root,
    };
    let named = |rest: &[String]| rest[1..].to_vec();
    let doing = match rest.first().map(String::as_str) {
        None => Doing::Interactive,
        Some("press") => Doing::Press(named(&rest)),
        Some("what") => Doing::What(named(&rest)),
        Some("devices") => Doing::Devices,
        Some("run") => Doing::Run(
            rest.get(1).ok_or("run takes a scenario to play")?.into(),
        ),
        Some(other) => return Err(format!("no such command as {other:?}\n{HELP}")),
    };
    Ok(Some(Asked { doing, profile, root }))
}

/// One command a line, until there are no more.
fn interactive<S: console_pad::devices::Sink>(go: &mut LegionGo<S, Passing>) {
    let where_: Vec<String> =
        go.devices.paths().iter().map(|(role, path)| format!("{role} at {path}")).collect();
    println!("Devices are up. {}", where_.join(", "));
    println!("One command a line, or 'help'. Control-D stops.");

    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        match line.trim() {
            "help" | "?" => println!("{VERBS}"),
            "quit" | "exit" => break,
            said => {
                if let Err(fault) = script::Step::read(said).and_then(|step| {
                    step.map_or(Ok(()), |step| step.done(go))
                }) {
                    eprintln!("{fault}");
                }
            }
        }
    }
}

/// What a button does, in every profile.
fn what(buttons: &[String], profiles: &std::collections::BTreeMap<String, Profile>) {
    for spoken in buttons {
        println!("{spoken}");

        for (name, profile) in profiles {
            let mappings = match profile.for_button(spoken) {
                Ok(mappings) => mappings,
                Err(fault) => {
                    println!("  {fault}");
                    break;
                }
            };

            if mappings.is_empty() {
                // A profile with no mappings at all passes everything through.
                // A profile with mappings that has none for this button is a
                // button that does nothing there.
                let does = match profile.mappings.is_empty() {
                    true => "passed through untouched",
                    false => "nothing",
                };
                println!("  {name:<9} {does}");
                continue;
            }

            for mapping in mappings {
                let does = match mapping.does() {
                    "" => "nothing yet",
                    said => said,
                };
                let reaches: String = mapping
                    .targets
                    .iter()
                    .map(|target| format!("[{} {}]", target.kind.said(), target.name))
                    .collect();
                println!("  {name:<9} {does}  {reaches}");
            }
        }

        println!();
    }
}

/// What the emulator publishes.
fn devices() {
    let found = match captured() {
        Ok(found) => found,
        Err(why) => {
            println!("console-emulate: {why}");
            return;
        }
    };

    for (role, descriptor) in found {
        let held = &descriptor.capabilities;
        let kinds: Vec<String> = [
            ("EV_ABS", held.abs.len()),
            ("EV_FF", held.ff.len()),
            ("EV_KEY", held.key.len()),
            ("EV_MSC", held.msc.len()),
            ("EV_REL", held.rel.len()),
        ]
        .iter()
        .filter(|(_, many)| *many > 0)
        .map(|(kind, many)| format!("{kind}×{many}"))
        .collect();
        println!("{role:<9} {:<34} {}", descriptor.name, kinds.join(", "));
    }
}

const HELP: &str = "\
console-emulate                  make the devices and take commands
console-emulate press a b        press those and stop
console-emulate run scenario     play a file of the same commands
console-emulate what x           what that button does, in every profile
console-emulate devices          what the emulator publishes

  --profile <name>              which profile the presses go through
  --root <path>                 the checkout the profiles are read from";

/// The repository this is being run inside.
fn repository() -> Result<PathBuf, String> {
    let here = std::env::current_dir().map_err(|fault| format!("no working directory: {fault}"))?;
    here.ancestors()
        .find(|at| at.join("desktop.conf").is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!("no desktop.conf above {}; run this inside the repository", here.display())
        })
}

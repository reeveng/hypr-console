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

use console_input_gamepad::capture::captured;
use console_input_gamepad::devices::Devices;
use console_input_gamepad::go::{LegionGo, Passing};
use console_input_gamepad::profile::Profile;
use console_input_gamepad::router::every_profile;
use console_input_gamepad::script::{self, VERBS};
use console_input_gamepad::uinput::Uinput;
use console_core_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Doing {
    Interactive,
    Press(Vec<String>),
    Run(PathBuf),
    What(Vec<String>),
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
    let said = read(std::env::args().skip(1).collect())?;
    let asked = match said {
        None => {
            println!("{HELP}");
            return Ok(ExitCode::SUCCESS);
        }
        Some(asked) => asked,
    };

    match &asked.doing {
        Doing::What(buttons) => {
            let profiles = every_profile(&asked.root)?;

            let Ok(()) = what(buttons, &profiles);

            return Ok(ExitCode::SUCCESS);
        }
        Doing::Devices => {
            let Ok(()) = devices();

            return Ok(ExitCode::SUCCESS);
        }
        Doing::Interactive | Doing::Press(_) | Doing::Run(_) => (),
    }

    let descriptors = captured().map_err(|why| format!("console-emulate: {why}"))?;
    let sink = Uinput::of(&descriptors).map_err(|fault| {
        format!(
            "console-emulate: {fault}. Tests that need no devices at all are \
             `just test`; see docs/emulator.md for the one rule that grants this."
        )
    })?;
    let profiles = every_profile(&asked.root)?;
    let Ok(devices) = Devices::new(descriptors, sink);

    let mut go = LegionGo::new(profiles, devices, Passing, &asked.profile)?;

    match &asked.doing {
        Doing::Press(buttons) => buttons.iter().try_for_each(|button| go.press(button))?,
        Doing::Run(scenario) => {
            let text = std::fs::read_to_string(scenario)
                .map_err(|fault| format!("{} could not be read: {fault}", scenario.display()))?;
            script::play(&mut go, &text)?;
        }
        Doing::Interactive | Doing::What(_) | Doing::Devices => {
            let Ok(()) = interactive(&mut go);
        }
    }

    let Ok(()) = go.close();

    Ok(ExitCode::SUCCESS)
}

fn read(args: Vec<String>) -> Result<Option<Asked>, String> {
    let mut profile = console_input_gamepad::router::NAME.to_string();
    let mut root = PathBuf::from(".");
    let mut rest: Vec<String> = Vec::new();
    let mut waiting = args.into_iter();

    while let Some(word) = waiting.next() {
        match word.as_str() {
            "--help" | "-h" => return Ok(None),
            "--profile" => {
                let name = waiting.next().ok_or("--profile takes a name")?;

                profile = name;
            }
            "--root" => {
                let path = waiting.next().ok_or("--root takes a path")?;

                root = path.into();
            }
            _ => rest.push(word),
        }
    }

    let root = match root == Path::new(".") {
        true => console_repository::root()?,
        false => root,
    };
    let named = |rest: &[String]| rest.get(1..).unwrap_or_default().to_vec();
    let doing = match rest.first().map(String::as_str) {
        None => Doing::Interactive,
        Some("press") => Doing::Press(named(&rest)),
        Some("what") => Doing::What(named(&rest)),
        Some("devices") => Doing::Devices,
        Some("run") => {
            let scenario = rest.get(1).ok_or("run takes a scenario to play")?;

            Doing::Run(scenario.into())
        }
        Some(other) => return Err(format!("no such command as {other:?}\n{HELP}")),
    };
    Ok(Some(Asked { doing, profile, root }))
}

fn interactive<S: console_input_gamepad::devices::Sink>(
    go: &mut LegionGo<S, Passing>,
) -> Result<(), Never> {
    let paths = go.devices.paths()?;

    let where_: Vec<String> =
        paths.iter().map(|(role, path)| format!("{role} at {path}")).collect();
    println!("Devices are up. {}", where_.join(", "));
    println!("One command a line, or 'help'. Control-D stops.");

    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        match line.trim() {
            "help" | "?" => println!("{VERBS}"),
            "quit" | "exit" => break,
            said => {
                let done = script::Step::read(said)
                    .and_then(|step| step.map_or(Ok(()), |step| step.done(go)));

                match done {
                    Ok(()) => {},
                    Err(fault) => eprintln!("{fault}"),
                }
            }
        }
    }

    Ok(())
}

fn what(
    buttons: &[String],
    profiles: &std::collections::BTreeMap<String, Profile>,
) -> Result<(), Never> {
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

            match mappings.is_empty() {
                true => {
                    let does = match profile.mappings.is_empty() {
                        true => "passed through untouched",
                        false => "nothing",
                    };
                    println!("  {name:<9} {does}");
                    continue;
                }
                false => {},
            }

            for mapping in mappings {
                let said = mapping.does()?;

                let does = match said {
                    "" => "nothing yet",
                    said => said,
                };

                let mut reaches = String::new();

                for target in &mapping.targets {
                    let kind = target.kind.said()?;

                    reaches.push_str(&format!("[{kind} {}]", target.name));
                }

                println!("  {name:<9} {does}  {reaches}");
            }
        }

        println!();
    }

    Ok(())
}

fn devices() -> Result<(), Never> {
    let found = match captured() {
        Ok(found) => found,
        Err(why) => {
            println!("console-emulate: {why}");

            return Ok(());
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

    Ok(())
}

const HELP: &str = "\
console-emulate                  make the devices and take commands
console-emulate press a b        press those and stop
console-emulate run scenario     play a file of the same commands
console-emulate what x           what that button does, in every profile
console-emulate devices          what the emulator publishes

  --profile <name>              which profile the presses go through
  --root <path>                 the checkout the profiles are read from";


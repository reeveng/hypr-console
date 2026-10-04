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
use std::path::PathBuf;
use std::process::ExitCode;

use console_input_gamepad::capture::load_capture;
use console_input_gamepad::devices::Devices;
use console_input_gamepad::go::LegionGo;
use console_waiting::clock::LiveClock;
use console_input_gamepad::profile::Profile;
use console_input_gamepad::router::every_profile;
use console_input_gamepad::script::{self, VERBS};
use console_input_gamepad::GamepadError;
use console_input_gamepad::uinput::Uinput;
use console_core_arguments::{Command, Flag, Operands, Subcommand, Takes, ValidationError, read_with};
use console_core_never::Never;
use console_core_words::Words;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    Interactive,
    ButtonPress(Vec<String>),
    Run(PathBuf),
    Describe(Vec<String>),
    Devices,
}

struct Arguments {
    effect: Action,
    profile: String,
    root: Option<PathBuf>,
}

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let Ok(code) = console_core_arguments::run_main(&COMMAND, &words, asked, run);

    code
}

#[derive(Debug)]
enum Unemulated {
    Rootless(console_repository::NotFound),
    Read(PathBuf, std::io::Error),
    NoDevices(GamepadError),
    NoUinput(GamepadError),
    Pressing(GamepadError),
}

impl std::fmt::Display for Unemulated {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unemulated::Rootless(fault) => write!(to, "{fault}"),
            Unemulated::Read(at, fault) => {
                write!(to, "{} could not be read: {fault}", at.display())
            }
            Unemulated::NoDevices(fault) => write!(to, "{fault}"),
            Unemulated::NoUinput(fault) => write!(
                to,
                "{fault}. Tests that need no devices at all are \
                 `just test`; see docs/emulator.md for the one rule that grants this."
            ),
            Unemulated::Pressing(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for Unemulated {}

impl From<console_repository::NotFound> for Unemulated {
    fn from(fault: console_repository::NotFound) -> Self {
        Unemulated::Rootless(fault)
    }
}

impl From<GamepadError> for Unemulated {
    fn from(fault: GamepadError) -> Self {
        Unemulated::Pressing(fault)
    }
}

fn run(asked: Arguments) -> Result<ExitCode, Unemulated> {
    let root = match &asked.root {
        Some(root) => root.clone(),
        None => console_repository::root()?,
    };

    match &asked.effect {
        Action::Describe(buttons) => {
            let profiles = every_profile(&root)?;

            let Ok(()) = what(buttons, &profiles);

            return Ok(ExitCode::SUCCESS);
        }
        Action::Devices => {
            let Ok(()) = devices();

            return Ok(ExitCode::SUCCESS);
        }
        Action::Interactive | Action::ButtonPress(_) | Action::Run(_) => (),
    }

    let descriptors = load_capture().map_err(Unemulated::NoDevices)?;
    let sink = Uinput::of(&descriptors).map_err(Unemulated::NoUinput)?;
    let profiles = every_profile(&root)?;
    let Ok(devices) = Devices::new(descriptors, sink);

    let Ok(clock) = LiveClock::started();
    let mut go = LegionGo::new(profiles, devices, clock, &asked.profile)?;

    match &asked.effect {
        Action::ButtonPress(buttons) => {
            for button in buttons {
                go.press(button)?;
            }
        }
        Action::Run(scenario) => {
            let text = std::fs::read_to_string(scenario)
                .map_err(|fault| Unemulated::Read(scenario.clone(), fault))?;
            script::play(&mut go, &text)?;
        }
        Action::Interactive | Action::Describe(_) | Action::Devices => {
            let Ok(()) = interactive(&mut go);
        }
    }

    let Ok(()) = go.close();

    Ok(ExitCode::SUCCESS)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
enum Verb {
    #[words(word = "press", about = "press those buttons and stop")]
    Press,
    #[words(word = "run", about = "play a file of the same commands")]
    Run,
    #[words(word = "what", about = "what those buttons do, in every profile")]
    Describe,
    #[words(word = "devices", about = "what the emulator publishes")]
    Devices,
}

impl Subcommand for Verb {
    fn variants() -> Result<impl Iterator<Item = Self>, Never> {
        Ok(Verb::VARIANTS.iter().copied())
    }

    fn spelling(self) -> Result<&'static str, Never> {
        self.word()
    }

    fn about(self) -> Result<&'static str, Never> {
        Verb::about(self)
    }
}

const PROFILE: Flag = Flag { spelling: "--profile", takes: Takes::Value("NAME"), about: "which profile the presses go through" };

const ROOT: Flag = Flag { spelling: "--root", takes: Takes::Value("PATH"), about: "the checkout the profiles are read from" };

const COMMAND: Command = Command {
    name: "console-emulate",
    about: "make the devices of a Legion Go and take commands, or do one thing with them and stop",
    flags: &[PROFILE, ROOT],
    operands: Operands::Any("BUTTON"),
};

fn asked(words: &[String]) -> Result<Arguments, ValidationError> {
    let line = read_with::<Verb, String>(&COMMAND, words)?;
    let Ok(verb) = line.subcommand();
    let Ok(operands) = line.operands();
    let Ok(profile) = line.value(PROFILE);
    let Ok(root) = line.value(ROOT);

    let effect = match verb {
        None => Action::Interactive,
        Some(Verb::Press) => Action::ButtonPress(operands.to_vec()),
        Some(Verb::Describe) => Action::Describe(operands.to_vec()),
        Some(Verb::Devices) => {
            let [] = line.exactly([])?;

            Action::Devices
        }
        Some(Verb::Run) => {
            let [scenario] = line.exactly(["SCENARIO"])?;

            Action::Run(PathBuf::from(scenario))
        }
    };

    let profile = match profile {
        Some(named) => named,
        None => console_input_gamepad::router::NAME,
    };

    Ok(Arguments { effect, profile: profile.to_string(), root: root.map(PathBuf::from) })
}

fn interactive<S: console_input_gamepad::devices::Sink>(
    go: &mut LegionGo<S, LiveClock>,
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
                let read = script::Step::read(said);

                let done = match read {
                    Ok(Some(step)) => step.run(go),
                    Ok(None) => Ok(()),
                    Err(fault) => Err(fault),
                };

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

        'over_profiles: for (name, profile) in profiles {
            let mappings = match profile.for_button(spoken) {
                Ok(mappings) => mappings,
                Err(fault) => {
                    println!("  {fault}");
                    break 'over_profiles;
                }
            };

            match mappings.is_empty() {
                true => {
                    let does = match profile.mappings.is_empty() {
                        true => "passed through untouched",
                        false => "nothing",
                    };
                    println!("  {name:<9} {does}");
                    continue 'over_profiles;
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
    let found = match load_capture() {
        Ok(found) => found,
        Err(why) => {
            println!("console-emulate: {why}");

            return Ok(());
        }
    };

    for (role, descriptor) in found {
        let held = &descriptor.capabilities;
        let kinds: Vec<String> = [
            ("EV_ABS", held.absolute.len()),
            ("EV_FF", held.force_feedback.len()),
            ("EV_KEY", held.key.len()),
            ("EV_MSC", held.miscellaneous.len()),
            ("EV_REL", held.relative.len()),
        ]
        .iter()
        .filter(|(_, many)| *many > 0)
        .map(|(kind, many)| format!("{kind}×{many}"))
        .collect();
        println!("{role:<9} {:<34} {}", descriptor.name, kinds.join(", "));
    }

    Ok(())
}



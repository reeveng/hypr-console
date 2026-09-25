//! Put this checkout on the device and bring the machine to match it.
//!
//! Everything decided is in `console_device::deploying`. What is here is the
//! half that touches this machine: the environment the device is named in, the
//! top of the tree, and the lock -- which is a `mkdir` and four small files,
//! and is the one effect no other program on this device will ever want.
//!
//! The lock is released by dropping it. A deploy that ends any way the loop
//! can see ends by returning through here, and one killed harder than that
//! leaves a directory the next run takes over because the pid in it is gone.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_device::deploying::{Alive, Deploy, DeployingEvent, Holder, DeployingEffect};
use console_device_name::device;
use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_program_contract::{Arguments, Event};
use console_program_runtime::Interpreter;

const NAMED: &str = "/proc/sys/kernel/hostname";

struct Locking {
    held: Option<PathBuf>,
}

impl Interpreter for Locking {
    type Event = DeployingEvent;
    type Effect = DeployingEffect;

    fn interpret(&mut self, acts: &DeployingEffect) -> Vec<Event<DeployingEvent>> {
        match acts {
            DeployingEffect::Take(at) => match std::fs::create_dir(at) {
                Ok(()) => {
                    self.held = Some(at.clone());

                    vec![Event::Custom(DeployingEvent::Took)]
                }
                Err(_already_held) => vec![Event::Custom(DeployingEvent::Busy)],
            },

            DeployingEffect::Mine(at) => {
                let Ok(here) = here();
                let Ok(today) = today();
                let pid = std::process::id().to_string();
                let Ok(()) = wrote(at, Line { called: "pid", said: &pid });
                let Ok(()) = wrote(at, Line { called: "on", said: &here });
                let Ok(()) = wrote(at, Line { called: "since", said: &today });

                Vec::new()
            }

            DeployingEffect::Read(at) => {
                let Ok(holding) = holding(at);

                vec![Event::Custom(DeployingEvent::Holder(holding))]
            }

            DeployingEffect::Free(at) => {
                let _ = std::fs::remove_dir_all(at);

                Vec::new()
            }
        }
    }
}

impl Drop for Locking {
    fn drop(&mut self) {
        match &self.held {
            Some(at) => {
                let _ = std::fs::remove_dir_all(at);
            }
            None => {},
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Line<'a> {
    called: &'a str,
    said: &'a str,
}

fn wrote(at: &Path, line: Line<'_>) -> Result<(), Never> {
    let Line { called, said } = line;

    let _ = console_core_atomic_writes::whole(&at.join(called), format!("{said}\n").as_bytes());

    Ok(())
}

fn said(at: &Path, called: &str) -> Result<String, Never> {
    kept(&at.join(called))
}

fn kept(at: &Path) -> Result<String, Never> {
    Ok(match console_core_atomic_writes::text_or_empty(at) {
        Ok(said) => said.trim().to_string(),
        Err(fault) => {
            eprintln!("console-deploy: {fault}");

            String::new()
        }
    })
}

fn holding(at: &Path) -> Result<Holder, Never> {
    let Ok(pid) = said(at, "pid");
    let Ok(on) = said(at, "on");
    let Ok(since) = said(at, "since");
    let Ok(here) = here();

    Ok(Holder {
        alive: match Path::new(&format!("/proc/{pid}")).is_dir() {
            true => Alive::Yes,
            false => Alive::No,
        },
        on,
        since,
        here,
        pid,
    })
}

fn host_of(words: &[String]) -> Result<String, Never> {
    Ok(match words.first() {
        Some(host) => host.clone(),
        None => String::new(),
    })
}

fn here() -> Result<String, Never> {
    kept(Path::new(NAMED))
}

fn today() -> Result<String, Never> {
    let Ok(asking) = ExternalProgram::Date.arguments(&[]);

    Ok(match console_core_external_programs::printed(&asking) {
        Ok(said) => said.trim().to_string(),
        Err(_unprinted) => String::new(),
    })
}

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the first thing this does, before a relative path has meant anything: what follows is git and cargo, which are run inside a tree rather than handed one, and `console_repository` is what says which tree that is"
    )
)]
fn main() -> ExitCode {
    let host = match device() {
        Ok(host) => host,
        Err(fault) => {
            eprintln!("console-deploy: {fault}");

            return ExitCode::FAILURE;
        }
    };
    let mut words = vec![host];

    words.extend(std::env::args().skip(1));

    let given: Vec<&str> = words.iter().map(String::as_str).collect();

    match console_repository::root() {
        Ok(root) => match std::env::set_current_dir(&root) {
            Ok(()) => {},
            Err(fault) => {
                eprintln!("console-deploy: {}: {fault}", root.display());

                return ExitCode::FAILURE;
            }
        },
        Err(fault) => {
            eprintln!("console-deploy: {fault}");

            return ExitCode::FAILURE;
        }
    }

    let Ok(host) = host_of(&words);
    let Ok(asked) = console_awake::taking_on(&host, console_awake::InhibitReason::FromDeploying);
    let _kept_up = match asked {
        console_awake::InhibitResult::Acquired(staying) => Some(staying),
        console_awake::InhibitResult::Failed(said) => {
            eprintln!("console-deploy: {said}");

            None
        }
    };

    let mut locking = Locking { held: None };

    let Ok(arguments) = Arguments::of(&given);
    let Ok(how) = console_program_runtime::run::<Deploy, Locking>(
        "console-deploy",
        &arguments,
        &mut locking,
    );

    how
}

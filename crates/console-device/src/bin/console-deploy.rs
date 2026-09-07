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

use console_device::deploying::{Alive, Deploy, Heard, Holder, Its};
use console_device::naming::device;
use console_core_external_programs::Program as Theirs;
use console_core_atomic_writes::{self, Held};
use console_core_never::Never;
use console_program_contract::{Argv, Word};
use console_program_runtime::Carrying;

const NAMED: &str = "/proc/sys/kernel/hostname";

struct Locking {
    held: Option<PathBuf>,
}

impl Carrying for Locking {
    type Hears = Heard;
    type Does = Its;

    fn its(&mut self, doing: &Its) -> Vec<Word<Heard>> {
        match doing {
            Its::Take(at) => match std::fs::create_dir(at) {
                Ok(()) => {
                    self.held = Some(at.clone());

                    vec![Word::Its(Heard::Took)]
                }
                Err(_) => vec![Word::Its(Heard::Taken)],
            },

            Its::Mine(at) => {
                let Ok(here) = here();
                let Ok(today) = today();
                let Ok(()) = wrote(at, "pid", &std::process::id().to_string());
                let Ok(()) = wrote(at, "on", &here);
                let Ok(()) = wrote(at, "since", &today);

                Vec::new()
            }

            Its::Read(at) => {
                let Ok(holding) = holding(at);

                vec![Word::Its(Heard::Holder(holding))]
            }

            Its::Free(at) => {
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

fn wrote(at: &Path, called: &str, what: &str) -> Result<(), Never> {
    let _ = std::fs::write(at.join(called), format!("{what}\n"));

    Ok(())
}

fn said(at: &Path, called: &str) -> Result<String, Never> {
    kept(&at.join(called))
}

fn kept(at: &Path) -> Result<String, Never> {
    let Ok(held) = console_core_atomic_writes::read(at);

    Ok(match held {
        Held::Said(said) => said.trim().to_string(),
        Held::Nothing => String::new(),
        Held::Unreadable(fault) => {
            eprintln!("console-deploy: {}: {fault}", at.display());

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

fn here() -> Result<String, Never> {
    kept(Path::new(NAMED))
}

fn today() -> Result<String, Never> {
    let Ok(mut date) = Theirs::Date.command();
    let said = date.output();

    Ok(match said {
        Ok(said) => String::from_utf8_lossy(&said.stdout).trim().to_string(),
        Err(_) => String::new(),
    })
}

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

    let mut locking = Locking { held: None };

    let Ok(argv) = Argv::of(&given);
    let Ok(how) = console_program_runtime::run::<Deploy, Locking>(
        "console-deploy",
        &argv,
        &mut locking,
    );

    how
}

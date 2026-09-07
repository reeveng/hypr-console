//! Whether the front of this machine has the buttons the desktop binds.
//! Everything else the engine checks is drift: the machine has wandered from
//! the manifest and `console apply` walks it back. This is not that. A device
//! without a right paddle is not going to grow one, so what is found here is
//! never counted as drift and never fails an apply -- it is said, once, in the
//! report and in a notice, and the setup screen is what settles it.  The
//! deciding is all in `console_input_gamepad`, which can be asked without a
//! machine. What is here is the machine: the bus, the kernel's list of devices,
//! and the table in somebody's home.

use std::path::Path;

use console_input_controller::means::Table;
use console_input_gamepad::front::{DEVICES, Front, asking, loading, one_said, wearing};
use console_input_gamepad::devices::Has;
use console_input_gamepad::jobs::{Jobs, Played, path_in};
use console_input_gamepad::router::{FILE, PROFILES, Router};
use console_input_gamepad::vocabulary::button_name;
use console_core_never::Never;

use crate::machine;
use crate::settled::Settled;

pub struct Standing {
    pub missing: Vec<String>,
    pub asked: bool,
    pub touchscreen: Option<bool>,
    pub told: bool,
    pub moved: usize,
}

impl Standing {
    pub fn settled(&self) -> Result<Settled, Never> {
        Ok(match self.missing.is_empty() {
            true => Settled::Yes,
            false => Settled::No,
        })
    }

    pub fn summary(&self) -> Result<String, Never> {
        Ok(match self.missing.len() {
            1 => "One thing this desktop does is on a button this device has not got".to_string(),
            many => format!(
                "{many} things this desktop does are on buttons this device has not got"
            ),
        })
    }

    pub fn body(&self) -> Result<String, Never> {
        Ok(format!("{}.\nSettings, Buttons is where they are moved.", self.missing.join("\n")))
    }
}

pub fn standing(_root: &Path, home: &str) -> Result<Standing, Never> {
    let devices = match std::fs::read_to_string(DEVICES) {
        Ok(said) => said,

        Err(fault) => {
            eprintln!("console: {DEVICES}: what this machine can be pressed with: {fault}");
            String::new()
        }
    };

    let Ok(asking) = asking();
    let Ok(asked) = machine::run(&asking);
    let Ok(front) = Front::of(&asked.out, &devices);
    let Ok(at) = path_in(home);
    let told = at.exists();
    let Ok(said) = read(home);
    let Ok(table) = Table::of(&said);
    let Ok(every) = table.every();

    let mut missing: Vec<String> = Vec::new();

    for (job, bound) in every {
        let played: Vec<&console_input_gamepad::jobs::Binding> = bound
            .iter()
            .filter(|one| {
                let Ok(played) = one.played();

                played == Played::ByAButton
            })
            .collect();

        let anywhere = played.iter().any(|one| {
            let Ok(here) = here(&front, &one.button);

            here == Has::Yes
        });

        match played.is_empty() || anywhere {
            true => continue,
            false => {},
        }

        let where_ = played.iter().map(|one| one.to_string()).collect::<Vec<_>>().join(" or ");
        let Ok(says) = job.what.says();

        missing.push(format!("{says}, on {where_}"));
    }

    Ok(Standing {
        missing,
        asked: front.capabilities.is_some(),
        touchscreen: front.touchscreen,
        told,
        moved: said.moved.len(),
    })
}

fn here(front: &Front, button: &str) -> Result<Has, Never> {
    let sends = button_name(button).is_ok_and(|named| {
        let Ok(can) = front.can_send(named);

        can == Has::Yes
    });

    Ok(match sends {
        true => Has::Yes,
        false => Has::No,
    })
}

pub fn wrote_router() -> Result<Option<String>, Never> {
    let Ok(asking) = asking();
    let Ok(asked) = machine::run(&asking);
    let Ok(front) = Front::of(&asked.out, "");

    let Some(capabilities) = front.capabilities else { return Ok(None) };

    let Ok(router) = Router::of(&capabilities);

    match !router.without.is_empty() {
        true => {
            println!(
                "this device sends buttons this desktop has no word for, so nothing can be put on them: {}",
                router.without.join(", ")
            );
        }
        false => {},
    }

    let live = format!("{PROFILES}{FILE}");

    let Ok(yaml) = router.yaml();

    Ok(match std::fs::write(&live, yaml) {
        Ok(()) => Some(live),
        Err(fault) => {
            eprintln!("{live}: {fault}");
            None
        }
    })
}

pub fn again(worn: Option<String>, still_here: impl Fn(&str) -> bool) -> Result<String, Never> {
    Ok(match worn {
        Some(path) if still_here(&path) => path,
        Some(_) | None => format!("{PROFILES}{FILE}"),
    })
}

pub fn wear_again() -> Result<(), Never> {
    let Ok(wearing) = wearing();
    let Ok(worn) = machine::run(&wearing);
    let Ok(one) = one_said(&worn.out);
    let Ok(path) = again(one, |path| std::path::Path::new(path).is_file());
    let Ok(asking) = loading(&path);
    let argv: Vec<&str> = asking.iter().map(String::as_str).collect();

    println!("the pad is reading {path} again");

    let Ok(_) = machine::run(&argv);

    Ok(())
}

pub fn read(home: &str) -> Result<Jobs, Never> {
    let Ok(at) = path_in(home);

    let Ok(said) = std::fs::read_to_string(&at) else { return Jobs::none() };

    match Jobs::read(&said) {
        Ok(jobs) => Ok(jobs),
        Err(fault) => {
            eprintln!("{}: {fault}", at.display());

            Jobs::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn again(worn: Option<String>, still_here: impl Fn(&str) -> bool) -> String {
        let Ok(again) = super::again(worn, still_here);

        again
    }

    #[test]
    fn the_pad_reads_the_profile_it_is_wearing_again() {
        let worn = format!("{PROFILES}{FILE}");
        assert_eq!(again(Some(worn.clone()), |_| true), worn);
    }

    #[test]
    fn a_profile_the_tree_no_longer_writes_is_not_handed_back() {
        let gone = format!("{PROFILES}desktop.yaml");
        assert_eq!(again(Some(gone), |_| false), format!("{PROFILES}{FILE}"));
    }

    #[test]
    fn a_pad_that_says_nothing_is_given_the_router() {
        assert_eq!(again(None, |_| true), format!("{PROFILES}{FILE}"));
    }
}

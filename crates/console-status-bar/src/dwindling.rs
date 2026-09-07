//! Watching the battery, which is the one reading nothing announces.  The sound
//! is told by pipewire and the network by NetworkManager. The battery has
//! nobody to tell it, so `watch::tick` takes a reading every thirty seconds for
//! the icon this bar draws -- and that reading is the only one on the machine.
//! Anything else wanting to know how full the battery is would be a second
//! program on a second clock, and two clocks reading one battery is two
//! machines' worth of opinions about when it crossed something.  So the
//! crossing is noticed here, where the reading already happens. What a crossing
//! is is `console_default_applications::battery`, which can be asked without a
//! machine; what is done about one is `console-battery`, which is a program of
//! its own because the last of the three stops the machine and a bar module is
//! not a thing that should be holding still for that.

use std::process::{Command, Stdio};

use console_program_lifetime::{LetGo, Still, let_go};
use console_default_applications::battery::{Charge, Levels, Step, asked};
use console_core_never::Never;
use console_notifications::saying::Kept;

pub const SAID: &str = "battery-said";

pub const DOES: &str = "console-battery";

#[derive(Debug, Default)]
pub struct Watching {
    doing: Option<LetGo>,
}

impl Watching {
    pub fn seen(&mut self, said: &str) -> Result<(), Never> {
        let Ok(()) = self.reap();
        let reading = Charge::of(said)?;

        let Some(charge) = reading.percent else { return Ok(()) };

        let Ok(kept) = Kept::named(SAID);
        let Ok(read) = kept.read();

        let told = match read {
            Some(number) => Step::of_number(number)?,
            None => None,
        };

        let levels = Levels::here()?;
        let said = asked(levels, charge, reading.filling, told)?;

        let number = match said.told {
            Some(step) => step.number()?,
            None => 0,
        };

        match said.act {
            Some(_) if self.doing.is_some() => (),
            Some(step) => {
                let Ok(started) = run(step);

                self.doing = started;

                let Ok(()) = kept.write(number);
            }
            None => {
                let Ok(()) = kept.write(number);
            }
        }

        Ok(())
    }

    fn reap(&mut self) -> Result<(), Never> {
        let Some(doing) = &mut self.doing else { return Ok(()) };

        let Ok(still) = doing.still();

        match still {
            Still::Ended => self.doing = None,
            Still::Running => {}
        }

        Ok(())
    }
}

fn run(step: Step) -> Result<Option<LetGo>, Never> {
    let word = step.word()?;
    let mut starting = Command::new(DOES);

    starting.arg(word).stdout(Stdio::null()).stderr(Stdio::null());

    Ok(match let_go(&mut starting) {
        Ok(child) => Some(child),
        Err(fault) => {
            eprintln!("{DOES} would not start for {word}: {fault}");
            None
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_machine_with_no_battery_is_left_alone() {
        let mut watching = Watching::default();
        let Ok(()) = watching.seen("");
        assert!(watching.doing.is_none());
    }

    #[test]
    fn every_step_is_asked_for_by_its_own_word() {
        for step in console_default_applications::battery::EVERY {
            let Ok(word) = step.word();

            assert!(!word.is_empty());
        }
        assert_eq!(DOES, "console-battery");
    }
}

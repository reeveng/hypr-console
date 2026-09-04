//! Watching the battery, which is the one reading nothing announces.
//!
//! The sound is told by pipewire and the network by NetworkManager. The
//! battery has nobody to tell it, so `watch::tick` takes a reading every
//! thirty seconds for the icon this bar draws -- and that reading is the only
//! one on the machine. Anything else wanting to know how full the battery is
//! would be a second program on a second clock, and two clocks reading one
//! battery is two machines' worth of opinions about when it crossed something.
//!
//! So the crossing is noticed here, where the reading already happens. What a
//! crossing is is `console_defaults::battery`, which can be asked without a
//! machine; what is done about one is `console-battery`, which is a program of
//! its own because the last of the three stops the machine and a bar module is
//! not a thing that should be holding still for that.

use std::process::{Child, Command, Stdio};

use console_defaults::battery::{Charge, Levels, Step, asked};
use console_notices::saying::Kept;

/// Where what has already been said is kept between readings.
///
/// In the runtime directory, so a reboot forgets it. waybar starts a module
/// again the moment it exits, and without this a bar rebuilt at nineteen per
/// cent would say the battery was getting low every time it was rebuilt.
pub const SAID: &str = "battery-said";

/// The program that does something about it.
pub const DOES: &str = "console-battery";

/// The watching, which is one number and at most one program.
#[derive(Debug, Default)]
pub struct Watching {
    /// What is being done about the last crossing, while it is still going on.
    ///
    /// Kept rather than let go of, for two reasons that happen to agree. A
    /// child nobody waits on is a zombie, and this process is meant to live as
    /// long as the session. And the last of the three steps waits a quarter of
    /// a minute under a card before it stops the machine, so a second one
    /// started over the top of it would be two countdowns for one battery.
    doing: Option<Child>,
}

impl Watching {
    /// One reading, and whatever it comes to.
    ///
    /// Handed the same line the icon is drawn from, so the two cannot disagree
    /// about what the battery said.
    pub fn seen(&mut self, said: &str) {
        self.reap();
        let reading = Charge::of(said);

        let Some(charge) = reading.percent else { return };

        let kept = Kept::named(SAID);
        let told = kept.read().and_then(Step::of_number);
        let said = asked(Levels::here(), charge, reading.filling, told);

        match said.act {
            // Something to do, and something already being done about the
            // step before it. What was told is left where it was, so the next
            // reading asks again rather than this one being lost.
            Some(_) if self.doing.is_some() => (),
            Some(step) => {
                self.doing = run(step);
                kept.write(said.told.map_or(0, Step::number));
            }
            None => kept.write(said.told.map_or(0, Step::number)),
        }
    }

    /// Let go of the last one, once it has finished.
    fn reap(&mut self) {
        let done = match &mut self.doing {
            Some(doing) => matches!(doing.try_wait(), Ok(Some(_)) | Err(_)),
            None => false,
        };

        if done {
            self.doing = None;
        }
    }
}

/// Started and let go of until the next reading.
///
/// Its output goes nowhere on purpose: this process writes one line of JSON to
/// its own stdout for every change the bar draws, and a word from a child on
/// the same handle is a bar module that stops drawing.
fn run(step: Step) -> Option<Child> {
    let started = Command::new(DOES)
        .arg(step.word())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    match started {
        Ok(child) => Some(child),
        // Nothing to hold on to, and nothing this can do about it either: the
        // step has been reached and the program that answers for it is not on
        // the machine. Said where the journal keeps it, because a battery
        // warning that never appears is not something anybody notices until
        // the machine has gone off in their hands.
        Err(fault) => {
            eprintln!("{DOES} would not start for {}: {fault}", step.word());
            None
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine with no battery is not a machine at nought per cent, and
    /// nothing here happens on one. Every laptop this could be installed on
    /// has a battery and the one it was written for has one, so the case that
    /// has to be right is the desktop nobody has tried it on yet.
    #[test]
    fn a_machine_with_no_battery_is_left_alone() {
        let mut watching = Watching::default();
        watching.seen("");
        assert!(watching.doing.is_none());
    }

    /// The program is named once, here, and asked for by the step's own word,
    /// so a step added to the table is a step this can already run.
    #[test]
    fn every_step_is_asked_for_by_its_own_word() {
        for step in console_defaults::battery::EVERY {
            assert!(!step.word().is_empty());
        }
        assert_eq!(DOES, "console-battery");
    }
}

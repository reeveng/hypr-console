//! Whether the front of this machine has the buttons the desktop binds.
//!
//! Everything else the engine checks is drift: the machine has wandered from
//! the manifest and `console apply` walks it back. This is not that. A device
//! without a right paddle is not going to grow one, so what is found here is
//! never counted as drift and never fails an apply -- it is said, once, in the
//! report and in a notice, and the setup screen is what settles it.
//!
//! The deciding is all in `console_pad`, which can be asked without a machine.
//! What is here is the machine: the bus, the kernel's list of devices, and the
//! table in somebody's home.

use std::path::Path;

use console_controller::means::Table;
use console_pad::asking::Asking;
use console_pad::front::{DEVICES, Front, asking, loading, one_said, wearing};
use console_pad::devices::Has;
use console_pad::jobs::{Jobs, Played, path_in};
use console_pad::router::{FILE, PROFILES, Router};
use console_pad::vocabulary::button_name;

use crate::machine;
use crate::settled::Settled;

/// Where the desktop and the device disagree about what can be pressed.
pub struct Standing {
    /// What this desktop does that no button on this machine can reach, after
    /// whatever somebody has moved.
    ///
    /// Said in words rather than as jobs, because what is done with it is
    /// printed: the report says it and so does the notice after an apply.
    pub missing: Vec<String>,
    /// Whether the machine answered at all.
    pub asked: bool,
    /// Whether there is a screen a finger can drive this desktop from.
    pub touchscreen: Option<bool>,
    /// Whether anybody has already said where the buttons are on this device.
    ///
    /// Whether the file exists, not whether it moved anything. Somebody who
    /// walked through the setup screen and left a button where it was has
    /// answered the question, and a machine that asked again every apply would
    /// be a machine that had not listened.
    pub told: bool,
    /// How many buttons the table moves.
    pub moved: usize,
}

impl Standing {
    /// Nothing to say: every button the desktop binds is on this machine.
    pub fn settled(&self) -> Settled {
        match self.missing.is_empty() {
            true => Settled::Yes,
            false => Settled::No,
        }
    }

    /// The one line a notice leads with.
    pub fn summary(&self) -> String {
        match self.missing.len() {
            1 => "One thing this desktop does is on a button this device has not got".to_string(),
            many => format!(
                "{many} things this desktop does are on buttons this device has not got"
            ),
        }
    }

    /// What is missing, and what is lost with it, in the words the setup
    /// screen says them in.
    pub fn body(&self) -> String {
        format!("{}.\nSettings, Buttons is where they are moved.", self.missing.join("\n"))
    }
}

/// What the machine says about itself, asked here and decided in the tables.
///
/// The question used to be asked of the profiles: they named the buttons this
/// desktop bound, and a device without one of them was a device missing a
/// button. The profile is made out of the device now and so names only buttons
/// it has, which would make that question answer itself. What is asked instead
/// is the honest version of it: of everything this desktop does, what is on a
/// button nothing on this machine can press?
pub fn standing(_root: &Path, home: &str) -> Standing {
    // Nothing at all where the kernel's list will not open. `Front::of` reads
    // an empty answer as a machine that was not asked rather than as a machine
    // with no touchscreen, which is the honest reading of both.
    let devices = match std::fs::read_to_string(DEVICES) {
        Ok(said) => said,

        Err(fault) => {
            eprintln!("console: {DEVICES}: what this machine can be pressed with: {fault}");
            String::new()
        }
    };

    let front = Front::of(&machine::run(&asking()).out, &devices);
    let told = path_in(home).exists();
    let said = read(home);
    let table = Table::of(&said);
    let mut missing: Vec<String> = Vec::new();

    for (job, bound) in table.every() {
        // A job on two buttons is reachable if either of them is here, and a
        // job somebody has taken the button off is not missing: it is where
        // they put it. Only a job whose every binding names a button this
        // machine has not got is something to say out loud.
        let played: Vec<&console_pad::jobs::Binding> =
            bound.iter().filter(|one| one.played() == Played::ByAButton).collect();

        if played.is_empty() || played.iter().any(|one| here(&front, &one.button) == Has::Yes) {
            continue;
        }

        let where_ = played.iter().map(|one| one.to_string()).collect::<Vec<_>>().join(" or ");
        missing.push(format!("{}, on {where_}", job.what.says()));
    }

    Standing {
        missing,
        asked: front.capabilities.is_some(),
        touchscreen: front.touchscreen,
        told,
        moved: said.moved.len(),
    }
}

/// Whether this machine has the button a binding names.
fn here(front: &Front, button: &str) -> Has {
    match button_name(button).is_ok_and(|named| front.can_send(named) == Has::Yes) {
        true => Has::Yes,
        false => Has::No,
    }
}

/// Write the profile this desktop is driven by, out of this device's buttons.
///
/// Made here rather than kept in the tree for the same reason the asking
/// profile is: what it holds is one device's buttons, and the tree is what
/// every machine running this desktop has in common. A handheld with no
/// paddles gets a profile with no paddles in it, and the jobs that were on
/// them are answered on the setup screen rather than bound to something nobody
/// can press.
///
/// A machine that would not say what it has keeps whatever profile it already
/// had. A profile written out of no answer would be a profile claiming this
/// device has no buttons, which is a machine nobody can drive.
pub fn wrote_router() -> Option<String> {
    let front = Front::of(&machine::run(&asking()).out, "");
    let capabilities = front.capabilities?;
    let router = Router::of(&capabilities);

    if !router.without.is_empty() {
        println!(
            "this device sends buttons this desktop has no word for, so nothing can be put on them: {}",
            router.without.join(", ")
        );
    }

    let live = format!("{PROFILES}{FILE}");

    match std::fs::write(&live, router.yaml()) {
        Ok(()) => Some(live),
        Err(fault) => {
            eprintln!("{live}: {fault}");
            None
        }
    }
}

/// Write the profile the setup screen asks its question with.
///
/// Made here rather than kept in the tree, because what it holds is one
/// device's buttons and the tree is what every machine has in common. It maps
/// every button this hardware says it can send to a key nothing is listening
/// for, so that while the question is on the screen a press says which button
/// it was and does nothing else at all.
///
/// A machine that would not say what it has is left with whatever profile it
/// already had, and the setup screen is what finds out: a file written out of
/// no answer would be a profile claiming this device has no buttons.
pub fn wrote_asking() -> Option<String> {
    let front = Front::of(&machine::run(&asking()).out, "");
    let capabilities = front.capabilities?;
    let live = format!("{PROFILES}asking.yaml");

    match std::fs::write(&live, Asking::of(&capabilities).yaml()) {
        Ok(()) => Some(live),
        Err(fault) => {
            eprintln!("{live}: {fault}");
            None
        }
    }
}

/// Which profile to have the pad read again, given what it is wearing.
///
/// Whatever it is wearing, as long as that is still a file. An apply has just
/// rewritten the profiles underneath a pad that has one of them loaded, and
/// nothing watching *which* profile is on -- the controller daemon included --
/// has anything to notice, because the name has not changed. So it is asked
/// for by path.
///
/// Anything else is the router, and that case is not hypothetical: the day the
/// two hand-written profiles became one generated one, the machine was wearing
/// `desktop.yaml`, an apply told InputPlumber to read a file this desktop no
/// longer writes, and the pad was left on the old meanings until the daemon
/// restarted a moment later and put it right. A profile the tree has stopped
/// shipping is not a profile to hand back to the machine.
pub fn again(worn: Option<String>, still_here: impl Fn(&str) -> bool) -> String {
    match worn {
        Some(path) if still_here(&path) => path,
        _ => format!("{PROFILES}{FILE}"),
    }
}

/// Ask InputPlumber to read the profile on the pad again.
pub fn wear_again() {
    let path = again(one_said(&machine::run(&wearing()).out), |path| {
        std::path::Path::new(path).is_file()
    });
    let asking = loading(&path);
    let argv: Vec<&str> = asking.iter().map(String::as_str).collect();
    println!("the pad is reading {path} again");
    machine::run(&argv);
}

/// What this device's owner has moved, or nothing, which is what a machine
/// nobody has touched has.
///
/// A table that will not parse is not a reason to stop an apply, and it is
/// also not something to swallow: what it says goes to the person running the
/// apply, and the machine goes on as though nothing had been moved. That is
/// the safe half of the choice, because where this desktop puts a job is what
/// it was built around.
pub fn read(home: &str) -> Jobs {
    let at = path_in(home);

    let Ok(said) = std::fs::read_to_string(&at) else { return Jobs::none() };

    match Jobs::read(&said) {
        Ok(jobs) => jobs,
        Err(fault) => {
            eprintln!("{}: {fault}", at.display());
            Jobs::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An apply rewrites the profiles under a pad that has one loaded, and the
    /// name has not changed, so the only way to make the new file take is to
    /// ask for the one it is already wearing.
    #[test]
    fn the_pad_reads_the_profile_it_is_wearing_again() {
        let worn = format!("{PROFILES}{FILE}");
        assert_eq!(again(Some(worn.clone()), |_| true), worn);
    }

    /// And not a profile this desktop has stopped shipping. The day the two
    /// hand-written profiles became one generated one, the machine was wearing
    /// `desktop.yaml` and an apply handed it straight back.
    #[test]
    fn a_profile_the_tree_no_longer_writes_is_not_handed_back() {
        let gone = format!("{PROFILES}desktop.yaml");
        assert_eq!(again(Some(gone), |_| false), format!("{PROFILES}{FILE}"));
    }

    /// A machine that would not say what it is wearing is a machine that gets
    /// the profile it should be wearing.
    #[test]
    fn a_pad_that_says_nothing_is_given_the_router() {
        assert_eq!(again(None, |_| true), format!("{PROFILES}{FILE}"));
    }
}

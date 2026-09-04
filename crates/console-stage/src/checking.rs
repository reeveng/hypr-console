//! Reading the checks, and running them somewhere.
//!
//! A check is one thing, and one feature. It says what somebody did and what
//! should have happened, and it is edited in place when the feature changes
//! rather than joined by a second one saying something different. Running them
//! in order walks everything this desktop has grown, oldest first, and says
//! which of it still works.
//!
//! Large features are split, because "the d-pad works" is not a thing that
//! fails: left works or right works, and a check that presses both and asserts
//! once tells you neither which failed nor that only one did.

use crate::desktop::Desktop;
use crate::device::{Device, Seen, Waited};
use crate::here::Here;

/// Why a check did not pass, or did not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    /// Nothing here can answer this, and saying so is the answer.
    Cannot(String),
    Failed(String),
}

/// Anything a stage refused to do is a failure of the check that asked.
impl From<String> for Why {
    fn from(said: String) -> Self {
        Why::Failed(said)
    }
}

/// What a check comes to.
pub type Done = Result<(), Why>;

/// Something that cannot be asked here.
pub fn cannot(why: &str) -> Done {
    Err(Why::Cannot(why.to_string()))
}

/// Something that should have been true and was not.
pub fn failed(why: String) -> Done {
    Err(Why::Failed(why))
}

/// Two things that had to be the same.
///
/// This and the checks below it replace one `ought(so, why)` that took the
/// answer already worked out. A bool parameter is a place a reader has to go
/// and find out what the truth of it meant, and `ought(stage.drawn(...), ...)`
/// was exactly that: the name says what was asked, not what a yes would be.
/// Named checks put the question in the call, and the reason stays a closure so
/// nothing is built for a check that passes.
pub fn same<T, U>(got: &T, wanted: &U, why: impl FnOnce() -> String) -> Done
where
    T: PartialEq<U> + ?Sized,
    U: ?Sized,
{
    match got == wanted {
        true => Ok(()),
        false => failed(why()),
    }
}

/// Two things that had to differ, which is how a check says something moved.
pub fn not_same<T, U>(got: &T, than: &U, why: impl FnOnce() -> String) -> Done
where
    T: PartialEq<U> + ?Sized,
    U: ?Sized,
{
    match got == than {
        true => failed(why()),
        false => Ok(()),
    }
}

/// A number that had to have gone up.
///
/// By value where `same` takes a reference, because these are only ever
/// numbers: a count the reason line also names is copied rather than moved out
/// from under it.
pub fn more_than<T, U>(got: T, than: U, why: impl FnOnce() -> String) -> Done
where
    T: PartialOrd<U>,
{
    match got > than {
        true => Ok(()),
        false => failed(why()),
    }
}

/// A number that had to have gone down.
pub fn less_than<T, U>(got: T, than: U, why: impl FnOnce() -> String) -> Done
where
    T: PartialOrd<U>,
{
    match got < than {
        true => Ok(()),
        false => failed(why()),
    }
}

/// Nothing at all, which is what a check for something not happening asks.
pub fn empty<T>(things: &[T], why: impl FnOnce() -> String) -> Done {
    match things.is_empty() {
        true => Ok(()),
        false => failed(why()),
    }
}

/// Something rather than nothing.
pub fn not_empty<T>(things: &[T], why: impl FnOnce() -> String) -> Done {
    match things.is_empty() {
        true => failed(why()),
        false => Ok(()),
    }
}

/// Every one of them the same, and at least one of them.
///
/// Nothing is not every: a list that came back empty is a question that was
/// never answered, and passing it would be the check saying yes about a machine
/// it never reached.
pub fn every<T, U>(things: &[T], wanted: U, why: impl FnOnce() -> String) -> Done
where
    T: PartialEq<U>,
    U: Copy,
{
    match !things.is_empty() && things.iter().all(|thing| *thing == wanted) {
        true => Ok(()),
        false => failed(why()),
    }
}

/// Something that had to be on the screen when it was looked for.
pub fn seen(seen: Seen, why: impl FnOnce() -> String) -> Done {
    match seen {
        Seen::Yes => Ok(()),
        Seen::NotYet => failed(why()),
    }
}

/// Something that had to happen before the time given for it ran out.
pub fn happened(waited: Waited, why: impl FnOnce() -> String) -> Done {
    match waited {
        Waited::Happened => Ok(()),
        Waited::RanOut => failed(why()),
    }
}

/// The check as written for one stage.
pub enum Body {
    Desktop(fn(&mut Desktop) -> Done),
    Device(fn(&mut Device) -> Done),
    Here(fn(&mut Here) -> Done),
}

/// One check.
pub struct Check {
    /// What it is called, beginning with when it arrived.
    pub name: &'static str,
    /// One line about itself.
    pub about: &'static str,
    /// The feature it is one part of, so a large one can be split.
    pub feature: &'static str,
    pub since: &'static str,
    pub bodies: &'static [Body],
}

impl Check {
    /// When it arrived, which is the number it opens with.
    pub fn number(&self) -> &str {
        self.name.split('-').next().unwrap_or_default()
    }

    /// The rest of the name, which is what it is about.
    pub fn rest(&self) -> &str {
        self.name.split_once('-').map_or("", |(_, rest)| rest)
    }

    /// Whether one of the words names this check or its feature.
    pub fn named_by(&self, words: &[String]) -> Named {
        let any = words.iter().any(|word| self.name.contains(word.as_str()) || word == self.feature);

        match any {
            true => Named::Yes,
            false => Named::No,
        }
    }

    /// Where this can be answered without the machine, if anywhere.
    ///
    /// Derived from what it is written for rather than declared beside it, so a
    /// check that grows an emulator body stops being the machine's business the
    /// same moment, with nobody having to remember to say so.
    pub fn without_the_device(&self) -> Option<Stage> {
        self.body(Stage::Here)
            .map(|_| Stage::Here)
            .or_else(|| self.body(Stage::Desktop).map(|_| Stage::Desktop))
    }

    fn body(&self, stage: Stage) -> Option<&Body> {
        self.bodies.iter().find(|body| {
            matches!(
                (body, stage),
                (Body::Desktop(_), Stage::Desktop)
                    | (Body::Device(_), Stage::Device)
                    | (Body::Here(_), Stage::Here)
            )
        })
    }
}

/// Whether a check is one of the ones somebody asked for by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    /// A word they gave names this check or the feature it is part of.
    Yes,
    /// None of them do, so this run is not about it.
    No,
}

/// Which of the three a check is being run in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Desktop,
    Device,
    Here,
}

impl Stage {
    pub fn name(self) -> &'static str {
        match self {
            Stage::Desktop => "desktop",
            Stage::Device => "device",
            Stage::Here => "here",
        }
    }
}

/// How a check ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum How {
    Ok,
    Skipped(String),
    Failed(String),
    /// Nothing is judged on a dry run. The machine answers nothing, so every
    /// assertion is about emptiness and would fail for a reason that is not
    /// about the desktop. What a dry run is for is reading the commands before
    /// they are sent.
    Would,
}

impl How {
    pub fn name(&self) -> &'static str {
        match self {
            How::Ok => "ok",
            How::Skipped(_) => "skipped",
            How::Failed(_) => "failed",
            How::Would => "would",
        }
    }

    pub fn why(&self) -> &str {
        match self {
            How::Failed(why) | How::Skipped(why) => why,
            _ => "",
        }
    }
}

fn ended(done: Done) -> How {
    match done {
        Ok(()) => How::Ok,
        Err(Why::Cannot(why)) => How::Skipped(why),
        Err(Why::Failed(why)) => How::Failed(why),
    }
}

/// One check, here.
pub fn here(check: &Check, stage: &mut Here) -> How {
    let Some(Body::Here(body)) = check.body(Stage::Here) else {
        return How::Skipped("nothing written for here".to_string());
    };

    // One check's idea of what has been run is its own.
    stage.fresh();
    ended(body(stage))
}

/// One check, on the machine.
pub fn device(check: &Check, stage: &mut Device) -> How {
    let Some(Body::Device(body)) = check.body(Stage::Device) else {
        return How::Skipped("nothing written for device".to_string());
    };

    // A chooser some earlier check left drawn is not scenery.
    stage.fresh();

    match (stage.dry, body(stage)) {
        // Something nothing can do is still nothing anybody can do.
        (true, Err(Why::Cannot(why))) => How::Skipped(why),
        (true, _) => How::Would,
        (false, done) => ended(done),
    }
}

/// One check, against a nested desktop.
pub fn desktop(check: &Check, stage: &mut Desktop) -> How {
    let Some(Body::Desktop(body)) = check.body(Stage::Desktop) else {
        return How::Skipped("nothing written for desktop".to_string());
    };

    stage.fresh();
    ended(body(stage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::Dry;

    const ONE: Check = Check {
        name: "010-workspaces-right",
        about: "R1 moves to the next workspace.",
        feature: "workspaces",
        since: "2026-08-24",
        bodies: &[Body::Here(|_| Ok(()))],
    };

    #[test]
    fn a_check_says_when_it_arrived_and_what_it_is_about() {
        assert_eq!(ONE.number(), "010");
        assert_eq!(ONE.rest(), "workspaces-right");
    }

    #[test]
    fn a_check_is_found_by_its_name_or_by_its_feature() {
        assert_eq!(ONE.named_by(&["workspaces".to_string()]), Named::Yes);
        assert_eq!(ONE.named_by(&["010".to_string()]), Named::Yes);
        assert_eq!(ONE.named_by(&["keyboard".to_string()]), Named::No);
    }

    /// What the machine alone can answer is read off what the check is written
    /// for, so nothing has to be kept in step by hand.
    #[test]
    fn a_check_written_for_somewhere_else_is_not_the_machines_business() {
        const BOTH: Check = Check {
            bodies: &[Body::Here(|_| Ok(())), Body::Device(|_| Ok(()))],
            ..ONE
        };
        const DRAWN: Check = Check {
            bodies: &[Body::Desktop(|_| Ok(())), Body::Device(|_| Ok(()))],
            ..ONE
        };
        const THERE: Check = Check { bodies: &[Body::Device(|_| Ok(()))], ..ONE };

        assert_eq!(THERE.without_the_device(), None);

        assert_eq!(BOTH.without_the_device(), Some(Stage::Here));

        assert_eq!(DRAWN.without_the_device(), Some(Stage::Desktop));
    }

    /// A stage nothing is written for skips it and says so rather than passing
    /// quietly.
    #[test]
    fn a_stage_nothing_is_written_for_says_so() {
        let mut nowhere = Desktop::new();
        assert_eq!(desktop(&ONE, &mut nowhere).name(), "skipped");
    }

    /// Nothing is judged on a dry run, but something nothing can do is still
    /// nothing anybody can do.
    #[test]
    fn a_dry_run_would_run_what_it_can_and_skips_what_it_cannot() {
        const CANNOT: Check = Check {
            name: "010-nothing",
            about: "Nothing.",
            feature: "nothing",
            since: "2026-08-29",
            bodies: &[Body::Device(|_| cannot("a thumb is wanted"))],
        };
        const FAILS: Check = Check { bodies: &[Body::Device(|_| failed("no".to_string()))], ..ONE };
        let mut dry = Device::new("nowhere", Dry::Pretend).expect("a stage");
        assert_eq!(device(&CANNOT, &mut dry), How::Skipped("a thumb is wanted".to_string()));
        assert_eq!(device(&FAILS, &mut dry), How::Would);
    }
}

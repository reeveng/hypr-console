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
use crate::device::Device;
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

/// Something that should have been true.
///
/// The reason is built only when it is needed, so a check can say what it saw
/// without asking the machine for it twice.
pub fn ought(so: bool, why: impl FnOnce() -> String) -> Done {
    match so {
        true => Ok(()),
        false => failed(why()),
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
    pub fn named_by(&self, words: &[String]) -> bool {
        words.iter().any(|word| self.name.contains(word.as_str()) || word == self.feature)
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

    pub fn settled(&self) -> bool {
        matches!(self, How::Ok | How::Would | How::Skipped(_))
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
        assert!(ONE.named_by(&["workspaces".to_string()]));
        assert!(ONE.named_by(&["010".to_string()]));
        assert!(!ONE.named_by(&["keyboard".to_string()]));
    }

    /// A stage nothing is written for skips it and says so rather than passing
    /// quietly.
    #[test]
    fn a_stage_nothing_is_written_for_says_so() {
        let mut nowhere = Desktop::new();
        assert_eq!(desktop(&ONE, &mut nowhere).name(), "skipped");
    }

    #[test]
    fn a_skip_and_a_pass_are_both_settled_and_a_failure_is_not() {
        assert!(How::Ok.settled());
        assert!(How::Skipped("nothing written".to_string()).settled());
        assert!(!How::Failed("it did not".to_string()).settled());
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
        let mut dry = Device::new("nowhere", true).expect("a stage");
        assert_eq!(device(&CANNOT, &mut dry), How::Skipped("a thumb is wanted".to_string()));
        assert_eq!(device(&FAILS, &mut dry), How::Would);
    }
}

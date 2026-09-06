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

use console_never::Never;

use crate::desktop::Desktop;
use crate::device::{Device, Seen, Waited};
use crate::here::Here;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    Cannot(String),
    Failed(String),
}

impl From<String> for Why {
    fn from(said: String) -> Self {
        Why::Failed(said)
    }
}

pub type Done = Result<(), Why>;

pub fn cannot(why: &str) -> Done {
    Err(Why::Cannot(why.to_string()))
}

pub fn failed(why: String) -> Done {
    Err(Why::Failed(why))
}

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

pub fn more_than<T, U>(got: T, than: U, why: impl FnOnce() -> String) -> Done
where
    T: PartialOrd<U>,
{
    match got > than {
        true => Ok(()),
        false => failed(why()),
    }
}

pub fn less_than<T, U>(got: T, than: U, why: impl FnOnce() -> String) -> Done
where
    T: PartialOrd<U>,
{
    match got < than {
        true => Ok(()),
        false => failed(why()),
    }
}

pub fn empty<T>(things: &[T], why: impl FnOnce() -> String) -> Done {
    match things.is_empty() {
        true => Ok(()),
        false => failed(why()),
    }
}

pub fn not_empty<T>(things: &[T], why: impl FnOnce() -> String) -> Done {
    match things.is_empty() {
        true => failed(why()),
        false => Ok(()),
    }
}

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

pub fn seen(seen: Seen, why: impl FnOnce() -> String) -> Done {
    match seen {
        Seen::Yes => Ok(()),
        Seen::NotYet => failed(why()),
    }
}

pub fn happened(waited: Waited, why: impl FnOnce() -> String) -> Done {
    match waited {
        Waited::Happened => Ok(()),
        Waited::RanOut => failed(why()),
    }
}

pub enum Body {
    Desktop(fn(&mut Desktop) -> Done),
    Device(fn(&mut Device) -> Done),
    Here(fn(&mut Here) -> Done),
}

pub struct Check {
    pub name: &'static str,
    pub about: &'static str,
    pub feature: &'static str,
    pub since: &'static str,
    pub bodies: &'static [Body],
}

impl Check {
    pub fn number(&self) -> Result<&str, Never> {
        Ok(self.name.split('-').next().unwrap_or_default())
    }

    pub fn rest(&self) -> Result<&str, Never> {
        Ok(self.name.split_once('-').map_or("", |(_, rest)| rest))
    }

    pub fn named_by(&self, words: &[String]) -> Result<Named, Never> {
        let any = words.iter().any(|word| self.name.contains(word.as_str()) || word == self.feature);

        Ok(match any {
            true => Named::Yes,
            false => Named::No,
        })
    }

    pub fn without_the_device(&self) -> Result<Option<Stage>, Never> {
        let Ok(here) = self.body(Stage::Here);
        let Ok(desktop) = self.body(Stage::Desktop);

        Ok(here.map(|_| Stage::Here).or_else(|| desktop.map(|_| Stage::Desktop)))
    }

    fn body(&self, stage: Stage) -> Result<Option<&Body>, Never> {
        Ok(self.bodies.iter().find(|body| {
            matches!(
                (body, stage),
                (Body::Desktop(_), Stage::Desktop)
                    | (Body::Device(_), Stage::Device)
                    | (Body::Here(_), Stage::Here)
            )
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Desktop,
    Device,
    Here,
}

impl Stage {
    pub fn name(self) -> Result<&'static str, Never> {
        Ok(match self {
            Stage::Desktop => "desktop",
            Stage::Device => "device",
            Stage::Here => "here",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum How {
    Ok,
    Skipped(String),
    Failed(String),
    Would,
}

impl How {
    pub fn name(&self) -> Result<&'static str, Never> {
        Ok(match self {
            How::Ok => "ok",
            How::Skipped(_) => "skipped",
            How::Failed(_) => "failed",
            How::Would => "would",
        })
    }

    pub fn why(&self) -> Result<&str, Never> {
        Ok(match self {
            How::Failed(why) | How::Skipped(why) => why,
            How::Ok | How::Would => "",
        })
    }
}

fn ended(done: Done) -> Result<How, Never> {
    Ok(match done {
        Ok(()) => How::Ok,
        Err(Why::Cannot(why)) => How::Skipped(why),
        Err(Why::Failed(why)) => How::Failed(why),
    })
}

pub fn here(check: &Check, stage: &mut Here) -> Result<How, Never> {
    let Ok(found) = check.body(Stage::Here);

    let Some(Body::Here(body)) = found else {
        return Ok(How::Skipped("nothing written for here".to_string()));
    };

    let Ok(()) = stage.fresh();

    ended(body(stage))
}

pub fn device(check: &Check, stage: &mut Device) -> Result<How, Never> {
    let Ok(found) = check.body(Stage::Device);

    let Some(Body::Device(body)) = found else {
        return Ok(How::Skipped("nothing written for device".to_string()));
    };

    let Ok(()) = stage.fresh();

    match (stage.dry, body(stage)) {
        (true, Err(Why::Cannot(why))) => Ok(How::Skipped(why)),
        (true, _) => Ok(How::Would),
        (false, done) => ended(done),
    }
}

pub fn desktop(check: &Check, stage: &mut Desktop) -> Result<How, Never> {
    let Ok(found) = check.body(Stage::Desktop);

    let Some(Body::Desktop(body)) = found else {
        return Ok(How::Skipped("nothing written for desktop".to_string()));
    };

    let Ok(()) = stage.fresh();

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

    fn number(check: &Check) -> &str {
        let Ok(number) = check.number();

        number
    }

    fn rest(check: &Check) -> &str {
        let Ok(rest) = check.rest();

        rest
    }

    fn named_by(check: &Check, words: &[String]) -> Named {
        let Ok(named) = check.named_by(words);

        named
    }

    fn without_the_device(check: &Check) -> Option<Stage> {
        let Ok(stage) = check.without_the_device();

        stage
    }

    fn desktop(check: &Check, stage: &mut Desktop) -> How {
        let Ok(how) = super::desktop(check, stage);

        how
    }

    fn device(check: &Check, stage: &mut Device) -> How {
        let Ok(how) = super::device(check, stage);

        how
    }

    fn name(how: &How) -> &str {
        let Ok(name) = how.name();

        name
    }

    fn new() -> Desktop {
        let Ok(desktop) = Desktop::new();

        desktop
    }

    #[test]
    fn a_check_says_when_it_arrived_and_what_it_is_about() {
        assert_eq!(number(&ONE), "010");
        assert_eq!(rest(&ONE), "workspaces-right");
    }

    #[test]
    fn a_check_is_found_by_its_name_or_by_its_feature() {
        assert_eq!(named_by(&ONE, &["workspaces".to_string()]), Named::Yes);
        assert_eq!(named_by(&ONE, &["010".to_string()]), Named::Yes);
        assert_eq!(named_by(&ONE, &["keyboard".to_string()]), Named::No);
    }

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

        assert_eq!(without_the_device(&THERE), None);

        assert_eq!(without_the_device(&BOTH), Some(Stage::Here));

        assert_eq!(without_the_device(&DRAWN), Some(Stage::Desktop));
    }

    #[test]
    fn a_stage_nothing_is_written_for_says_so() {
        let mut nowhere = new();
        assert_eq!(name(&desktop(&ONE, &mut nowhere)), "skipped");
    }

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

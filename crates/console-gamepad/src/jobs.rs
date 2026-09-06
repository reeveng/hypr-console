//! What a job is bound to: a button, and whatever has to be held with it.
//!
//! This is the shape of an answer, not the answers themselves. What the jobs
//! are and where they sit by default is `console_controller::means`, because
//! that is the thing that carries them out. What is here is the vocabulary the
//! two ends share: a binding is a button and a layer, it is written as
//! `l2 + right-paddle-bottom`, and it is read back the same way.
//!
//! Held in `console_gamepad` because both ends need it and neither owns it. The
//! daemon matches presses against these; the setup screen writes them; the
//! guide reads them out loud. A copy of this in any of the three would be the
//! copy that drifts.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use serde::Deserialize;

use console_never::Never;

use crate::vocabulary::{self, Names, TRIGGERS};

pub const UNDER: &str = ".config/console/buttons.toml";

pub fn path_in(home: &str) -> Result<PathBuf, Never> {
    Ok(PathBuf::from(home).join(UNDER))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Layer {
    pub l2: bool,
    pub r2: bool,
}

pub const ALONE: Layer = Layer { l2: false, r2: false };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Played {
    ByAButton,
    ByNothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rebound {
    Something,
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    Down,
    Up,
}

impl Layer {
    pub const fn of(l2: Held, r2: Held) -> Result<Self, Never> {
        Ok(Layer { l2: matches!(l2, Held::Down), r2: matches!(r2, Held::Down) })
    }

    pub fn held(self) -> Result<Held, Never> {
        Ok(match self.l2 || self.r2 {
            true => Held::Down,
            false => Held::Up,
        })
    }

    pub fn said(self) -> Result<Vec<&'static str>, Never> {
        let mut said = Vec::new();

        match self.l2 {
            true => said.push("l2"),
            false => {},
        }

        match self.r2 {
            true => said.push("r2"),
            false => {},
        }

        Ok(said)
    }

    pub fn is_a_trigger(word: &str) -> Result<Names, Never> {
        Ok(match TRIGGERS.iter().any(|(spoken, _)| *spoken == word) {
            true => Names::ATrigger,
            false => Names::AButton,
        })
    }

    fn of_word(word: &str) -> Result<Option<Self>, Never> {
        Ok(match word {
            "l2" => {
                let one = Layer::of(Held::Down, Held::Up)?;

                Some(one)
            }
            "r2" => {
                let one = Layer::of(Held::Up, Held::Down)?;

                Some(one)
            }
            _ => None,
        })
    }

    fn with(self, other: Self) -> Result<Self, Never> {
        Ok(Layer { l2: self.l2 || other.l2, r2: self.r2 || other.r2 })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Binding {
    pub layer: Layer,
    pub button: String,
}

pub const NOTHING: &str = "";

impl Binding {
    pub fn on(button: &str) -> Result<Self, Never> {
        Ok(Binding { layer: ALONE, button: button.to_string() })
    }

    pub const fn held(layer: Layer, button: String) -> Result<Self, Never> {
        Ok(Binding { layer, button })
    }

    pub fn nothing() -> Result<Self, Never> {
        Binding::on(NOTHING)
    }

    pub fn played(&self) -> Result<Played, Never> {
        Ok(match self.button.is_empty() {
            true => Played::ByNothing,
            false => Played::ByAButton,
        })
    }

    pub fn read(said: &str) -> Result<Self, String> {
        let said = said.trim();

        match said.is_empty() {
            true => {
                let Ok(nothing) = Binding::nothing();

                return Ok(nothing);
            }
            false => {},
        }

        let mut words: Vec<&str> = said.split('+').map(str::trim).collect();
        let button = words.pop().unwrap_or_default().to_string();
        let mut layer = ALONE;

        for word in words {
            let Ok(found) = Layer::of_word(word);

            let Some(one) = found else {
                return Err(format!("{word:?} is not a trigger to hold, in {said:?}"));
            };

            let Ok(joined) = layer.with(one);

            layer = joined;
        }

        let Ok(trigger) = Layer::is_a_trigger(&button);

        match trigger {
            Names::ATrigger => {
                return Err(format!(
                    "{button:?} is a trigger, and a trigger is what is held: {said:?}"
                ));
            }
            Names::AButton => {},
        }

        match vocabulary::button_name(&button) {
            Ok(_) => {},
            Err(_unnamed) => {
                return Err(format!("nothing on this machine is called {button:?}"));
            }
        }

        Ok(Binding { layer, button })
    }
}

impl fmt::Display for Binding {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Ok(mut said) = self.layer.said();

        match self.button.is_empty() {
            true => return write!(out, ""),
            false => {},
        }

        said.push(&self.button);
        write!(out, "{}", said.join(" + "))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Jobs {
    pub moved: BTreeMap<String, Vec<Binding>>,
}

#[derive(Deserialize)]
struct Written {
    #[serde(default)]
    jobs: BTreeMap<String, Said>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Said {
    One(String),
    Any(Vec<String>),
}

impl Said {
    fn every(self) -> Result<Vec<String>, Never> {
        Ok(match self {
            Said::One(said) => vec![said],
            Said::Any(said) => said,
        })
    }
}

impl Jobs {
    pub fn read(said: &str) -> Result<Self, String> {
        let written: Written = toml::from_str(said)
            .map_err(|fault| format!("the button table does not parse: {fault}"))?;
        let mut moved: BTreeMap<String, Vec<Binding>> = BTreeMap::new();

        for (job, said) in written.jobs {
            let mut bound = Vec::new();

            let Ok(every) = said.every();

            for one in every {
                let binding = Binding::read(&one).map_err(|fault| format!("{job}: {fault}"))?;

                bound.push(binding);
            }

            moved.insert(job, bound);
        }

        Ok(Jobs { moved })
    }

    pub fn none() -> Result<Self, Never> {
        Ok(Jobs::default())
    }

    pub fn moved(&self) -> Result<Rebound, Never> {
        Ok(match self.moved.is_empty() {
            true => Rebound::Nothing,
            false => Rebound::Something,
        })
    }

    pub fn bound(&self, job: &str) -> Result<Option<&[Binding]>, Never> {
        Ok(self.moved.get(job).map(Vec::as_slice))
    }

    pub fn moving(
        &mut self,
        every: &BTreeMap<String, Vec<Binding>>,
        job: &str,
        onto: &Binding,
    ) -> Result<Moved, Never> {
        let already = every.get(job).is_some_and(|bound| bound.contains(onto));
        let taken: Vec<String> = every
            .iter()
            .filter(|(named, _)| named.as_str() != job)
            .filter(|(_, bound)| bound.contains(onto))
            .map(|(named, _)| named.clone())
            .collect();

        for lost in &taken {
            let left: Vec<Binding> = every
                .get(lost)
                .map(|bound| bound.iter().filter(|one| *one != onto).cloned().collect())
                .unwrap_or_default();

            let standing = match left.is_empty() {
                true => {
                    let nothing = Binding::nothing()?;

                    vec![nothing]
                }
                false => left,
            };

            self.moved.insert(lost.clone(), standing);
        }

        self.moved.insert(job.to_string(), vec![onto.clone()]);

        Ok(match (taken.first(), already) {
            (Some(taken), _) => Moved::TookFrom(taken.clone()),
            (None, true) => Moved::Already,
            (None, false) => Moved::Onto,
        })
    }

    pub fn written(&self) -> Result<String, Never> {
        let mut said = String::from(
            "# What each thing this desktop does is bound to, on this machine.\n\
             #\n\
             # The left is the job. The right is the button that does it, and whatever\n\
             # has to be held down with it: `l2 + dpad-up` is the d-pad pressed up with\n\
             # the left trigger held. An empty answer is a job with no button at all.\n\
             #\n\
             # Only what somebody moved is here. Everything absent is where this desktop\n\
             # puts it, which is what `console-buttons` lists and what the setup screen\n\
             # shows. Written by the setup screen; the controller daemon reads it.\n\
             \n[jobs]\n",
        );

        for (job, bound) in &self.moved {
            let each: Vec<String> = bound.iter().map(Binding::to_string).collect();

            match each.as_slice() {
                [one] => said.push_str(&format!("{job} = \"{one}\"\n")),
                _ => said.push_str(&format!(
                    "{job} = [{}]\n",
                    each.iter().map(|one| format!("\"{one}\"")).collect::<Vec<_>>().join(", ")
                )),
            }
        }

        Ok(said)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Moved {
    Already,
    Onto,
    TookFrom(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    #[test]
    fn a_binding_is_read_the_way_it_is_written() {
        let held = Binding::read("l2 + right-paddle-bottom").expect("a binding");
        assert_eq!(held.layer, ok(Layer::of(Held::Down, Held::Up)));
        assert_eq!(held.button, "right-paddle-bottom");
        assert_eq!(held.to_string(), "l2 + right-paddle-bottom");
    }

    #[test]
    fn a_button_on_its_own_holds_nothing() {
        let alone = Binding::read("a").expect("a binding");
        assert_eq!(alone.layer, ALONE);
        assert_eq!(alone.layer.held(), Ok(Held::Up));
        assert_eq!(alone.to_string(), "a");
    }

    #[test]
    fn both_triggers_are_a_layer_of_their_own() {
        let both = Binding::read("l2 + r2 + x").expect("a binding");
        assert_eq!(both.layer, ok(Layer::of(Held::Down, Held::Down)));
        assert_eq!(both.to_string(), "l2 + r2 + x");
        assert_ne!(both.layer, ok(Layer::of(Held::Down, Held::Up)));
    }

    #[test]
    fn a_job_with_no_button_says_so_rather_than_being_left_out() {
        let none = Binding::read("").expect("a binding");
        assert_eq!(none.played(), Ok(Played::ByNothing));
        assert_eq!(none.to_string(), "");
    }

    #[test]
    fn a_chord_this_machine_cannot_read_is_a_fault_and_not_a_guess() {
        let two = Binding::read("x + a").expect_err("x is not a trigger");
        assert!(two.contains("not a trigger"), "{two}");
        let trigger = Binding::read("l2 + r2").expect_err("r2 is what is held");
        assert!(trigger.contains("is a trigger"), "{trigger}");
        let nothing = Binding::read("triangle").expect_err("no such button");
        assert!(nothing.contains("triangle"), "{nothing}");
    }

    #[test]
    fn the_file_holds_one_answer_or_several() {
        let jobs = Jobs::read(
            "[jobs]\nscreenshot = \"l2 + right-paddle-bottom\"\nkeyboard = [\"x\", \"keyboard\"]\n",
        )
        .expect("a table");
        assert_eq!(ok(jobs.bound("screenshot")).expect("one").len(), 1);
        assert_eq!(ok(jobs.bound("keyboard")).expect("two").len(), 2);
        assert_eq!(jobs.bound("menu"), Ok(None));
    }

    #[test]
    fn what_is_written_reads_back_the_same() {
        let said = "[jobs]\nkeyboard = [\"x\", \"keyboard\"]\nmenu = \"\"\nscreenshot = \"l2 + r2 + a\"\n";
        let jobs = Jobs::read(said).expect("a table");
        let again = Jobs::read(&ok(jobs.written())).expect("what it wrote");
        assert_eq!(jobs, again);
    }

    #[test]
    fn a_binding_that_does_not_read_takes_the_file_with_it() {
        let fault = Jobs::read("[jobs]\nmenu = \"a\"\nscreenshot = \"nose + a\"\n")
            .expect_err("nose is not a trigger");
        assert!(fault.starts_with("screenshot: "), "{fault}");
    }

    fn every() -> BTreeMap<String, Vec<Binding>> {
        [
            ("menu".to_string(), vec![ok(Binding::on("left-paddle-top"))]),
            ("screenshot".to_string(), vec![ok(Binding::held(ok(Layer::of(Held::Down, Held::Up)), "b".into()))]),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn moving_a_job_onto_a_free_button_leaves_everything_else_alone() {
        let mut jobs = ok(Jobs::none());
        assert_eq!(jobs.moving(&every(), "menu", &ok(Binding::on("menu"))), Ok(Moved::Onto));
        assert_eq!(jobs.bound("menu"), Ok(Some([ok(Binding::on("menu"))].as_slice())));
        assert_eq!(jobs.bound("screenshot"), Ok(None));
    }

    #[test]
    fn moving_a_job_onto_a_taken_button_takes_the_button() {
        let mut jobs = ok(Jobs::none());
        let onto = ok(Binding::on("left-paddle-top"));
        assert_eq!(jobs.moving(&every(), "screenshot", &onto), Ok(Moved::TookFrom("menu".into())));
        assert_eq!(jobs.bound("screenshot"), Ok(Some([onto].as_slice())));
        assert_eq!(ok(jobs.bound("menu")).expect("the menu")[0].played(), Ok(Played::ByNothing));
    }

    #[test]
    fn a_chord_does_not_take_the_button_it_is_held_over() {
        let mut jobs = ok(Jobs::none());
        let mut every = every();
        every.insert("back".to_string(), vec![ok(Binding::on("b"))]);
        let onto = ok(Binding::held(ok(Layer::of(Held::Up, Held::Down)), "b".into()));
        assert_eq!(jobs.moving(&every, "menu", &onto), Ok(Moved::Onto));
        assert_eq!(jobs.bound("back"), Ok(None), "b on its own is still back");
    }

    #[test]
    fn pressing_the_button_a_job_is_already_on_is_not_a_move() {
        let mut jobs = ok(Jobs::none());
        assert_eq!(jobs.moving(&every(), "menu", &ok(Binding::on("left-paddle-top"))), Ok(Moved::Already));
    }
}

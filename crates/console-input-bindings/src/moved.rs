//! The file in somebody's home, holding only what they moved.
//!
//! A job that is not in here is a job where this desktop put it, so a machine
//! nobody has touched has an empty file and the whole of its answer in
//! `console_input_controller::means`. It is not in the manifest and never
//! travels in this repository: it is the one file that is true of one person's
//! machine and wrong for every other.
//!
//! A job's answer is the whole of its answer, across every input. `menu =
//! ["keyboard: super + m"]` is the menu reached from a keyboard and from
//! nothing on the pad, and `menu = ""` is the menu with nothing on it
//! anywhere. That is why the list is not merged with the defaults a job at a
//! time: a person who took a job off the pad meant to.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use console_core_never::Never;

use crate::bound::Binding;

pub const NAMED: &str = "buttons.toml";

pub fn path_in(home: &Path) -> Result<PathBuf, Never> {
    let Ok(ours) = console_core_places::Base::Config.ours_under(home);

    Ok(ours.join(NAMED))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rebound {
    Something,
    Nothing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Moved {
    Already,
    Onto,
    TookFrom(String),
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

        let mut ours: Vec<Binding> = every
            .get(job)
            .map(|bound| {
                bound.iter().filter(|one| one.on != onto.on).cloned().collect()
            })
            .unwrap_or_default();

        ours.push(onto.clone());
        ours.sort();

        self.moved.insert(job.to_string(), ours);

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
             # The left is the job. The right is the input, what is held, and the one\n\
             # thing pressed: `pad: l2 + dpad-up` is the d-pad pressed up with the left\n\
             # trigger held, and `keyboard: super + i` is I with Super held. An input\n\
             # nobody names is the pad. An empty answer is a job with nothing on it at\n\
             # all, and a job listed here says the whole of where it is, on every input.\n\
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::bound::{Input, Played};

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
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
    fn a_job_can_be_on_one_input_and_another_at_once() {
        let jobs =
            Jobs::read("[jobs]\nsettings = [\"legion-right\", \"keyboard: super + i\"]\n")
                .expect("a table");
        let bound = ok(jobs.bound("settings")).expect("two");

        assert_eq!(bound.iter().map(|one| one.on).collect::<Vec<Input>>(), vec![
            Input::Pad,
            Input::Keyboard
        ]);
    }

    #[test]
    fn what_is_written_reads_back_the_same() {
        let said = "[jobs]\nkeyboard = [\"x\", \"keyboard\"]\nmenu = \"\"\nscreenshot = \"keyboard: ctrl + shift + p\"\n";
        let jobs = Jobs::read(said).expect("a table");
        let again = Jobs::read(&ok(jobs.written())).expect("what it wrote");

        assert_eq!(jobs, again);
    }

    #[test]
    fn a_file_written_before_there_was_more_than_a_pad_still_reads() {
        let jobs = Jobs::read("[jobs]\nscreenshot = \"l2 + right-paddle-bottom\"\n")
            .expect("a table");
        let bound = ok(jobs.bound("screenshot")).expect("one");

        assert_eq!(bound.first().map(|one| one.on), Some(Input::Pad));
    }

    #[test]
    fn a_binding_that_does_not_read_takes_the_file_with_it() {
        let fault = Jobs::read("[jobs]\nmenu = \"a\"\nscreenshot = \"nose + a\"\n")
            .expect_err("nose is nothing");

        assert!(fault.starts_with("screenshot: "), "{fault}");
    }

    fn every() -> BTreeMap<String, Vec<Binding>> {
        [
            ("menu".to_string(), vec![ok(Binding::pad("left-paddle-top"))]),
            (
                "screenshot".to_string(),
                vec![ok(Binding::holding(Input::Pad, &["l2"], "b"))],
            ),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn moving_a_job_onto_a_free_button_leaves_everything_else_alone() {
        let mut jobs = ok(Jobs::none());

        assert_eq!(jobs.moving(&every(), "menu", &ok(Binding::pad("menu"))), Ok(Moved::Onto));
        assert_eq!(jobs.bound("menu"), Ok(Some([ok(Binding::pad("menu"))].as_slice())));
        assert_eq!(jobs.bound("screenshot"), Ok(None));
    }

    #[test]
    fn moving_a_job_onto_a_taken_button_takes_the_button() {
        let mut jobs = ok(Jobs::none());
        let onto = ok(Binding::pad("left-paddle-top"));

        assert_eq!(jobs.moving(&every(), "screenshot", &onto), Ok(Moved::TookFrom("menu".into())));
        assert_eq!(jobs.bound("screenshot"), Ok(Some([onto].as_slice())));
        assert_eq!(
            ok(jobs.bound("menu")).and_then(|bound| bound.first()).map(|one| one.played()),
            Some(Ok(Played::ByNothing))
        );
    }

    #[test]
    fn a_chord_does_not_take_the_button_it_is_held_over() {
        let mut jobs = ok(Jobs::none());
        let mut every = every();

        every.insert("back".to_string(), vec![ok(Binding::pad("b"))]);

        let onto = ok(Binding::holding(Input::Pad, &["r2"], "b"));

        assert_eq!(jobs.moving(&every, "menu", &onto), Ok(Moved::Onto));
        assert_eq!(jobs.bound("back"), Ok(None), "b on its own is still back");
    }

    #[test]
    fn a_key_does_not_take_a_button_and_leaves_the_pad_where_it_was() {
        let mut jobs = ok(Jobs::none());
        let onto = ok(Binding::holding(Input::Keyboard, &["super"], "m"));

        assert_eq!(jobs.moving(&every(), "menu", &onto), Ok(Moved::Onto));

        let bound = ok(jobs.bound("menu")).expect("the menu");

        assert_eq!(bound.len(), 2, "the paddle it was on is still the paddle it is on");
        assert!(bound.contains(&ok(Binding::pad("left-paddle-top"))));
        assert!(bound.contains(&onto));
    }

    #[test]
    fn a_second_key_for_one_job_replaces_the_first() {
        let mut jobs = ok(Jobs::none());
        let first = ok(Binding::holding(Input::Keyboard, &["super"], "m"));

        let Ok(_) = jobs.moving(&every(), "menu", &first);

        let mut now = every();

        now.insert("menu".to_string(), ok(jobs.bound("menu")).unwrap_or_default().to_vec());

        let second = ok(Binding::holding(Input::Keyboard, &["ctrl"], "m"));
        let Ok(_) = jobs.moving(&now, "menu", &second);

        let bound = ok(jobs.bound("menu")).expect("the menu");

        assert!(bound.contains(&second));
        assert!(!bound.contains(&first), "one input, one place");
    }

    #[test]
    fn pressing_the_button_a_job_is_already_on_is_not_a_move() {
        let mut jobs = ok(Jobs::none());

        assert_eq!(
            jobs.moving(&every(), "menu", &ok(Binding::pad("left-paddle-top"))),
            Ok(Moved::Already)
        );
    }
}

//! The file in someone's home, holding only what they moved.
//!
//! A job that is not in here is a job where this desktop put it, so a machine
//! no one has touched has an empty file and the whole of its answer in
//! `console_input_controller::means`. It is not in the manifest and never
//! travels in this repository: it is the one file that is true of one person's
//! machine and wrong for every other.
//!
//! A job's answer is the whole of its answer, across every input. `menu =
//! ["keyboard: super + m"]` is the menu reached from a keyboard and from
//! nothing on the pad, and `menu = ""` is the menu with nothing on it
//! anywhere. That is why the list is not merged with the defaults a job at a
//! time: a person who took a job off the pad meant to.
//!
//! A job is on as many buttons and keys as someone gave it, so a press given to
//! it is added beside the ones it has rather than put in their place. It used
//! to replace whatever the job had on the same input, which made a second
//! button impossible to give and made the setup screen a place where every
//! answer undid the last. Taking one away is its own act, and a button is still
//! only ever one job's: given to a second, it leaves the first.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use console_core_never::Never;

use crate::Unbound;
use crate::bound::{Binding, Played};

pub const NAMED: &str = "buttons.toml";

pub fn path_in(home: &Path) -> Result<PathBuf, Never> {
    let Ok(ours) = console_core_places::Base::Configuration.application_under(home);

    Ok(ours.join(NAMED))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rebound {
    Some,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Moved {
    Already,
    Onto,
    TookFrom(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tasks {
    pub moved: BTreeMap<String, Vec<Binding>>,
}

#[derive(Deserialize)]
struct Written {
    #[serde(default)]
    jobs: BTreeMap<String, OneOrMany>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Any(Vec<String>),
}

impl OneOrMany {
    fn every(self) -> Result<Vec<String>, Never> {
        Ok(match self {
            OneOrMany::One(said) => vec![said],
            OneOrMany::Any(said) => said,
        })
    }
}

impl Tasks {
    pub fn read(said: &str) -> Result<Self, Unbound> {
        let written: Written = toml::from_str(said).map_err(Unbound::Untabled)?;
        let mut moved: BTreeMap<String, Vec<Binding>> = BTreeMap::new();

        for (job, said) in written.jobs {
            let mut bound = Vec::new();

            let Ok(every) = said.every();

            for one in every {
                let binding = Binding::read(&one)
                    .map_err(|fault| Unbound::UnderAJob(job.clone(), Box::new(fault)))?;

                bound.push(binding);
            }

            moved.insert(job, bound);
        }

        Ok(Tasks { moved })
    }

    pub fn none() -> Result<Self, Never> {
        Ok(Tasks::default())
    }

    pub fn rebound(&self) -> Result<Rebound, Never> {
        Ok(match self.moved.is_empty() {
            true => Rebound::None,
            false => Rebound::Some,
        })
    }

    pub fn bound(&self, job: &str) -> Result<Option<&[Binding]>, Never> {
        Ok(self.moved.get(job).map(Vec::as_slice))
    }

    pub fn add(
        &mut self,
        every: &BTreeMap<String, Vec<Binding>>,
        job: &str,
        onto: &Binding,
    ) -> Result<Moved, Never> {
        let already = every.get(job).is_some_and(|bound| bound.contains(onto));
        #[cfg_attr(
            dylint_lib = "explicit028_no_search_in_a_loop",
            allow(
                explicit028_no_search_in_a_loop,
                reason = "every job that could hold a binding, against the bindings one job holds: a screen of them at the very most"
            )
        )]
        let taken: Vec<String> = every
            .iter()
            .filter(|(named, _)| named.as_str() != job)
            .filter(|(_, bound)| bound.contains(onto))
            .map(|(named, _)| named.clone())
            .collect();

        for lost in &taken {
            let Ok(standing) = without(every.get(lost), onto);

            self.moved.insert(lost.clone(), standing);
        }

        let mut ours: Vec<Binding> = match every.get(job) {
            Some(bound) => bound
                .iter()
                .filter(|one| *one != onto && one.played() == Ok(Played::ByAPress))
                .cloned()
                .collect(),
            None => Vec::new(),
        };

        ours.push(onto.clone());
        ours.sort();

        self.moved.insert(job.to_string(), ours);

        Ok(match (taken.first(), already) {
            (Some(taken), _) => Moved::TookFrom(taken.clone()),
            (None, true) => Moved::Already,
            (None, false) => Moved::Onto,
        })
    }

    pub fn remove(
        &mut self,
        every: &BTreeMap<String, Vec<Binding>>,
        job: &str,
        off: &Binding,
    ) -> Result<(), Never> {
        let Ok(standing) = without(every.get(job), off);

        self.moved.insert(job.to_string(), standing);

        Ok(())
    }

    pub fn serialize(&self) -> Result<String, Never> {
        let mut said = String::from(
            "# What each thing this desktop does is bound to, on this machine.\n\
             #\n\
             # The left is the job. The right is the input, what is held, and the one\n\
             # thing pressed: `pad: l2 + dpad-up` is the d-pad pressed up with the left\n\
             # trigger held, and `keyboard: super + i` is I with Super held. An input\n\
             # no one names is the pad. An empty answer is a job with nothing on it at\n\
             # all, and a job listed here says the whole of where it is, on every input.\n\
             #\n\
             # Only what someone moved is here. Everything absent is where this desktop\n\
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

fn without(bound: Option<&Vec<Binding>>, off: &Binding) -> Result<Vec<Binding>, Never> {
    let left: Vec<Binding> = match bound {
        Some(bound) => bound.iter().filter(|one| *one != off).cloned().collect(),
        None => Vec::new(),
    };

    match left.is_empty() {
        true => {
            let nothing = Binding::unbound()?;

            Ok(vec![nothing])
        }
        false => Ok(left),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    use crate::bound::{Input, Played};

    #[test]
    fn the_file_holds_one_answer_or_several() -> Result<(), Box<dyn Error>> {
        let jobs = Tasks::read(
            "[jobs]\nscreenshot = \"l2 + right-paddle-bottom\"\nkeyboard = [\"x\", \"keyboard\"]\n",
        )?;
        let Ok(screenshot) = jobs.bound("screenshot");
        let Ok(keyboard) = jobs.bound("keyboard");

        assert_eq!(screenshot.map(<[Binding]>::len), Some(1));
        assert_eq!(keyboard.map(<[Binding]>::len), Some(2));
        assert_eq!(jobs.bound("menu"), Ok(None));
        Ok(())
    }

    #[test]
    fn a_job_can_be_on_one_input_and_another_at_once() -> Result<(), Box<dyn Error>> {
        let jobs = Tasks::read("[jobs]\nsettings = [\"legion-right\", \"keyboard: super + i\"]\n")?;
        let Ok(bound) = jobs.bound("settings");
        let bound = bound.ok_or("settings is bound to nothing")?;

        assert_eq!(bound.iter().map(|one| one.on).collect::<Vec<Input>>(), vec![
            Input::Pad,
            Input::Keyboard
        ]);
        Ok(())
    }

    #[test]
    fn what_is_written_reads_back_the_same() -> Result<(), Box<dyn Error>> {
        let said = "[jobs]\nkeyboard = [\"x\", \"keyboard\"]\nmenu = \"\"\nscreenshot = \"keyboard: ctrl + shift + p\"\n";
        let jobs = Tasks::read(said)?;
        let Ok(written) = jobs.serialize();
        let again = Tasks::read(&written)?;

        assert_eq!(jobs, again);
        Ok(())
    }

    #[test]
    fn a_file_written_before_there_was_more_than_a_pad_still_reads() -> Result<(), Box<dyn Error>> {
        let jobs = Tasks::read("[jobs]\nscreenshot = \"l2 + right-paddle-bottom\"\n")?;
        let Ok(bound) = jobs.bound("screenshot");
        let bound = bound.ok_or("the screenshot is bound to nothing")?;

        assert_eq!(bound.first().map(|one| one.on), Some(Input::Pad));
        Ok(())
    }

    #[test]
    fn a_binding_that_does_not_read_takes_the_file_with_it() {
        let fault = Tasks::read("[jobs]\nmenu = \"a\"\nscreenshot = \"nose + a\"\n")
            .expect_err("nose is nothing");

        assert!(
            matches!(fault, Unbound::UnderAJob(ref job, _) if job == "screenshot"),
            "{fault}"
        );
    }

    fn every() -> Result<BTreeMap<String, Vec<Binding>>, Never> {
        let Ok(top_paddle) = Binding::pad("left-paddle-top");
        let Ok(held_b) = Binding::chord(Input::Pad, &["l2"], "b");

        Ok([("menu".to_string(), vec![top_paddle]), ("screenshot".to_string(), vec![held_b])]
            .into_iter()
            .collect())
    }

    #[test]
    fn a_free_button_is_added_beside_the_one_a_job_already_has() {
        let Ok(mut jobs) = Tasks::none();
        let Ok(every) = every();
        let Ok(top_paddle) = Binding::pad("left-paddle-top");
        let Ok(menu) = Binding::pad("menu");

        assert_eq!(jobs.add(&every, "menu", &menu), Ok(Moved::Onto));
        assert_eq!(jobs.bound("menu"), Ok(Some([top_paddle, menu].as_slice())));
        assert_eq!(jobs.bound("screenshot"), Ok(None));
    }

    #[test]
    fn adding_a_button_another_job_is_on_takes_the_button() {
        let Ok(mut jobs) = Tasks::none();
        let Ok(every) = every();
        let Ok(onto) = Binding::pad("left-paddle-top");

        assert_eq!(jobs.add(&every, "screenshot", &onto), Ok(Moved::TookFrom("menu".to_string())));

        let Ok(screenshot) = jobs.bound("screenshot");
        let Ok(menu) = jobs.bound("menu");

        assert!(screenshot.is_some_and(|bound| bound.contains(&onto)));
        assert_eq!(
            menu.and_then(|bound| bound.first()).map(|one| one.played()),
            Some(Ok(Played::ByNothing))
        );
    }

    #[test]
    fn a_chord_does_not_take_the_button_it_is_held_over() {
        let Ok(mut jobs) = Tasks::none();
        let Ok(mut every) = every();
        let Ok(b) = Binding::pad("b");

        every.insert("back".to_string(), vec![b]);

        let Ok(onto) = Binding::chord(Input::Pad, &["r2"], "b");

        assert_eq!(jobs.add(&every, "menu", &onto), Ok(Moved::Onto));
        assert_eq!(jobs.bound("back"), Ok(None), "b on its own is still back");
    }

    #[test]
    fn a_key_does_not_take_a_button_and_leaves_the_pad_where_it_was() -> Result<(), Box<dyn Error>> {
        let Ok(mut jobs) = Tasks::none();
        let Ok(every) = every();
        let Ok(onto) = Binding::chord(Input::Keyboard, &["super"], "m");
        let Ok(top_paddle) = Binding::pad("left-paddle-top");

        assert_eq!(jobs.add(&every, "menu", &onto), Ok(Moved::Onto));

        let Ok(bound) = jobs.bound("menu");
        let bound = bound.ok_or("the menu is bound to nothing")?;

        assert_eq!(bound.len(), 2, "the paddle it was on is still the paddle it is on");
        assert!(bound.contains(&top_paddle));
        assert!(bound.contains(&onto));
        Ok(())
    }

    #[test]
    fn a_second_key_for_one_job_is_kept_beside_the_first() -> Result<(), Box<dyn Error>> {
        let Ok(mut jobs) = Tasks::none();
        let Ok(every) = every();
        let Ok(first) = Binding::chord(Input::Keyboard, &["super"], "m");
        let Ok(_) = jobs.add(&every, "menu", &first);
        let Ok(mut now) = self::every();
        let Ok(menu) = jobs.bound("menu");
        let menu = menu.ok_or("the menu is bound to nothing")?;

        now.insert("menu".to_string(), menu.to_vec());

        let Ok(second) = Binding::chord(Input::Keyboard, &["ctrl"], "m");
        let Ok(_) = jobs.add(&now, "menu", &second);
        let Ok(bound) = jobs.bound("menu");
        let bound = bound.ok_or("the menu is bound to nothing")?;

        assert!(bound.contains(&second));
        assert!(bound.contains(&first), "one job, as many places as someone gave it");
        Ok(())
    }

    #[test]
    fn a_button_added_to_a_job_with_nothing_on_it_is_the_whole_of_it() {
        let Ok(mut jobs) = Tasks::none();
        let Ok(mut every) = every();
        let Ok(nothing) = Binding::unbound();
        let Ok(y) = Binding::pad("y");

        every.insert("menu".to_string(), vec![nothing]);

        let Ok(_) = jobs.add(&every, "menu", &y);

        assert_eq!(jobs.bound("menu"), Ok(Some([y].as_slice())));
    }

    #[test]
    fn removing_one_place_leaves_the_others() {
        let Ok(mut jobs) = Tasks::none();
        let Ok(key) = Binding::chord(Input::Keyboard, &["super"], "m");
        let Ok(top_paddle) = Binding::pad("left-paddle-top");
        let Ok(mut every) = every();

        every.insert("menu".to_string(), vec![top_paddle.clone(), key.clone()]);

        let Ok(()) = jobs.remove(&every, "menu", &top_paddle);

        assert_eq!(jobs.bound("menu"), Ok(Some([key].as_slice())));
    }

    #[test]
    fn removing_the_last_place_leaves_a_job_with_nothing_on_it_rather_than_its_default() {
        let Ok(mut jobs) = Tasks::none();
        let Ok(every) = every();
        let Ok(top_paddle) = Binding::pad("left-paddle-top");
        let Ok(()) = jobs.remove(&every, "menu", &top_paddle);
        let Ok(menu) = jobs.bound("menu");

        assert_eq!(
            menu.and_then(|bound| bound.first()).map(|one| one.played()),
            Some(Ok(Played::ByNothing))
        );
    }

    #[test]
    fn pressing_the_button_a_job_is_already_on_is_not_a_move() {
        let Ok(mut jobs) = Tasks::none();
        let Ok(every) = every();
        let Ok(top_paddle) = Binding::pad("left-paddle-top");

        assert_eq!(jobs.add(&every, "menu", &top_paddle), Ok(Moved::Already));
    }
}

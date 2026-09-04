//! What a job is bound to: a button, and whatever has to be held with it.
//!
//! This is the shape of an answer, not the answers themselves. What the jobs
//! are and where they sit by default is `console_controller::means`, because
//! that is the thing that carries them out. What is here is the vocabulary the
//! two ends share: a binding is a button and a layer, it is written as
//! `l2 + right-paddle-bottom`, and it is read back the same way.
//!
//! Held in `console_pad` because both ends need it and neither owns it. The
//! daemon matches presses against these; the setup screen writes them; the
//! guide reads them out loud. A copy of this in any of the three would be the
//! copy that drifts.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use serde::Deserialize;

use crate::vocabulary::{self, Names, TRIGGERS};

/// Where the table lives, under the home of whoever this desktop belongs to.
///
/// Their home rather than `/etc`, and not in the manifest either. The manifest
/// is what every machine running this desktop has in common; this is one
/// person's answers about one machine, so it is kept where a person's own
/// answers are kept and never travels in the repository.
pub const UNDER: &str = ".config/console/buttons.toml";

pub fn path_in(home: &str) -> PathBuf {
    PathBuf::from(home).join(UNDER)
}

/// What is being held while a button is pressed.
///
/// Two triggers, so four answers. Both at once is a layer of its own rather
/// than either one of them: a machine that read L2 + R2 + X as the L2 binding
/// would be a machine where a thumb resting on the other trigger changed what
/// a press meant, which is the fault the paddle behind L2 taught.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Layer {
    pub l2: bool,
    pub r2: bool,
}

/// Nothing held, which is what nearly every binding is.
pub const ALONE: Layer = Layer { l2: false, r2: false };

/// Whether a binding has a button on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Played {
    /// Something plays it.
    ByAButton,
    /// Nothing does, which is a job written down and left unbound.
    ByNothing,
}

/// Whether anything in a table was moved off where this desktop puts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rebound {
    /// Somebody's own file says where at least one job goes.
    Something,
    /// Nothing was said, so every job is where the desktop puts it.
    Nothing,
}

/// Whether a trigger is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// It is held, so the bindings on its layer are the ones that play.
    Down,
    /// It is not.
    Up,
}

impl Layer {
    pub const fn of(l2: Held, r2: Held) -> Self {
        Layer { l2: matches!(l2, Held::Down), r2: matches!(r2, Held::Down) }
    }

    /// Whether anything is held at all.
    pub fn held(self) -> Held {
        match self.l2 || self.r2 {
            true => Held::Down,
            false => Held::Up,
        }
    }

    /// The triggers, in the order they are said and drawn.
    pub fn said(self) -> Vec<&'static str> {
        let mut said = Vec::new();

        if self.l2 {
            said.push("l2");
        }

        if self.r2 {
            said.push("r2");
        }

        said
    }

    /// Whether a word is the name of a trigger rather than of a button.
    ///
    /// The trigger names are `vocabulary::TRIGGERS`, so a device whose layers
    /// are called something else is one word away rather than a rewrite.
    pub fn is_a_trigger(word: &str) -> Names {
        match TRIGGERS.iter().any(|(spoken, _)| *spoken == word) {
            true => Names::ATrigger,
            false => Names::AButton,
        }
    }

    /// The layer a word names, if it names one.
    fn of_word(word: &str) -> Option<Self> {
        match word {
            "l2" => Some(Layer::of(Held::Down, Held::Up)),
            "r2" => Some(Layer::of(Held::Up, Held::Down)),
            _ => None,
        }
    }

    /// Both of these held at once.
    fn with(self, other: Self) -> Self {
        Layer { l2: self.l2 || other.l2, r2: self.r2 || other.r2 }
    }
}

/// A button, and what is held with it.
///
/// The button is said the way it is written on the machine -- `a`, `dpad-up`,
/// `right-paddle-bottom` -- and not in InputPlumber's words. Somebody opening
/// this file is reading about their own hands.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Binding {
    pub layer: Layer,
    /// The button, in the words on the machine. Empty where a job has been
    /// left with no button at all.
    pub button: String,
}

/// What a job with no button at all is written as.
///
/// Said rather than left out, for the reason the old table said it: a job
/// somebody took the button off is a thing done on purpose and has to be
/// undoable, and a row that is not in the file is a row that never moved.
pub const NOTHING: &str = "";

impl Binding {
    pub fn on(button: &str) -> Self {
        Binding { layer: ALONE, button: button.to_string() }
    }

    pub const fn held(layer: Layer, button: String) -> Self {
        Binding { layer, button }
    }

    /// Nothing plays this.
    pub fn nothing() -> Self {
        Binding::on(NOTHING)
    }

    pub fn played(&self) -> Played {
        match self.button.is_empty() {
            true => Played::ByNothing,
            false => Played::ByAButton,
        }
    }

    /// What was said, read back.
    ///
    /// Everything before the last word is a trigger to hold; the last word is
    /// the button. A word that is not a trigger where a trigger belongs is a
    /// fault rather than a button: `x + a` is somebody asking for a chord this
    /// machine cannot read, and answering it with `a` would be answering a
    /// question they did not ask.
    pub fn read(said: &str) -> Result<Self, String> {
        let said = said.trim();

        if said.is_empty() {
            return Ok(Binding::nothing());
        }

        let mut words: Vec<&str> = said.split('+').map(str::trim).collect();
        let button = words.pop().unwrap_or_default().to_string();
        let mut layer = ALONE;

        for word in words {
            let Some(one) = Layer::of_word(word) else {
                return Err(format!("{word:?} is not a trigger to hold, in {said:?}"));
            };

            layer = layer.with(one);
        }

        if Layer::is_a_trigger(&button) == Names::ATrigger {
            return Err(format!("{button:?} is a trigger, and a trigger is what is held: {said:?}"));
        }

        if vocabulary::button_name(&button).is_err() {
            return Err(format!("nothing on this machine is called {button:?}"));
        }

        Ok(Binding { layer, button })
    }
}

impl fmt::Display for Binding {
    /// The way it is written in the file, and the way it is said out loud.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut said = self.layer.said();

        if self.button.is_empty() {
            return write!(out, "");
        }

        said.push(&self.button);
        write!(out, "{}", said.join(" + "))
    }
}

/// Every job somebody has moved, and where they moved it to.
///
/// Only what differs, which is the rule the button table had and the reason a
/// machine nobody has touched has an empty file. What a job is bound to when
/// it is not in here is the default, which lives beside the job itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Jobs {
    pub moved: BTreeMap<String, Vec<Binding>>,
}

/// As it is written.
#[derive(Deserialize)]
struct Written {
    #[serde(default)]
    jobs: BTreeMap<String, Said>,
}

/// One job's answer: a binding, or several where two buttons do one job.
#[derive(Deserialize)]
#[serde(untagged)]
enum Said {
    One(String),
    Any(Vec<String>),
}

impl Said {
    fn every(self) -> Vec<String> {
        match self {
            Said::One(said) => vec![said],
            Said::Any(said) => said,
        }
    }
}

impl Jobs {
    /// The table, out of what was in the file.
    ///
    /// A binding that does not parse takes the file down rather than being
    /// quietly dropped. A file half read is a machine where some of what
    /// somebody asked for happened, which is worse to work out than a file
    /// that would not read at all.
    pub fn read(said: &str) -> Result<Self, String> {
        let written: Written = toml::from_str(said)
            .map_err(|fault| format!("the button table does not parse: {fault}"))?;
        let mut moved: BTreeMap<String, Vec<Binding>> = BTreeMap::new();

        for (job, said) in written.jobs {
            let mut bound = Vec::new();

            for one in said.every() {
                bound.push(Binding::read(&one).map_err(|fault| format!("{job}: {fault}"))?);
            }

            moved.insert(job, bound);
        }

        Ok(Jobs { moved })
    }

    pub fn none() -> Self {
        Jobs::default()
    }

    pub fn moved(&self) -> Rebound {
        match self.moved.is_empty() {
            true => Rebound::Nothing,
            false => Rebound::Something,
        }
    }

    /// What plays this job, where somebody has said something about it.
    pub fn bound(&self, job: &str) -> Option<&[Binding]> {
        self.moved.get(job).map(Vec::as_slice)
    }

    /// Put a job on a button, taking that button off whatever had it.
    ///
    /// One press still does one thing. The button goes to the job being moved
    /// and whatever held it before is left playing nothing, which is the rule
    /// the button table settled on and for the same reason: on a machine where
    /// every button worth pressing already does something, refusing the move
    /// is a screen that cannot be used.
    ///
    /// `every` is what each job is bound to now, defaults and all, because a
    /// button can be taken from a job that has never been moved and so has no
    /// row here to find it by.
    pub fn moving(
        &mut self,
        every: &BTreeMap<String, Vec<Binding>>,
        job: &str,
        onto: &Binding,
    ) -> Moved {
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
            self.moved.insert(
                lost.clone(),
                match left.is_empty() {
                    true => vec![Binding::nothing()],
                    false => left,
                },
            );
        }

        self.moved.insert(job.to_string(), vec![onto.clone()]);

        match (taken.first(), already) {
            (Some(taken), _) => Moved::TookFrom(taken.clone()),
            (None, true) => Moved::Already,
            (None, false) => Moved::Onto,
        }
    }

    /// The table, as the file that holds it.
    ///
    /// Written out rather than serialised, so that whoever opens it finds out
    /// what it is for. The setup screen writes this; the ordinary way of
    /// things is that nobody opens it at all, and the ordinary way of things
    /// is not when somebody does.
    pub fn written(&self) -> String {
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

            match each.len() {
                1 => said.push_str(&format!("{job} = \"{}\"\n", each[0])),
                _ => said.push_str(&format!(
                    "{job} = [{}]\n",
                    each.iter().map(|one| format!("\"{one}\"")).collect::<Vec<_>>().join(", ")
                )),
            }
        }

        said
    }
}

/// What moving a job onto a button came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Moved {
    /// It was already there, and nothing was written. The commonest thing
    /// anybody does at the card, because it is how you find out what a row is
    /// bound to.
    Already,
    /// It is there now, and the button was going spare.
    Onto,
    /// It is there now, and this is the job that had it, now bound to nothing.
    TookFrom(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_binding_is_read_the_way_it_is_written() {
        let held = Binding::read("l2 + right-paddle-bottom").expect("a binding");
        assert_eq!(held.layer, Layer::of(Held::Down, Held::Up));
        assert_eq!(held.button, "right-paddle-bottom");
        assert_eq!(held.to_string(), "l2 + right-paddle-bottom");
    }

    #[test]
    fn a_button_on_its_own_holds_nothing() {
        let alone = Binding::read("a").expect("a binding");
        assert_eq!(alone.layer, ALONE);
        assert_eq!(alone.layer.held(), Held::Up);
        assert_eq!(alone.to_string(), "a");
    }

    /// Both triggers is a layer of its own, and says so in both directions.
    #[test]
    fn both_triggers_are_a_layer_of_their_own() {
        let both = Binding::read("l2 + r2 + x").expect("a binding");
        assert_eq!(both.layer, Layer::of(Held::Down, Held::Down));
        assert_eq!(both.to_string(), "l2 + r2 + x");
        assert_ne!(both.layer, Layer::of(Held::Down, Held::Up));
    }

    /// A job with no button is written down as one, and reads back as one.
    #[test]
    fn a_job_with_no_button_says_so_rather_than_being_left_out() {
        let none = Binding::read("").expect("a binding");
        assert_eq!(none.played(), Played::ByNothing);
        assert_eq!(none.to_string(), "");
    }

    /// The two ways of asking for something this machine cannot do, both
    /// answered rather than guessed at.
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
        assert_eq!(jobs.bound("screenshot").expect("one").len(), 1);
        assert_eq!(jobs.bound("keyboard").expect("two").len(), 2);
        assert_eq!(jobs.bound("menu"), None);
    }

    /// What it writes is what it reads.
    #[test]
    fn what_is_written_reads_back_the_same() {
        let said = "[jobs]\nkeyboard = [\"x\", \"keyboard\"]\nmenu = \"\"\nscreenshot = \"l2 + r2 + a\"\n";
        let jobs = Jobs::read(said).expect("a table");
        let again = Jobs::read(&jobs.written()).expect("what it wrote");
        assert_eq!(jobs, again);
    }

    /// A file with one bad line does not come back as a file with the rest of
    /// its lines.
    #[test]
    fn a_binding_that_does_not_read_takes_the_file_with_it() {
        let fault = Jobs::read("[jobs]\nmenu = \"a\"\nscreenshot = \"nose + a\"\n")
            .expect_err("nose is not a trigger");
        assert!(fault.starts_with("screenshot: "), "{fault}");
    }

    fn every() -> BTreeMap<String, Vec<Binding>> {
        [
            ("menu".to_string(), vec![Binding::on("left-paddle-top")]),
            ("screenshot".to_string(), vec![Binding::held(Layer::of(Held::Down, Held::Up), "b".into())]),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn moving_a_job_onto_a_free_button_leaves_everything_else_alone() {
        let mut jobs = Jobs::none();
        assert_eq!(jobs.moving(&every(), "menu", &Binding::on("menu")), Moved::Onto);
        assert_eq!(jobs.bound("menu"), Some([Binding::on("menu")].as_slice()));
        assert_eq!(jobs.bound("screenshot"), None);
    }

    /// The button goes to the job being moved, and the job that had it says so
    /// on its own row.
    #[test]
    fn moving_a_job_onto_a_taken_button_takes_the_button() {
        let mut jobs = Jobs::none();
        let onto = Binding::on("left-paddle-top");
        assert_eq!(jobs.moving(&every(), "screenshot", &onto), Moved::TookFrom("menu".into()));
        assert_eq!(jobs.bound("screenshot"), Some([onto].as_slice()));
        assert_eq!(jobs.bound("menu").expect("the menu")[0].played(), Played::ByNothing);
    }

    /// A chord and the button on its own are two different bindings, so moving
    /// onto `l2 + b` does not take `b` from anything.
    #[test]
    fn a_chord_does_not_take_the_button_it_is_held_over() {
        let mut jobs = Jobs::none();
        let mut every = every();
        every.insert("back".to_string(), vec![Binding::on("b")]);
        let onto = Binding::held(Layer::of(Held::Up, Held::Down), "b".into());
        assert_eq!(jobs.moving(&every, "menu", &onto), Moved::Onto);
        assert_eq!(jobs.bound("back"), None, "b on its own is still back");
    }

    #[test]
    fn pressing_the_button_a_job_is_already_on_is_not_a_move() {
        let mut jobs = Jobs::none();
        assert_eq!(jobs.moving(&every(), "menu", &Binding::on("left-paddle-top")), Moved::Already);
    }
}

//! A binding: which input, what is held, and the one thing pressed.
//!
//! It used to be a trigger and a button, and the trigger was the only thing
//! that could be held because a trigger is the only thing on this machine with
//! nothing of its own on it. That shape could not say `left-paddle-top +
//! right-paddle-top`, and it could not say anything at all about a keyboard,
//! so the two hands this desktop can be driven with were described in two
//! places -- one of them a table three programs read, the other a list of
//! `hl.bind` lines nothing checked.
//!
//! What is held is a set now, and the input is part of the answer. Everything
//! else follows from that: `fits` is a subset question, so a chord beats the
//! bare press of the same button by holding more, and a job with nothing on
//! this input is a job the other input may still reach.
//!
//! ## The button on the way into a chord
//!
//! Pressing the top left paddle and then the top right one is two presses, and
//! the first of them is a press. If the first button has a job of its own it
//! does that job, and no reading of the second press can undo it. The way that
//! is not true of `l2 + x` is that a trigger has no job at all, which is what
//! made the layers work and is not a property of buttons in general.
//!
//! Nothing here refuses that, because a timer is the only thing that could and
//! a duration is an assumption this tree does not make (`console-waiting` is
//! the argument, EXPLICIT021 is the rule). What happens instead is that the
//! card says it: binding a chord over a button that plays something says which
//! job still happens on the way in, in the same breath as saying where the job
//! went.

use std::fmt;

use console_core_never::Never;
use console_input_gamepad::vocabulary::{self, Names};

use crate::keys::{self, Words};

pub const NOTHING: &str = "";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Input {
    Pad,
    Keyboard,
}

pub const EVERY: [Input; 2] = [Input::Pad, Input::Keyboard];

impl Input {
    pub fn word(self) -> Result<&'static str, Never> {
        Ok(match self {
            Input::Pad => "pad",
            Input::Keyboard => "keyboard",
        })
    }

    pub fn says(self) -> Result<&'static str, Never> {
        Ok(match self {
            Input::Pad => "Controller",
            Input::Keyboard => "Keyboard",
        })
    }

    fn of_word(word: &str) -> Result<Option<Self>, Never> {
        Ok(EVERY.into_iter().find(|input| {
            let Ok(said) = input.word();

            said == word
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Played {
    ByAPress,
    ByNothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fits {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Binding {
    pub on: Input,
    pub held: Vec<String>,
    pub pressed: String,
}

impl Binding {
    pub fn pad(button: &str) -> Result<Self, Never> {
        Ok(Binding { on: Input::Pad, held: Vec::new(), pressed: button.to_string() })
    }

    pub fn holding(on: Input, held: &[&str], pressed: &str) -> Result<Self, Never> {
        Ok(Binding {
            on,
            held: held.iter().map(|word| (*word).to_string()).collect(),
            pressed: pressed.to_string(),
        })
    }

    pub fn nothing() -> Result<Self, Never> {
        Binding::pad(NOTHING)
    }

    pub fn played(&self) -> Result<Played, Never> {
        Ok(match self.pressed.is_empty() {
            true => Played::ByNothing,
            false => Played::ByAPress,
        })
    }

    pub fn depth(&self) -> Result<usize, Never> {
        Ok(self.held.len())
    }

    pub fn fits(&self, on: Input, held: &[&str], pressed: &str) -> Result<Fits, Never> {
        let Ok(played) = self.played();

        let ours = played == Played::ByAPress
            && self.on == on
            && self.pressed == pressed
            && self.held.iter().all(|word| held.contains(&word.as_str()));

        Ok(match ours {
            true => Fits::Yes,
            false => Fits::No,
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

        let (on, gesture) = match said.split_once(':') {
            Some((word, rest)) => {
                let Ok(named) = Input::of_word(word.trim());

                let on = match named {
                    Some(on) => on,
                    None => {
                        return Err(format!("nothing here is called {:?}, in {said:?}", word.trim()));
                    }
                };

                (on, rest.trim())
            }
            None => (Input::Pad, said),
        };

        let mut words: Vec<&str> = gesture.split('+').map(str::trim).collect();
        let pressed = words.pop().unwrap_or_default().to_string();
        let held: Vec<String> = words.iter().map(|word| (*word).to_string()).collect();

        let binding = Binding { on, held, pressed };

        binding.makes_sense()?;

        Ok(binding)
    }

    fn makes_sense(&self) -> Result<(), String> {
        match self.on {
            Input::Pad => self.on_the_pad(),
            Input::Keyboard => self.on_the_keyboard(),
        }
    }

    fn on_the_pad(&self) -> Result<(), String> {
        let Ok(trigger) = vocabulary::is_trigger(&self.pressed);

        match trigger {
            Names::ATrigger => {
                return Err(format!(
                    "{:?} is a trigger, and a trigger is what is held: {self}",
                    self.pressed
                ));
            }
            Names::AButton => {},
        }

        vocabulary::button_name(&self.pressed)
            .map_err(|_unnamed| format!("nothing on this machine is called {:?}", self.pressed))?;

        for word in &self.held {
            let Ok(trigger) = vocabulary::is_trigger(word);

            match trigger {
                Names::ATrigger => continue,
                Names::AButton => {},
            }

            vocabulary::button_name(word)
                .map_err(|_unnamed| format!("nothing on this machine is called {word:?}"))?;
        }

        Ok(())
    }

    fn on_the_keyboard(&self) -> Result<(), String> {
        let Ok(modifier) = keys::is_a_modifier(&self.pressed);

        match modifier {
            Words::AModifier => {
                return Err(format!(
                    "{:?} is a modifier, and a modifier is what is held: {self}",
                    self.pressed
                ));
            }
            Words::AKey => {},
        }

        keys::key_named(&self.pressed)?;

        for word in &self.held {
            let Ok(modifier) = keys::is_a_modifier(word);

            match modifier {
                Words::AModifier => {},
                Words::AKey => {
                    return Err(format!(
                        "{word:?} is a key, and a key is what is pressed rather than held"
                    ));
                }
            }
        }

        Ok(())
    }
}

impl fmt::Display for Binding {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Ok(played) = self.played();

        match played {
            Played::ByNothing => return write!(out, "{NOTHING}"),
            Played::ByAPress => {},
        }

        let Ok(word) = self.on.word();
        let mut said: Vec<&str> = self.held.iter().map(String::as_str).collect();

        said.push(&self.pressed);

        write!(out, "{word}: {}", said.join(" + "))
    }
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

        assert_eq!(held.on, Input::Pad);
        assert_eq!(held.held, vec!["l2".to_string()]);
        assert_eq!(held.pressed, "right-paddle-bottom");
        assert_eq!(held.to_string(), "pad: l2 + right-paddle-bottom");
    }

    #[test]
    fn a_button_on_its_own_holds_nothing() {
        let alone = Binding::read("a").expect("a binding");

        assert_eq!(ok(alone.depth()), 0);
        assert_eq!(alone.to_string(), "pad: a");
    }

    #[test]
    fn a_paddle_can_be_held_the_way_a_trigger_can() {
        let chord = Binding::read("left-paddle-top + right-paddle-top").expect("a binding");

        assert_eq!(chord.held, vec!["left-paddle-top".to_string()]);
        assert_eq!(ok(chord.depth()), 1);
    }

    #[test]
    fn a_keyboard_binding_says_which_input_it_is_on() {
        let chord = Binding::read("keyboard: super + i").expect("a binding");

        assert_eq!(chord.on, Input::Keyboard);
        assert_eq!(chord.pressed, "i");
        assert_eq!(chord.to_string(), "keyboard: super + i");
    }

    #[test]
    fn a_job_with_nothing_on_it_says_so_rather_than_being_left_out() {
        let none = Binding::read("").expect("a binding");

        assert_eq!(none.played(), Ok(Played::ByNothing));
        assert_eq!(none.to_string(), NOTHING);
    }

    #[test]
    fn what_this_machine_cannot_press_is_a_fault_and_not_a_guess() {
        let trigger = Binding::read("l2 + r2").expect_err("r2 is what is held");
        assert!(trigger.contains("is a trigger"), "{trigger}");

        let nothing = Binding::read("triangle").expect_err("no such button");
        assert!(nothing.contains("triangle"), "{nothing}");

        let key = Binding::read("keyboard: i + super").expect_err("super is held");
        assert!(key.contains("is a modifier"), "{key}");

        let word = Binding::read("mouse: left").expect_err("no such input");
        assert!(word.contains("mouse"), "{word}");
    }

    #[test]
    fn a_pad_button_is_not_a_key_and_a_key_is_not_a_pad_button() {
        assert!(Binding::read("keyboard: left-paddle-top").is_err());
        assert!(Binding::read("pad: super + i").is_err());
    }

    #[test]
    fn a_chord_fits_only_while_what_it_holds_is_held() {
        let chord = Binding::read("l2 + dpad-up").expect("a binding");

        assert_eq!(chord.fits(Input::Pad, &["l2"], "dpad-up"), Ok(Fits::Yes));
        assert_eq!(chord.fits(Input::Pad, &[], "dpad-up"), Ok(Fits::No));
        assert_eq!(chord.fits(Input::Keyboard, &["l2"], "dpad-up"), Ok(Fits::No));
    }

    #[test]
    fn something_else_held_does_not_stop_a_press_that_asks_for_nothing() {
        let bare = Binding::read("left-paddle-top").expect("a binding");

        assert_eq!(bare.fits(Input::Pad, &["l2", "r2"], "left-paddle-top"), Ok(Fits::Yes));
    }

    #[test]
    fn a_job_on_no_button_fits_nothing() {
        let Ok(none) = Binding::nothing();

        assert_eq!(none.fits(Input::Pad, &[], ""), Ok(Fits::No));
    }
}

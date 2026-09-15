//! A scenario: what somebody did with their thumbs, written down.
//!
//! ```text
//! profile desktop
//! press left-paddle-top
//! wait 0.3
//! press dpad-down
//! press a
//! ```
//!
//! The same lines drive the emulator whether it is making real devices for the
//! desktop in front of you or a world inside a test, which is the point of
//! having them: what was tried by hand is what gets kept as a test.
//!
//! Reading a line and doing it are two things here. A scenario can be read for
//! what it says without a device in the room, which is how a test can hold one
//! to the buttons that exist.


use console_core_number_conversion::toward_zero_i32;
use crate::Unpressed;
use crate::devices::Sink;
use crate::go::{Clock, LegionGo, MIDDLE};
use console_core_geometry::Point;
use console_core_never::Never;

const DRAG_STEPS: i32 = 8;

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Profile(String),
    Press(Vec<String>),
    Hold(Vec<String>),
    Release(Vec<String>),
    Stick { which: String, to: Point<f64> },
    Centre(String),
    Trigger { which: String, amount: f64 },
    Tap { at: Point<i32> },
    Drag { from: Point<i32>, to: Point<i32>, seconds: f64 },
    Click(bool),
    Wait(f64),
}

fn number(word: &str) -> Result<f64, Unpressed> {
    word.parse::<f64>()
        .map_err(|_| Unpressed::NotANumber(word.to_string()))
}

fn whole(word: &str) -> Result<i32, Unpressed> {
    let said = number(word)?;
    let Ok(whole) = toward_zero_i32(said);

    Ok(whole)
}

fn word(rest: &[&str], at: usize, what: &'static str) -> Result<String, Unpressed> {
    rest.get(at)
        .map(|said| (*said).to_string())
        .ok_or(Unpressed::NothingSaid(what))
}

fn stick_named(said: &str) -> Result<String, Never> {
    Ok(match said.ends_with("-stick") {
        true => said.to_string(),
        false => format!("{said}-stick"),
    })
}

impl Step {
    pub fn read(line: &str) -> Result<Option<Step>, Unpressed> {
        let Ok(bare) = console_core_ini_files::without_a_comment(line);
        let words: Vec<&str> = bare.split_whitespace().collect();
        let (verb, rest) = match words.split_first() {
            None => return Ok(None),
            Some((verb, rest)) => (*verb, rest),
        };
        let named = || rest.iter().map(|said| (*said).to_string()).collect();
        let step = match verb {
            "profile" => {
                let name = word(rest, 0, "profile")?;

                Step::Profile(name)
            }
            "press" => Step::Press(named()),
            "hold" => Step::Hold(named()),
            "release" => Step::Release(named()),
            "stick" => {
                let which = word(rest, 0, "stick")?;
                let sideways = word(rest, 1, "sideways")?;
                let up_or_down = word(rest, 2, "up or down")?;
                let x = number(&sideways)?;
                let y = number(&up_or_down)?;

                let Ok(named) = stick_named(&which);

                Step::Stick { which: named, to: Point { across: x, down: y } }
            }
            "centre" => {
                let which = word(rest, 0, "stick")?;

                let Ok(named) = stick_named(&which);

                Step::Centre(named)
            }
            "trigger" => {
                let which = word(rest, 0, "trigger")?;
                let how_far = word(rest, 1, "how far")?;
                let amount = number(&how_far)?;

                Step::Trigger { which, amount }
            }
            "tap" => match rest.is_empty() {
                true => Step::Tap { at: Point { across: MIDDLE, down: MIDDLE } },
                false => {
                    let across = word(rest, 0, "x")?;
                    let down = word(rest, 1, "y")?;
                    let x = whole(&across)?;
                    let y = whole(&down)?;

                    Step::Tap { at: Point { across: x, down: y } }
                }
            },
            "drag" => {
                let from_across = word(rest, 0, "x")?;
                let from_down = word(rest, 1, "y")?;
                let to_across = word(rest, 2, "x")?;
                let to_down = word(rest, 3, "y")?;
                let from_x = whole(&from_across)?;
                let from_y = whole(&from_down)?;
                let to_x = whole(&to_across)?;
                let to_y = whole(&to_down)?;
                let seconds = match rest.get(4) {
                    Some(said) => number(said)?,
                    None => 0.0,
                };

                Step::Drag {
                    from: Point { across: from_x, down: from_y },
                    to: Point { across: to_x, down: to_y },
                    seconds,
                }
            }
            "click" => {
                let said = word(rest, 0, "down or up")?;

                Step::Click(matches!(said.as_str(), "down" | "1"))
            }
            "wait" => {
                let how_long = word(rest, 0, "how long")?;
                let seconds = number(&how_long)?;

                Step::Wait(seconds)
            }
            other => return Err(Unpressed::NoSuchStep(other.to_string())),
        };
        Ok(Some(step))
    }

    pub fn done<S: Sink, C: Clock>(&self, go: &mut LegionGo<S, C>) -> Result<(), Unpressed> {
        match self {
            Step::Profile(name) => go.load_profile(name),
            Step::Press(buttons) => buttons.iter().try_for_each(|button| go.press(button)),
            Step::Hold(buttons) => buttons.iter().try_for_each(|button| go.hold(button)),
            Step::Release(buttons) if buttons.is_empty() => go.release_all(),
            Step::Release(buttons) => buttons.iter().try_for_each(|button| go.release(button)),
            Step::Stick { which, to } => go.stick(which, *to),
            Step::Centre(which) => go.centre(which),
            Step::Trigger { which, amount } => go.trigger(which, *amount),
            Step::Tap { at } => {
                let Ok(()) = go.tap(*at);

                Ok(())
            }
            Step::Drag { from, to, seconds } => {
                let Ok(()) = go.drag(*from, *to, DRAG_STEPS, *seconds);

                Ok(())
            }
            Step::Click(down) => {
                let Ok(()) = go.touch_click(i32::from(*down));

                Ok(())
            }
            Step::Wait(seconds) => {
                let Ok(()) = go.wait(*seconds);

                Ok(())
            }
        }
    }
}

pub fn read(text: &str) -> Result<Vec<Step>, Unpressed> {
    text.lines()
        .enumerate()
        .map(|(number, line)| {
            Step::read(line)
                .map_err(|fault| Unpressed::AtLine(number.saturating_add(1), Box::new(fault)))
        })
        .collect::<Result<Vec<Option<Step>>, Unpressed>>()
        .map(|steps| steps.into_iter().flatten().collect())
}

pub fn play<S: Sink, C: Clock>(
    go: &mut LegionGo<S, C>,
    text: &str,
) -> Result<Vec<Step>, Unpressed> {
    let steps = read(text)?;
    steps.iter().try_for_each(|step| step.done(go))?;
    Ok(steps)
}

pub const VERBS: &str = "\
  profile <name>            which profile the presses go through
  press <button>...         press and let go
  hold <button>...          press and keep pressing
  release [<button>...]     let go, of everything if nothing is named
  stick left|right <x> <y>  push a stick, each axis from -1 to 1
  centre left|right         let it go back
  trigger l2|r2 <amount>    pull a trigger, from 0 to 1
  tap [<x> <y>]             a quick touch on the touchpad
  drag <x> <y> <x> <y> [s]  a finger from one place to another
  click down|up             press the touchpad in, and let it out
  wait <seconds>            do nothing for a moment";

#[cfg(test)]
mod tests {
    use super::*;

    fn step(line: &str) -> Option<Step> {
        Step::read(line).expect("a line this knows")
    }

    #[test]
    fn a_blank_line_and_a_comment_are_nothing() {
        assert_eq!(step(""), None);
        assert_eq!(step("   "), None);
        assert_eq!(step("# what somebody did"), None);
    }

    #[test]
    fn a_comment_after_a_step_is_still_a_comment() {
        assert_eq!(step("press a  # click"), Some(Step::Press(vec!["a".into()])));
    }

    #[test]
    fn a_stick_is_the_same_stick_by_either_name() {
        let both = [step("stick left 1 0"), step("stick left-stick 1 0")];
        assert_eq!(both[0], both[1]);
        assert_eq!(
            both[0],
            Some(Step::Stick {
                which: "left-stick".into(),
                to: Point { across: 1.0, down: 0.0 },
            })
        );
    }

    #[test]
    fn releasing_nothing_is_releasing_everything() {
        assert_eq!(step("release"), Some(Step::Release(vec![])));
    }

    #[test]
    fn a_tap_with_nowhere_named_lands_in_the_middle() {
        assert_eq!(
            step("tap"),
            Some(Step::Tap { at: Point { across: MIDDLE, down: MIDDLE } })
        );
    }

    #[test]
    fn a_drag_may_say_how_long_it_takes_or_not() {
        assert_eq!(
            step("drag 0 0 10 10"),
            Some(Step::Drag {
                from: Point { across: 0, down: 0 },
                to: Point { across: 10, down: 10 },
                seconds: 0.0,
            })
        );
        assert_eq!(
            step("drag 0 0 10 10 0.5"),
            Some(Step::Drag {
                from: Point { across: 0, down: 0 },
                to: Point { across: 10, down: 10 },
                seconds: 0.5,
            })
        );
    }

    #[test]
    fn a_line_that_is_not_a_step_says_which_line_it_was() {
        let fault = read("press a\nsqueeze b\n").expect_err("no such verb");

        match fault {
            Unpressed::AtLine(2, ref inner) => match **inner {
                Unpressed::NoSuchStep(ref said) => assert_eq!(said, "squeeze"),
                ref other => panic!("{other:?}"),
            },
            ref other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_step_missing_what_it_needs_says_what_is_missing() {
        assert!(matches!(
            Step::read("stick left 1"),
            Err(Unpressed::NothingSaid("up or down"))
        ));
        assert!(matches!(
            Step::read("wait soon"),
            Err(Unpressed::NotANumber(ref said)) if said == "soon"
        ));
    }
}

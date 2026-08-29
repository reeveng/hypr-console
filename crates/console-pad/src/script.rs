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

use crate::devices::Sink;
use crate::go::{Clock, LegionGo, MIDDLE};

/// How many reports a drag is made of.
const DRAG_STEPS: i32 = 8;

/// One line of a scenario, read.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Profile(String),
    Press(Vec<String>),
    Hold(Vec<String>),
    /// Nothing named means everything held.
    Release(Vec<String>),
    Stick { which: String, x: f64, y: f64 },
    Centre(String),
    Trigger { which: String, amount: f64 },
    Tap { x: i32, y: i32 },
    Drag { from: (i32, i32), to: (i32, i32), seconds: f64 },
    Click(bool),
    Wait(f64),
}

fn number(word: &str) -> Result<f64, String> {
    word.parse::<f64>().map_err(|_| format!("{word:?} is not a number"))
}

fn whole(word: &str) -> Result<i32, String> {
    number(word).map(|found| found as i32)
}

fn word(rest: &[&str], at: usize, what: &str) -> Result<String, String> {
    rest.get(at).map(|said| (*said).to_string()).ok_or_else(|| format!("no {what}"))
}

/// `left` and `left-stick` are the same stick.
fn stick_named(said: &str) -> String {
    match said.ends_with("-stick") {
        true => said.to_string(),
        false => format!("{said}-stick"),
    }
}

impl Step {
    /// One line, read. Nothing at all is a blank line or a comment.
    pub fn read(line: &str) -> Result<Option<Step>, String> {
        let bare = line.split('#').next().unwrap_or("");
        let words: Vec<&str> = bare.split_whitespace().collect();
        let (verb, rest) = match words.split_first() {
            None => return Ok(None),
            Some((verb, rest)) => (*verb, rest),
        };
        let named = || rest.iter().map(|said| (*said).to_string()).collect();
        let step = match verb {
            "profile" => Step::Profile(word(rest, 0, "profile")?),
            "press" => Step::Press(named()),
            "hold" => Step::Hold(named()),
            "release" => Step::Release(named()),
            "stick" => Step::Stick {
                which: stick_named(&word(rest, 0, "stick")?),
                x: number(&word(rest, 1, "sideways")?)?,
                y: number(&word(rest, 2, "up or down")?)?,
            },
            "centre" => Step::Centre(stick_named(&word(rest, 0, "stick")?)),
            "trigger" => Step::Trigger {
                which: word(rest, 0, "trigger")?,
                amount: number(&word(rest, 1, "how far")?)?,
            },
            "tap" => match rest.is_empty() {
                true => Step::Tap { x: MIDDLE, y: MIDDLE },
                false => Step::Tap { x: whole(&word(rest, 0, "x")?)?, y: whole(&word(rest, 1, "y")?)? },
            },
            "drag" => Step::Drag {
                from: (whole(&word(rest, 0, "x")?)?, whole(&word(rest, 1, "y")?)?),
                to: (whole(&word(rest, 2, "x")?)?, whole(&word(rest, 3, "y")?)?),
                seconds: match rest.get(4) {
                    Some(said) => number(said)?,
                    None => 0.0,
                },
            },
            "click" => Step::Click(matches!(word(rest, 0, "down or up")?.as_str(), "down" | "1")),
            "wait" => Step::Wait(number(&word(rest, 0, "how long")?)?),
            other => return Err(format!("no such thing as {other:?}")),
        };
        Ok(Some(step))
    }

    /// The step, done.
    pub fn done<S: Sink, C: Clock>(&self, go: &mut LegionGo<S, C>) -> Result<(), String> {
        match self {
            Step::Profile(name) => go.load_profile(name),
            Step::Press(buttons) => buttons.iter().try_for_each(|button| go.press(button)),
            Step::Hold(buttons) => buttons.iter().try_for_each(|button| go.hold(button)),
            Step::Release(buttons) if buttons.is_empty() => go.release_all(),
            Step::Release(buttons) => buttons.iter().try_for_each(|button| go.release(button)),
            Step::Stick { which, x, y } => go.stick(which, *x, *y),
            Step::Centre(which) => go.centre(which),
            Step::Trigger { which, amount } => go.trigger(which, *amount),
            Step::Tap { x, y } => {
                go.tap(*x, *y);
                Ok(())
            }
            Step::Drag { from, to, seconds } => {
                go.drag(*from, *to, DRAG_STEPS, *seconds);
                Ok(())
            }
            Step::Click(down) => {
                go.touch_click(i32::from(*down));
                Ok(())
            }
            Step::Wait(seconds) => {
                go.wait(*seconds);
                Ok(())
            }
        }
    }
}

/// Every line of a scenario, read. Says which line went wrong, if one did.
pub fn read(text: &str) -> Result<Vec<Step>, String> {
    text.lines()
        .enumerate()
        .map(|(number, line)| {
            Step::read(line).map_err(|fault| format!("line {}: {fault}", number + 1))
        })
        .collect::<Result<Vec<Option<Step>>, String>>()
        .map(|steps| steps.into_iter().flatten().collect())
}

/// Every line of a scenario, in order.
pub fn play<S: Sink, C: Clock>(go: &mut LegionGo<S, C>, text: &str) -> Result<Vec<Step>, String> {
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

    #[test]
    fn a_blank_line_and_a_comment_are_nothing() {
        assert_eq!(Step::read(""), Ok(None));
        assert_eq!(Step::read("   "), Ok(None));
        assert_eq!(Step::read("# what somebody did"), Ok(None));
    }

    #[test]
    fn a_comment_after_a_step_is_still_a_comment() {
        assert_eq!(Step::read("press a  # click"), Ok(Some(Step::Press(vec!["a".into()]))));
    }

    #[test]
    fn a_stick_is_the_same_stick_by_either_name() {
        let both = [Step::read("stick left 1 0"), Step::read("stick left-stick 1 0")];
        assert_eq!(both[0], both[1]);
        assert_eq!(both[0], Ok(Some(Step::Stick { which: "left-stick".into(), x: 1.0, y: 0.0 })));
    }

    #[test]
    fn releasing_nothing_is_releasing_everything() {
        assert_eq!(Step::read("release"), Ok(Some(Step::Release(vec![]))));
    }

    #[test]
    fn a_tap_with_nowhere_named_lands_in_the_middle() {
        assert_eq!(Step::read("tap"), Ok(Some(Step::Tap { x: MIDDLE, y: MIDDLE })));
    }

    #[test]
    fn a_drag_may_say_how_long_it_takes_or_not() {
        assert_eq!(
            Step::read("drag 0 0 10 10"),
            Ok(Some(Step::Drag { from: (0, 0), to: (10, 10), seconds: 0.0 }))
        );
        assert_eq!(
            Step::read("drag 0 0 10 10 0.5"),
            Ok(Some(Step::Drag { from: (0, 0), to: (10, 10), seconds: 0.5 }))
        );
    }

    #[test]
    fn a_line_that_is_not_a_step_says_which_line_it_was() {
        let fault = read("press a\nsqueeze b\n").expect_err("no such verb");
        assert!(fault.starts_with("line 2:"), "{fault}");
        assert!(fault.contains("squeeze"), "{fault}");
    }

    #[test]
    fn a_step_missing_what_it_needs_says_what_is_missing() {
        assert_eq!(Step::read("stick left 1"), Err("no up or down".to_string()));
        assert_eq!(Step::read("wait soon"), Err("\"soon\" is not a number".to_string()));
    }
}

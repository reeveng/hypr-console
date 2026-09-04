//! How far an apply has got, for the strip under the bar to fill.
//!
//!     bar-updating
//!
//! One line of JSON and then it exits. waybar runs it again when the engine
//! signals, so there is nothing running and nothing polling while no apply is
//! happening -- which is the desktop's ordinary state, and a handheld pays for
//! every wake-up.
//!
//! # Why it says nothing
//!
//! There is no text and no icon. The module is a bare strip a row tall
//! running the width of the screen, the colour of the bar until an apply
//! starts and then filled from the left as it goes. `console-updating` already
//! raises a card that says an apply is running; what the card cannot say, and
//! says the same sentence for a minute instead, is how much longer. A line
//! that fills answers that at a glance, from across a room, without covering
//! anything or asking to be read.
//!
//! Nothing is drawn at all the rest of the time. An empty `text` is a module
//! waybar hides, and what is behind it is the strip's own background, which is
//! the bar's. So the strip is invisible until there is something to say, and
//! goes back to being invisible after.
//!
//! # Why a class and not a number
//!
//! waybar has no progress widget. A custom module is text, a tooltip, a
//! percentage and a class, and the percentage only chooses a class -- nothing
//! in waybar or GTK will take a number and fill a box to it. What will fill a
//! box is a CSS gradient with a stop in it, and CSS is written down in
//! advance, so the number has to arrive as one of a fixed set of names.
//!
//! Hence `at-0` to `at-100` in fives: twenty-one rules in the stylesheet, one
//! per class this can send, and `every_step_the_bar_can_send_is_one_the_style_
//! paints` holds the two ends together. Fives are finer than the number is --
//! an apply passes through thirteen weighted stretches, not a hundred -- and
//! a step is a fiftieth of the screen, which is under a millimetre on this
//! panel.

use std::io::Write;
use std::process::ExitCode;

use console_notices::updating::{Far, far};

/// How coarse the fill is, in percent.
///
/// Every class this sends is a multiple of this, and the stylesheet has a rule
/// for each. Making it finer means writing more rules; making it coarser means
/// fewer. Five is where the steps stop being visible on this screen.
pub const STEP: u16 = 5;

fn main() -> ExitCode {
    println!("{}", said(far().as_ref()));
    let _ = std::io::stdout().flush();
    ExitCode::SUCCESS
}

/// The line waybar reads.
///
/// One object, always: waybar takes a line at a time and a line that is not an
/// object is a module that stops updating with no way back and nothing said.
pub fn said(far: Option<&Far>) -> String {
    let Some(far) = far else {
        return r#"{"text":""}"#.to_string();
    };

    // A space rather than nothing. waybar hides a module whose text is empty,
    // and a hidden module is a strip that paints no fill however far along the
    // apply is. There is nothing to read here; the space is what keeps the box
    // on the screen for the background to be painted in.
    format!(
        r#"{{"text":" ","tooltip":"{} — {}%","percentage":{},"class":"{}"}}"#,
        escaped(&far.doing),
        far.percent,
        far.percent.min(100),
        step(far.percent)
    )
}

/// Which of the stylesheet's rules this number falls under.
///
/// Rounded down, so the strip never says an apply has got further than it has.
pub fn step(percent: u16) -> String {
    format!("at-{}", percent.min(100) / STEP * STEP)
}

/// Every class this can ever send, which is what the stylesheet has to paint.
pub fn steps() -> Vec<String> {
    (0..=100 / STEP).map(|step| format!("at-{}", step * STEP)).collect()
}

/// The little of JSON's escaping a stretch's name can need.
///
/// The names are ours and none of them holds a quote or a backslash today.
/// This is here so that the day one does, the strip fills rather than stops.
fn escaped(doing: &str) -> String {
    doing.replace('\\', r"\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty text is a module waybar hides, which is what no apply running
    /// should look like: the strip is the colour of the bar and nothing else.
    #[test]
    fn nothing_running_draws_nothing() {
        let line: serde_json::Value = serde_json::from_str(&said(None)).expect("json");
        assert_eq!(line.get("text").and_then(|t| t.as_str()), Some(""));
        assert!(line.get("class").is_none(), "a hidden module still carries a class");
    }

    /// Every line is one JSON object, whatever it is drawn from. waybar reads
    /// a line at a time, and a line it cannot parse is a module that stops.
    #[test]
    fn every_line_is_an_object_waybar_can_read() {
        for percent in 0..=100_u16 {
            let far = Far { percent, doing: "building".to_string() };
            let line = said(Some(&far));
            let read: serde_json::Value = serde_json::from_str(&line)
                .unwrap_or_else(|_| panic!("not json at {percent}: {line}"));
            assert_eq!(read.get("text").and_then(|t| t.as_str()), Some(" "));
            assert_eq!(
                read.get("percentage").and_then(|p| p.as_u64()),
                Some(u64::from(percent))
            );
        }
    }

    /// A name with a quote in it is drawn rather than breaking the line.
    #[test]
    fn a_name_with_a_quote_in_it_does_not_break_the_line() {
        let far = Far { percent: 5, doing: r#"a "stretch" \ here"#.to_string() };
        let read: serde_json::Value = serde_json::from_str(&said(Some(&far))).expect("json");
        assert!(
            read.get("tooltip").and_then(|t| t.as_str()).is_some_and(|t| t.contains("stretch")),
            "the stretch's name did not survive being written into JSON"
        );
    }

    /// Every class it can send is one `steps` names.
    ///
    /// The stylesheet is written from `steps`, so a number that fell outside it
    /// would be a strip that stops filling and stays where it was -- which
    /// looks exactly like an apply that has hung.
    #[test]
    fn no_number_falls_outside_the_steps() {
        let steps = steps();
        for percent in 0..=200_u16 {
            let step = step(percent);
            assert!(steps.contains(&step), "{percent}% is class {step}, which nothing paints");
        }
    }

    /// It never says an apply has got further than it has.
    #[test]
    fn the_step_never_runs_ahead_of_the_number() {
        for percent in 0..=100_u16 {
            let at: u16 = step(percent).trim_start_matches("at-").parse().expect("a number");
            assert!(at <= percent, "{percent}% is painted as {at}%");
            assert!(percent - at < STEP, "{percent}% is painted as {at}%, a whole step short");
        }
    }

    /// The repository, which these last two are asked of.
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the repository")
    }

    /// Every class this can send is one the stylesheet paints.
    ///
    /// The two lists are in two files in two languages and nothing else holds
    /// them together. A step with no rule is a strip that stops where it was
    /// and stays there, which on a screen looks exactly like an apply that has
    /// hung -- and there is no message anywhere, because nothing went wrong.
    #[test]
    fn every_step_the_bar_can_send_is_one_the_style_paints() {
        let style =
            std::fs::read_to_string(root().join("files/home/@user@/.config/waybar/style.css"))
                .expect("the waybar stylesheet");
        for step in steps() {
            assert!(
                style.contains(&format!("#custom-updating.{step} {{")),
                "the strip can be {step} and the stylesheet has no rule for it"
            );
        }
    }

    /// And the stylesheet paints nothing this cannot send.
    ///
    /// The other direction, which catches the rule left behind when `STEP` is
    /// made coarser: dead CSS that reads as though the strip has fifty
    /// positions when it has ten.
    #[test]
    fn the_style_paints_no_step_the_bar_cannot_send() {
        let style =
            std::fs::read_to_string(root().join("files/home/@user@/.config/waybar/style.css"))
                .expect("the waybar stylesheet");
        let steps = steps();
        for painted in style
            .lines()
            .filter_map(|line| line.trim().strip_prefix("#custom-updating."))
            .filter_map(|rest| rest.split_whitespace().next())
        {
            assert!(
                steps.iter().any(|step| step == painted),
                "the stylesheet paints {painted}, which the strip can never be"
            );
        }
    }

    /// Empty at nothing and full at the end, which are the two the eye checks.
    #[test]
    fn it_starts_empty_and_ends_full() {
        assert_eq!(step(0), "at-0");
        assert_eq!(step(100), "at-100");
        assert_eq!(step(u16::MAX), "at-100", "a number past the end fills past the screen");
    }
}

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
//! Hence `at-0` to `at-100`, one rule in the stylesheet per class this can
//! send, and `every_step_the_bar_can_send_is_one_the_style_paints` holds the
//! two ends together.
//!
//! The step is a whole per cent because that is what the number is. It was five
//! when the number behind it only moved a handful of times in a run -- there
//! was nothing finer to say, and a shorter list is a shorter stylesheet. Both
//! ends of that changed: an apply now moves the number per crate, per file and
//! per package, and a check run moves it on the clock, so the number is
//! continuous and it was the stylesheet that made the strip jump. A step is a
//! hundredth of the screen now, and going finer would mean painting precision
//! nothing upstream has.

use std::io::Write;
use std::process::ExitCode;

use console_core_never::Never;
use console_notifications::updating::{Far, far};

pub const STEP: u16 = 1;

fn main() -> ExitCode {
    let Ok(far) = far();
    let Ok(said) = said(far.as_ref());

    println!("{said}");

    let _ = std::io::stdout().flush();

    ExitCode::SUCCESS
}

pub fn said(far: Option<&Far>) -> Result<String, Never> {
    let Some(far) = far else {
        return Ok(r#"{"text":""}"#.to_string());
    };

    let Ok(step) = step(far.percent);
    let Ok(doing) = escaped(&far.doing);

    Ok(format!(
        r#"{{"text":" ","tooltip":"{} — {}%","percentage":{},"class":"{}"}}"#,
        doing,
        far.percent,
        far.percent.min(100),
        step
    ))
}

pub fn step(percent: u16) -> Result<String, Never> {
    let at = percent.min(100).checked_div(STEP).unwrap_or(0).saturating_mul(STEP);

    Ok(format!("at-{at}"))
}

pub fn steps() -> Result<Vec<String>, Never> {
    Ok((0..=100u16.checked_div(STEP).unwrap_or(0))
        .map(|step| format!("at-{}", step.saturating_mul(STEP)))
        .collect())
}

fn escaped(doing: &str) -> Result<String, Never> {
    Ok(doing.replace('\\', r"\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_running_draws_nothing() {
        let Ok(said) = said(None);
        let line: serde_json::Value = serde_json::from_str(&said).expect("json");
        assert_eq!(line.get("text").and_then(|t| t.as_str()), Some(""));
        assert!(line.get("class").is_none(), "a hidden module still carries a class");
    }

    #[test]
    fn every_line_is_an_object_waybar_can_read() {
        for percent in 0..=100_u16 {
            let far = Far { percent, doing: "building".to_string() };
            let Ok(line) = said(Some(&far));
            let read: serde_json::Value = serde_json::from_str(&line)
                .unwrap_or_else(|_| panic!("not json at {percent}: {line}"));
            assert_eq!(read.get("text").and_then(|t| t.as_str()), Some(" "));
            assert_eq!(
                read.get("percentage").and_then(|p| p.as_u64()),
                Some(u64::from(percent))
            );
        }
    }

    #[test]
    fn a_name_with_a_quote_in_it_does_not_break_the_line() {
        let far = Far { percent: 5, doing: r#"a "stretch" \ here"#.to_string() };
        let Ok(said) = said(Some(&far));
        let read: serde_json::Value = serde_json::from_str(&said).expect("json");
        assert!(
            read.get("tooltip").and_then(|t| t.as_str()).is_some_and(|t| t.contains("stretch")),
            "the stretch's name did not survive being written into JSON"
        );
    }

    #[test]
    fn no_number_falls_outside_the_steps() {
        let Ok(steps) = steps();

        for percent in 0..=200_u16 {
            let Ok(step) = step(percent);

            assert!(steps.contains(&step), "{percent}% is class {step}, which nothing paints");
        }
    }

    #[test]
    fn the_step_never_runs_ahead_of_the_number() {
        for percent in 0..=100_u16 {
            let Ok(step) = step(percent);
            let at: u16 = step.trim_start_matches("at-").parse().expect("a number");
            assert!(at <= percent, "{percent}% is painted as {at}%");
            assert!(percent - at < STEP, "{percent}% is painted as {at}%, a whole step short");
        }
    }

    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the repository")
    }

    #[test]
    fn every_step_the_bar_can_send_is_one_the_style_paints() {
        let style =
            std::fs::read_to_string(root().join("files/home/@user@/.config/waybar/style.css"))
                .expect("the waybar stylesheet");
        let Ok(steps) = steps();

        for step in steps {
            assert!(
                style.contains(&format!("#custom-updating.{step} {{")),
                "the strip can be {step} and the stylesheet has no rule for it"
            );
        }
    }

    #[test]
    fn the_style_paints_no_step_the_bar_cannot_send() {
        let style =
            std::fs::read_to_string(root().join("files/home/@user@/.config/waybar/style.css"))
                .expect("the waybar stylesheet");
        let Ok(steps) = steps();

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

    #[test]
    fn it_starts_empty_and_ends_full() {
        assert_eq!(step(0), Ok("at-0".to_string()));
        assert_eq!(step(100), Ok("at-100".to_string()));
        assert_eq!(
            step(u16::MAX),
            Ok("at-100".to_string()),
            "a number past the end fills past the screen"
        );
    }
}

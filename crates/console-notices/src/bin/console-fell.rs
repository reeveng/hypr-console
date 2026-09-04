//! Say that a piece of the desktop died. Run by every console unit as it stops.
//!
//!     ExecStopPost=-/usr/local/bin/console-fell %n
//!
//! systemd puts the reason in the environment, and a clean stop is most of what
//! this sees: the target stopping at logout stops all of them. Only a service
//! that fell over says anything.
//!
//! The service still comes back on its own, because every one of them restarts.
//! What this adds is that somebody knows it happened. A daemon that dies and
//! repairs itself every few minutes looks exactly like a daemon that is
//! running, right up to `systemctl --user show -p NRestarts`, which nobody
//! thinks to ask for because nothing ever suggested it.

use console_notices::saying::{Kept, fault, for_the_journal, journal, raise};

/// What systemd calls a stop that was meant.
const WELL: &str = "success";

/// What systemd calls a unit that was asked whether to start and said no.
///
/// `ExecCondition=` is how the warm colours are switched off: the unit is there
/// and enabled, and it declines to start anything while the answer in the file
/// is no. systemd reports that as a result of its own rather than as a success,
/// and taken as a fault it is a card on the screen saying the screen's colour
/// daemon stopped on its own -- every boot, for as long as somebody prefers
/// their screen the colour it is.
///
/// It is not a fault. It is the one outcome that means nothing was started
/// because nothing was meant to be.
const DECLINED: &str = "exec-condition";

/// What is said about a unit that stopped on its own, or nothing if it did not.
///
/// `result` is systemd's `$SERVICE_RESULT`, which is missing when this is run
/// by hand. Missing is taken as a clean stop: the one thing this must never do
/// is cry about a desktop that is being shut down on purpose.
///
/// `said` is what the unit says it is -- its own `Description=` -- because
/// that is what a person reads. `console-bar.service stopped on its own` names
/// a file nobody holding this machine has seen; *The status bar stopped* is
/// the same fact in words they already have. The unit is kept in the body for
/// whoever is going to go and look, which is the same person a minute later.
pub fn fell(unit: &str, said: &str, result: Option<&str>) -> Option<(String, String, String)> {
    let how = result.unwrap_or(WELL);

    if how == WELL || how == DECLINED {
        return None;
    }

    let what = match said.trim().is_empty() {
        true => unit.to_string(),
        false => said.trim().to_string(),
    };

    Some((
        format!("unit-{unit}"),
        format!("{what} stopped, and is starting again"),
        format!(
            "It stopped on its own and the desktop is putting it back. If it keeps happening, \
             `journalctl --user -u {unit}` says why ({how})."
        ),
    ))
}

/// What a unit says it is, asked of systemd.
///
/// Empty where the manager will not say, which `fell` reads as "use the unit's
/// name": a card about a piece nobody can name is still better than no card.
fn described(unit: &str) -> String {
    let Ok(said) = std::process::Command::new("systemctl")
        .args(["--user", "show", "-p", "Description", "--value", unit])
        .output()
    else {
        return String::new();
    };

    String::from_utf8_lossy(&said.stdout).trim().to_string()
}

fn main() {
    let unit = std::env::args().nth(1).unwrap_or_else(|| "a piece of the desktop".to_string());

    // No $SERVICE_RESULT is this run by hand rather than by systemd, which
    // `fell` reads as a clean stop and so says nothing about.
    let Ok(result) = std::env::var("SERVICE_RESULT") else { return };

    let Some((kind, summary, body)) = fell(&unit, &described(&unit), Some(&result)) else {
        return;
    };

    journal(&for_the_journal(&kind, &summary, &body));

    if let Some(notice) = fault(&summary, &body, Kept::counting(&kind).again()) {
        raise(&notice);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The target stopping at logout stops all of them, and none of that is a
    /// fault. A desktop that cried on the way out would cry every single time.
    #[test]
    fn a_unit_that_was_stopped_on_purpose_says_nothing() {
        assert_eq!(fell("console-bar.service", "Status bar", Some("success")), None);
    }

    /// Run by hand, with no systemd around it to say.
    #[test]
    fn a_stop_with_no_reason_given_is_taken_as_a_clean_one() {
        assert_eq!(fell("console-bar.service", "Status bar", None), None);
    }

    /// The warm colours are switched off by a condition on their unit, so this
    /// is what a boot looks like on a machine where somebody said no. It has
    /// to be silent, or the switch comes with a notification attached to it.
    #[test]
    fn a_unit_whose_condition_said_not_to_start_says_nothing() {
        assert_eq!(
            fell("console-warm.service", "The colour of the screen, on a clock", Some("exec-condition")),
            None,
        );
    }

    /// The line at the top is in words a person has, and the line under it
    /// keeps the unit for whoever goes looking. A card that led with
    /// `console-bar.service` named a file nobody holding this machine has
    /// seen.
    #[test]
    fn a_unit_that_fell_over_says_so_in_words_and_says_where_to_look() {
        let (kind, summary, body) =
            fell("console-bar.service", "Status bar", Some("exit-code")).expect("a fall");
        assert_eq!(kind, "unit-console-bar.service");
        assert_eq!(summary, "Status bar stopped, and is starting again");
        assert!(!summary.contains("console-bar.service"), "the top line names a unit: {summary}");
        assert!(body.contains("journalctl --user -u console-bar.service"), "{body}");
    }

    /// A unit nothing described is named after itself rather than left blank.
    #[test]
    fn a_unit_with_no_description_is_said_by_its_name() {
        let (_, summary, _) =
            fell("something-else.service", "", Some("exit-code")).expect("a fall");
        assert!(summary.starts_with("something-else.service"), "{summary}");
    }

    /// The kind is what is counted, so it is the unit and never the reason: a
    /// service failing five different ways is one thing wrong, not five.
    #[test]
    fn one_unit_is_one_kind_however_many_ways_it_falls() {
        let one = fell("console-bar.service", "Status bar", Some("exit-code")).expect("a fall");
        let other = fell("console-bar.service", "Status bar", Some("signal")).expect("a fall");
        assert_eq!(one.0, other.0);
    }
}

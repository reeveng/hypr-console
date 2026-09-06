//! Say that a piece of the desktop died. Run by every console unit as it stops.
//!
//!     ExecStopPost=-/usr/local/bin/console-fell %n
//!
//! systemd puts the reason in the environment, and a clean stop is most of what
//! this sees: the target stopping at logout stops all of them. Only a service
//! that fell over says anything.
//!
//! The card is a name and a sentence: what fell, and that it was not asked to.
//! The reason systemd gives for the fall is a word like `core-dump`, which is a
//! thing to look up rather than a thing to read, so it goes to the journal line
//! and not to the screen.
//!
//! The service still comes back on its own, because every one of them restarts.
//! What this adds is that somebody knows it happened. A daemon that dies and
//! repairs itself every few minutes looks exactly like a daemon that is
//! running, right up to `systemctl --user show -p NRestarts`, which nobody
//! thinks to ask for because nothing ever suggested it.

use console_external_programs::Program;
use console_never::Never;
use console_notifications::saying::{Kept, fault, for_the_journal, journal, raise};

const WELL: &str = "success";

const DECLINED: &str = "exec-condition";

pub fn fell(
    unit: &str,
    said: &str,
    result: Option<&str>,
) -> Result<Option<(String, String, String)>, Never> {
    let how = result.unwrap_or(WELL);

    match how == WELL || how == DECLINED {
        true => return Ok(None),
        false => {}
    }

    let what = match said.trim().is_empty() {
        true => unit.to_string(),
        false => said.trim().to_string(),
    };

    Ok(Some((
        format!("unit-{unit}"),
        format!("{what} restarted"),
        "It stopped unexpectedly.".to_string(),
    )))
}

fn described(unit: &str) -> Result<String, Never> {
    let Ok(mut asking) = Program::Systemctl.command();

    let Ok(said) = asking
        .args(["--user", "show", "-p", "Description", "--value", unit])
        .output()
    else {
        return Ok(String::new());
    };

    Ok(String::from_utf8_lossy(&said.stdout).trim().to_string())
}

fn main() {
    let unit = std::env::args().nth(1).unwrap_or_else(|| "a piece of the desktop".to_string());

    let Ok(result) = std::env::var("SERVICE_RESULT") else { return };

    let Ok(described) = described(&unit);
    let Ok(fell) = fell(&unit, &described, Some(&result));

    let Some((kind, summary, body)) = fell else {
        return;
    };

    let Ok(said) = for_the_journal(&kind, &summary, &format!("{body} ({result})"));
    let Ok(()) = journal(&said);
    let Ok(counting) = Kept::counting(&kind);
    let Ok(again) = counting.again();
    let Ok(fault) = fault(&summary, &body, again);

    match fault {
        Some(notice) => {
            let Ok(_) = raise(&notice);
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    fn fell(unit: &str, said: &str, result: Option<&str>) -> Option<(String, String, String)> {
        let Ok(fell) = super::fell(unit, said, result);

        fell
    }

    #[test]
    fn a_unit_that_was_stopped_on_purpose_says_nothing() {
        assert_eq!(fell("console-bar.service", "Status bar", Some("success")), None);
    }

    #[test]
    fn a_stop_with_no_reason_given_is_taken_as_a_clean_one() {
        assert_eq!(fell("console-bar.service", "Status bar", None), None);
    }

    #[test]
    fn a_unit_whose_condition_said_not_to_start_says_nothing() {
        assert_eq!(
            fell("console-warm.service", "The colour of the screen, on a clock", Some("exec-condition")),
            None,
        );
    }

    #[test]
    fn a_unit_that_fell_over_says_so_in_a_line_anybody_can_read() {
        let (kind, summary, body) =
            fell("console-bar.service", "Status bar", Some("exit-code")).expect("a fall");
        assert_eq!(kind, "unit-console-bar.service");
        assert_eq!(summary, "Status bar restarted");
        assert!(!summary.contains("console-bar.service"), "the top line names a unit: {summary}");
        assert_eq!(body, "It stopped unexpectedly.");
    }

    #[test]
    fn a_unit_with_no_description_is_said_by_its_name() {
        let (_, summary, _) =
            fell("something-else.service", "", Some("exit-code")).expect("a fall");
        assert!(summary.starts_with("something-else.service"), "{summary}");
    }

    #[test]
    fn one_unit_is_one_kind_however_many_ways_it_falls() {
        let one = fell("console-bar.service", "Status bar", Some("exit-code")).expect("a fall");
        let other = fell("console-bar.service", "Status bar", Some("signal")).expect("a fall");
        assert_eq!(one.0, other.0);
    }
}

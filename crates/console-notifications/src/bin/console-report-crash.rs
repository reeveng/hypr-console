//! Say that a piece of the desktop died. Run by every console unit as it stops.
//!
//!     ExecStopPost=-/usr/local/bin/console-report-crash %n
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
//! What this adds is that someone knows it happened. A daemon that dies and
//! repairs itself every few minutes looks exactly like a daemon that is
//! running, right up to `systemctl --user show -p NRestarts`, which no one
//! thinks to ask for because nothing ever suggested it.

use console_core_external_programs::Program;
use console_core_never::Never;
use console_notifications::saying::{StatePath, Content, fault, for_the_journal, journal, raise};

const SOMETHING_OF_THE_DESKTOPS: &str = "Part of the desktop";


const SUCCESS: &str = "success";

const DECLINED: &str = "exec-condition";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stopped<'a> {
    pub unit: &'a str,
    pub said: &'a str,
}

pub fn crashed(
    stopped: Stopped<'_>,
    result: Option<&str>,
) -> Result<Option<(String, String, String)>, Never> {
    let Stopped { unit, said } = stopped;
    let how = match result {
        Some(how) => how,
        None => SUCCESS,
    };

    match how == SUCCESS || how == DECLINED {
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
        "It quit unexpectedly.".to_string(),
    )))
}

fn described(unit: &str) -> Result<String, Never> {
    let Ok(mut asking) = Program::Systemctl.command();

    let said = match asking
        .args(["--user", "show", "-p", "Description", "--value", unit])
        .output()
    {
        Ok(said) => said,
        Err(_would_not_start) => return Ok(String::new()),
    };

    Ok(String::from_utf8_lossy(&said.stdout).trim().to_string())
}

fn main() {
    let unit = match std::env::args().nth(1) {
        Some(unit) => unit,
        None => SOMETHING_OF_THE_DESKTOPS.to_string(),
    };

    #[cfg_attr(
        dylint_lib = "explicit026_env_read_once",
        allow(
            explicit026_env_read_once,
            reason = "SERVICE_RESULT is what systemd hands an OnFailure unit, and this binary is that unit. Nothing else in this tree is started that way"
        )
    )]
    let result = match std::env::var("SERVICE_RESULT") {
        Ok(result) => result,
        Err(_unset) => return,
    };

    let Ok(described) = described(&unit);
    let Ok(crashed) = crashed(Stopped { unit: &unit, said: &described }, Some(&result));

    let (kind, summary, body) = match crashed {
        Some((kind, summary, body)) => (kind, summary, body),
        None => return,
    };

    let body = format!("{body} ({result})");
    let Ok(said) = for_the_journal(&kind, Content { summary: &summary, body: &body });
    let Ok(()) = journal(&said);
    let Ok(counting) = StatePath::counting(&kind);
    let Ok(again) = counting.again();
    let Ok(fault) = fault(Content { summary: &summary, body: &body }, again);

    match fault {
        Some(notification) => {
            let Ok(_) = raise(&notification);
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::Stopped;

    const BAR: Stopped<'static> = Stopped { unit: "console-bar.service", said: "Status bar" };

    fn crashed(stopped: Stopped<'_>, result: Option<&str>) -> Option<(String, String, String)> {
        let Ok(crashed) = super::crashed(stopped, result);

        crashed
    }

    #[test]
    fn a_unit_that_was_stopped_on_purpose_says_nothing() {
        assert_eq!(crashed(BAR, Some("success")), None);
    }

    #[test]
    fn a_stop_with_no_reason_given_is_taken_as_a_clean_one() {
        assert_eq!(crashed(BAR, None), None);
    }

    #[test]
    fn a_unit_whose_condition_said_not_to_start_says_nothing() {
        assert_eq!(
            crashed(
                Stopped {
                    unit: "console-warm.service",
                    said: "The color of the screen, on a clock",
                },
                Some("exec-condition")
            ),
            None,
        );
    }

    #[test]
    fn a_unit_that_fell_over_says_so_in_a_line_anyone_can_read() {
        let (kind, summary, body) =
            crashed(BAR, Some("exit-code")).expect("a fall");
        assert_eq!(kind, "unit-console-bar.service");
        assert_eq!(summary, "Status bar restarted");
        assert!(!summary.contains("console-bar.service"), "the top line names a unit: {summary}");
        assert_eq!(body, "It quit unexpectedly.");
    }

    #[test]
    fn a_unit_with_no_description_is_said_by_its_name() {
        let untitled = Stopped { unit: "something-else.service", said: "" };
        let (_, summary, _) = crashed(untitled, Some("exit-code")).expect("a fall");
        assert!(summary.starts_with("something-else.service"), "{summary}");
    }

    #[test]
    fn one_unit_is_one_kind_however_many_ways_it_falls() {
        let one = crashed(BAR, Some("exit-code")).expect("a fall");
        let other = crashed(BAR, Some("signal")).expect("a fall");
        assert_eq!(one.0, other.0);
    }
}

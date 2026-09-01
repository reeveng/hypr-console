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

/// What is said about a unit that stopped on its own, or nothing if it did not.
///
/// `result` is systemd's `$SERVICE_RESULT`, which is missing when this is run
/// by hand. Missing is taken as a clean stop: the one thing this must never do
/// is cry about a desktop that is being shut down on purpose.
pub fn fell(unit: &str, result: Option<&str>) -> Option<(String, String, String)> {
    let how = result.unwrap_or(WELL);
    if how == WELL {
        return None;
    }
    Some((
        format!("unit-{unit}"),
        format!("{unit} stopped on its own"),
        format!(
            "systemd calls it {how}. It is being started again, and journalctl --user -u \
             {unit} says why."
        ),
    ))
}

fn main() {
    let unit = std::env::args().nth(1).unwrap_or_else(|| "a piece of the desktop".to_string());
    let result = std::env::var("SERVICE_RESULT").ok();
    let Some((kind, summary, body)) = fell(&unit, result.as_deref()) else {
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
        assert_eq!(fell("console-bar.service", Some("success")), None);
    }

    /// Run by hand, with no systemd around it to say.
    #[test]
    fn a_stop_with_no_reason_given_is_taken_as_a_clean_one() {
        assert_eq!(fell("console-bar.service", None), None);
    }

    #[test]
    fn a_unit_that_fell_over_says_so_and_says_where_to_look() {
        let (kind, summary, body) =
            fell("console-bar.service", Some("exit-code")).expect("a fall");
        assert_eq!(kind, "unit-console-bar.service");
        assert!(summary.contains("console-bar.service"));
        assert!(body.contains("journalctl --user -u console-bar.service"));
    }

    /// The kind is what is counted, so it is the unit and never the reason: a
    /// service failing five different ways is one thing wrong, not five.
    #[test]
    fn one_unit_is_one_kind_however_many_ways_it_falls() {
        let one = fell("console-bar.service", Some("exit-code")).expect("a fall");
        let other = fell("console-bar.service", Some("signal")).expect("a fall");
        assert_eq!(one.0, other.0);
    }
}

//! What the daemon is holding, and what it has already let go of.
//!
//! `console-notify` writes one file under the runtime directory whenever what
//! it holds changes, and this is that file: what is waiting on the screen now,
//! what has already gone, and whether the cards are being held back.
//!
//! ```json
//! {
//!   "waiting": [
//!     {
//!       "id": 3,
//!       "app_name": "Console",
//!       "summary": "Notifications fell over",
//!       "body": "console-notify.service stopped",
//!       "urgency": "critical"
//!     }
//!   ],
//!   "earlier": [],
//!   "quiet": "coming"
//! }
//! ```
//!
//! A file rather than a question put to the daemon, for the reason the strip
//! under the bar is a file: the bell reads this every time a notification
//! moves, and a reading that costs a subprocess and a round trip is a reading
//! the bar cannot take as often as the thing changes. What the daemon holds is
//! its own; what is here is a copy of it written where anything can look.
//!
//! Both halves are read in one go, which is the other half of the same
//! argument. The panel used to ask twice -- once for what was waiting and once
//! for the history -- and the two answers were two moments; a tab opened
//! between them showed a notification in neither list or in both.
//!
//! Nothing here is required to be there. Every field but the id is allowed to
//! be missing, because a notification is someone else's text; and a daemon
//! that is not running has written no file at all -- which is no notifications
//! rather than a fault of its own, because the bell has to go quiet when the
//! daemon dies rather than light up.

use console_core_never::Never;
use console_core_words::Words;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Words)]
pub enum Urgency {
    #[words(says = "", wearing = "soft", sent = "low")]
    Low,
    #[default]
    #[words(says = "", wearing = "", sent = "normal")]
    Normal,
    #[words(says = "Urgent", wearing = "wrong", sent = "critical")]
    Critical,
}

impl Urgency {
    fn named(said: Option<&str>) -> Result<Self, Never> {
        Ok(match said {
            Some("low") => Urgency::Low,
            Some("critical") => Urgency::Critical,
            Some(_) | None => Urgency::Normal,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Notification {
    pub id: u32,
    pub application: String,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Error {
    Yes,
    #[default]
    No,
}

impl Notification {
    pub fn wrong(&self) -> Result<Error, Never> {
        Ok(match self.urgency == Urgency::Critical {
            true => Error::Yes,
            false => Error::No,
        })
    }

    pub fn says(&self) -> Result<String, Never> {
        for said in [&self.summary, &self.body] {
            match said.trim().is_empty() {
                true => {}
                false => return Ok(said.trim().to_string()),
            }
        }

        Ok(format!("Notification {}", self.id))
    }
}

#[derive(Deserialize, Serialize)]
struct StoredNotification {
    id: u32,
    #[serde(rename = "app_name")]
    application_name: Option<String>,
    summary: Option<String>,
    body: Option<String>,
    urgency: Option<String>,
}

#[derive(Deserialize, Serialize, Default)]
struct StoredInbox {
    waiting: Vec<StoredNotification>,
    earlier: Vec<StoredNotification>,
    #[serde(rename = "quiet")]
    do_not_disturb: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Inbox {
    pub waiting: Vec<Notification>,
    pub earlier: Vec<Notification>,
    pub do_not_disturb: DoNotDisturb,
}

pub fn whole(said: &str) -> Result<Inbox, Never> {
    let kept = match serde_json::from_str::<StoredInbox>(said) {
        Ok(kept) => kept,
        Err(_not_json) => return Ok(Inbox::default()),
    };

    let Ok(waiting) = every(kept.waiting);
    let Ok(earlier) = every(kept.earlier);

    let do_not_disturb = match kept.do_not_disturb.as_deref() {
        Some("held-back") => DoNotDisturb::On,
        Some(_) | None => DoNotDisturb::Off,
    };

    Ok(Inbox { waiting, earlier, do_not_disturb })
}

pub fn written(whole: &Inbox) -> Result<String, Never> {
    let Ok(waiting) = spelled(&whole.waiting);
    let Ok(earlier) = spelled(&whole.earlier);
    let Ok(do_not_disturb) = whole.do_not_disturb.said();

    let kept = StoredInbox { waiting, earlier, do_not_disturb: Some(do_not_disturb.to_string()) };

    Ok(match serde_json::to_string(&kept) {
        Ok(said) => said,
        Err(_unserializable) => String::new(),
    })
}

fn spelled(held: &[Notification]) -> Result<Vec<StoredNotification>, Never> {
    let mut every: Vec<StoredNotification> = Vec::new();

    for notification in held {
        let Ok(urgency) = notification.urgency.sent();

        every.push(StoredNotification {
            id: notification.id,
            application_name: Some(notification.application.clone()),
            summary: Some(notification.summary.clone()),
            body: Some(notification.body.clone()),
            urgency: Some(urgency.to_string()),
        });
    }

    Ok(every)
}

pub fn read(said: &str) -> Result<Vec<Notification>, Never> {
    let held = match serde_json::from_str::<Vec<StoredNotification>>(said) {
        Ok(held) => held,
        Err(_not_json) => return Ok(Vec::new()),
    };

    every(held)
}

fn every(held: Vec<StoredNotification>) -> Result<Vec<Notification>, Never> {
    let mut every: Vec<Notification> = Vec::new();

    for said in held {
        let Ok(application) = word(said.application_name);
        let Ok(summary) = word(said.summary);
        let Ok(body) = word(said.body);
        let Ok(urgency) = Urgency::named(said.urgency.as_deref());

        every.push(Notification { id: said.id, application, summary, body, urgency });
    }

    Ok(every)
}

fn word(said: Option<String>) -> Result<String, Never> {
    Ok(match said {
        Some(said) => said,
        None => String::new(),
    })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Words)]
pub enum DoNotDisturb {
    #[words(said = "held-back")]
    On,
    #[default]
    #[words(said = "coming")]
    Off,
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = r#"[
  {
    "id": 4,
    "app_name": "Console",
    "app_icon": null,
    "category": null,
    "desktop_entry": null,
    "summary": "Notifications fell over",
    "body": "console-notify.service stopped",
    "urgency": "critical",
    "actions": {}
  },
  {
    "id": 3,
    "app_name": "Console",
    "app_icon": null,
    "category": null,
    "desktop_entry": null,
    "summary": "Listening",
    "body": null,
    "urgency": "low",
    "actions": {}
  }
]"#;

    const NONE: &str = "[\n]";

    #[test]
    fn what_is_being_held_is_read_whole() {
        let Ok(held) = read(TWO);

        assert_eq!(held.len(), 2);
        assert_eq!(held[0].id, 4);
        assert_eq!(held[0].summary, "Notifications fell over");
        assert_eq!(held[0].body, "console-notify.service stopped");
        assert_eq!(held[0].application, "Console");
    }

    #[test]
    fn the_order_is_left_as_it_was_written() {
        let Ok(held) = read(TWO);

        assert_eq!(held.iter().map(|held| held.id).collect::<Vec<_>>(), [4, 3]);
    }

    #[test]
    fn a_fault_says_it_is_one_and_nothing_else_does() {
        let Ok(held) = read(TWO);

        assert_eq!(held[0].wrong(), Ok(Error::Yes));
        assert_eq!(held[1].wrong(), Ok(Error::No));
        assert_eq!(held[0].urgency.says(), Ok("Urgent"));
    }

    #[test]
    fn only_a_fault_says_anything_beside_itself() {
        assert_eq!(Urgency::Normal.says(), Ok(""));
        assert_eq!(Urgency::Low.says(), Ok(""));
        assert_eq!(Urgency::Critical.says(), Ok("Urgent"));
    }

    #[test]
    fn a_notification_with_nothing_in_it_is_still_one() {
        let bare = r#"[{"id":9,"app_name":null,"summary":null,"body":null,"urgency":null}]"#;
        let Ok(held) = read(bare);

        assert_eq!(held.len(), 1);
        assert_eq!(held[0].urgency, Urgency::Normal);
        assert_eq!(held[0].says(), Ok("Notification 9".to_string()));
    }

    #[test]
    fn a_notification_with_no_summary_is_named_by_its_body() {
        let Ok(held) = read(r#"[{"id":2,"summary":"","body":"the microphone is on"}]"#);

        assert_eq!(held[0].says(), Ok("the microphone is on".to_string()));
    }

    #[test]
    fn an_answer_that_is_not_a_list_is_no_notifications() {
        for said in ["", NONE, "\n", "no", "{}", "[{}]", "  Urgency: critical"] {
            let Ok(held) = read(said);

            assert!(held.is_empty(), "{said:?}");
        }
    }

    #[test]
    fn what_the_daemon_is_holding_is_read_whole_and_in_one_go() {
        let Ok(whole) = whole(
            r#"{"waiting":[{"id":4,"app_name":"Console","summary":"One","body":"and its body","urgency":"critical"}],
                "earlier":[{"id":2,"summary":"Two"}],"quiet":"held-back"}"#,
        );

        assert_eq!(whole.waiting.len(), 1);
        assert_eq!(whole.waiting[0].body, "and its body");
        assert_eq!(whole.earlier.len(), 1);
        assert_eq!(whole.do_not_disturb, DoNotDisturb::On);
    }

    #[test]
    fn what_is_written_is_what_is_read_back() {
        let Ok(held) = read(TWO);
        let whole = Inbox { waiting: held.clone(), earlier: Vec::new(), do_not_disturb: DoNotDisturb::Off };
        let Ok(written) = written(&whole);
        let Ok(back) = whole_of(&written);

        assert_eq!(back, whole);
        assert_eq!(back.waiting[0].urgency, Urgency::Critical);
    }

    fn whole_of(said: &str) -> Result<Inbox, Never> {
        whole(said)
    }

    #[test]
    fn a_file_that_is_not_there_is_no_notifications_and_not_a_fault() {
        for said in ["", "no", "{}", "[]", "null"] {
            let Ok(whole) = whole(said);

            assert_eq!(whole, Inbox::default(), "{said:?}");
        }
    }

    #[test]
    fn a_daemon_that_is_quiet_says_so_in_the_file() {
        let quiet = Inbox { do_not_disturb: DoNotDisturb::On, ..Inbox::default() };
        let Ok(written) = written(&quiet);

        assert!(written.contains("held-back"), "{written}");
    }
}

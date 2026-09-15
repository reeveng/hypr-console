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
//! be missing, because a notification is somebody else's text; and a daemon
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
    #[words(says = "wrong", wearing = "wrong", sent = "critical")]
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
pub struct Notice {
    pub id: u32,
    pub app: String,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Wrong {
    Yes,
    #[default]
    No,
}

impl Notice {
    pub fn wrong(&self) -> Result<Wrong, Never> {
        Ok(match self.urgency == Urgency::Critical {
            true => Wrong::Yes,
            false => Wrong::No,
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
struct Said {
    id: u32,
    app_name: Option<String>,
    summary: Option<String>,
    body: Option<String>,
    urgency: Option<String>,
}

#[derive(Deserialize, Serialize, Default)]
struct Kept {
    waiting: Vec<Said>,
    earlier: Vec<Said>,
    quiet: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Whole {
    pub waiting: Vec<Notice>,
    pub earlier: Vec<Notice>,
    pub quiet: Quiet,
}

pub fn whole(said: &str) -> Result<Whole, Never> {
    let kept = match serde_json::from_str::<Kept>(said) {
        Ok(kept) => kept,
        Err(_fault) => return Ok(Whole::default()),
    };

    let Ok(waiting) = every(kept.waiting);
    let Ok(earlier) = every(kept.earlier);

    let quiet = match kept.quiet.as_deref() {
        Some("held-back") => Quiet::HeldBack,
        Some(_) | None => Quiet::Coming,
    };

    Ok(Whole { waiting, earlier, quiet })
}

pub fn written(whole: &Whole) -> Result<String, Never> {
    let Ok(waiting) = spelt(&whole.waiting);
    let Ok(earlier) = spelt(&whole.earlier);
    let Ok(quiet) = whole.quiet.said();

    let kept = Kept { waiting, earlier, quiet: Some(quiet.to_string()) };

    Ok(match serde_json::to_string(&kept) {
        Ok(said) => said,
        Err(_fault) => String::new(),
    })
}

fn spelt(held: &[Notice]) -> Result<Vec<Said>, Never> {
    let mut every: Vec<Said> = Vec::new();

    for notice in held {
        let Ok(urgency) = notice.urgency.sent();

        every.push(Said {
            id: notice.id,
            app_name: Some(notice.app.clone()),
            summary: Some(notice.summary.clone()),
            body: Some(notice.body.clone()),
            urgency: Some(urgency.to_string()),
        });
    }

    Ok(every)
}

pub fn read(said: &str) -> Result<Vec<Notice>, Never> {
    let held = match serde_json::from_str::<Vec<Said>>(said) {
        Ok(held) => held,
        Err(_fault) => return Ok(Vec::new()),
    };

    every(held)
}

fn every(held: Vec<Said>) -> Result<Vec<Notice>, Never> {
    let mut every: Vec<Notice> = Vec::new();

    for said in held {
        let Ok(app) = word(said.app_name);
        let Ok(summary) = word(said.summary);
        let Ok(body) = word(said.body);
        let Ok(urgency) = Urgency::named(said.urgency.as_deref());

        every.push(Notice { id: said.id, app, summary, body, urgency });
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
pub enum Quiet {
    #[words(said = "held-back")]
    HeldBack,
    #[default]
    #[words(said = "coming")]
    Coming,
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
        assert_eq!(held[0].app, "Console");
    }

    #[test]
    fn the_order_is_left_as_it_was_written() {
        let Ok(held) = read(TWO);

        assert_eq!(held.iter().map(|held| held.id).collect::<Vec<_>>(), [4, 3]);
    }

    #[test]
    fn a_fault_says_it_is_one_and_nothing_else_does() {
        let Ok(held) = read(TWO);

        assert_eq!(held[0].wrong(), Ok(Wrong::Yes));
        assert_eq!(held[1].wrong(), Ok(Wrong::No));
        assert_eq!(held[0].urgency.says(), Ok("wrong"));
    }

    #[test]
    fn only_a_fault_says_anything_beside_itself() {
        assert_eq!(Urgency::Normal.says(), Ok(""));
        assert_eq!(Urgency::Low.says(), Ok(""));
        assert_eq!(Urgency::Critical.says(), Ok("wrong"));
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
        assert_eq!(whole.quiet, Quiet::HeldBack);
    }

    #[test]
    fn what_is_written_is_what_is_read_back() {
        let Ok(held) = read(TWO);
        let whole = Whole { waiting: held.clone(), earlier: Vec::new(), quiet: Quiet::Coming };
        let Ok(written) = written(&whole);
        let Ok(back) = whole_of(&written);

        assert_eq!(back, whole);
        assert_eq!(back.waiting[0].urgency, Urgency::Critical);
    }

    fn whole_of(said: &str) -> Result<Whole, Never> {
        whole(said)
    }

    #[test]
    fn a_file_that_is_not_there_is_no_notifications_and_not_a_fault() {
        for said in ["", "no", "{}", "[]", "null"] {
            let Ok(whole) = whole(said);

            assert_eq!(whole, Whole::default(), "{said:?}");
        }
    }

    #[test]
    fn a_daemon_that_is_quiet_says_so_in_the_file() {
        let quiet = Whole { quiet: Quiet::HeldBack, ..Whole::default() };
        let Ok(written) = written(&quiet);

        assert!(written.contains("held-back"), "{written}");
    }
}

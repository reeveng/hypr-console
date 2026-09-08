//! What mako is holding, and what it has already let go of.
//!
//! `makoctl list -j` and `makoctl history -j` each print an array of objects,
//! one per notification, and that is what this reads:
//!
//! ```json
//! [
//!   {
//!     "id": 3,
//!     "app_name": "Console",
//!     "app_icon": null,
//!     "category": null,
//!     "desktop_entry": null,
//!     "summary": "Notifications fell over",
//!     "body": "console-notify.service stopped",
//!     "urgency": "critical",
//!     "actions": {}
//!   }
//! ]
//! ```
//!
//! Asked for as JSON because of the body. The form makoctl prints without
//! `-j` carries the id, the app, the urgency and the summary, and it does not
//! carry the body -- and the body is the half of a fault worth opening a panel
//! for: the summary says a thing broke and the body says which. A panel built
//! on the printed form would be a panel showing back the part somebody had
//! already read on the card.
//!
//! ## Two shapes, one daemon
//!
//! `-j` arrived in mako 1.11. In 1.10 the flag is not an error and not
//! honoured either: `run_list` there takes no options at all and prints the
//! plain form regardless, which is exactly what this device was seen to do.
//!
//! ```text
//! Notification 3: Notifications fell over
//!   App name: Console
//!   Urgency: critical
//! ```
//!
//! So both are read, and which one arrived decides which is used rather than
//! anything having to know what is installed. On 1.10 the panel lists what is
//! waiting and shows no bodies, because there are none to show; on 1.11 the
//! bodies are there. Asking twice, or asking the version first, would be two
//! more subprocesses on a reading the bar takes every time a notification
//! moves, to learn something the answer itself already says.
//!
//! This is also why the printed form is still read at all. It is the shape the
//! bell was built on and the only one that has been seen on the device, and a
//! bell that goes permanently empty is worse than no bell: it is a reading,
//! and it is wrong.
//!
//! Nothing here is required to be there. Every field but the id is `null` when
//! mako has nothing to put in it, a notification is somebody else's text, and
//! a mako that is not running answers nothing at all -- which is no
//! notifications rather than a fault of its own, because the bell has to go
//! quiet when the daemon dies rather than light up.

use console_core_never::Never;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
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

    pub fn says(self) -> Result<&'static str, Never> {
        Ok(match self {
            Urgency::Critical => "wrong",
            Urgency::Low | Urgency::Normal => "",
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

#[derive(Deserialize)]
struct Said {
    id: u32,
    app_name: Option<String>,
    summary: Option<String>,
    body: Option<String>,
    urgency: Option<String>,
}

pub fn read(said: &str) -> Result<Vec<Notice>, Never> {
    let held = match serde_json::from_str::<Vec<Said>>(said) {
        Ok(held) => held,
        Err(_fault) => return printed(said),
    };

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
    Ok(said.unwrap_or_default())
}

fn printed(said: &str) -> Result<Vec<Notice>, Never> {
    let mut held: Vec<Notice> = Vec::new();

    for line in said.lines() {
        let Ok(heads) = heads_one(line);

        match heads {
            Some((id, summary)) => {
                held.push(Notice { id, summary: summary.trim().to_string(), ..Notice::default() });
                continue;
            }
            None => {}
        }

        let notice = match held.last_mut() {
            Some(notice) => notice,
            None => continue,
        };

        match line.strip_prefix("  App name: ") {
            Some(app) => notice.app = app.trim().to_string(),
            None => {}
        }

        match line.strip_prefix("  Urgency: ") {
            Some(urgency) => {
                let Ok(named) = Urgency::named(Some(urgency.trim()));

                notice.urgency = named;
            }
            None => {}
        }
    }

    Ok(held)
}

fn heads_one(line: &str) -> Result<Option<(u32, &str)>, Never> {
    let after = match line.strip_prefix("Notification ") {
        Some(after) => after,
        None => return Ok(None),
    };

    let (id, summary) = match after.split_once(':') {
        Some((id, summary)) => (id, summary),
        None => return Ok(None),
    };

    let id = match id.parse::<u32>() {
        Ok(id) => id,
        Err(_fault) => return Ok(None),
    };

    Ok(Some((id, summary)))
}

pub const QUIET: &str = "do-not-disturb";

pub fn held_back(said: &str) -> Result<Quiet, Never> {
    Ok(match said.lines().any(|line| line.trim() == QUIET) {
        true => Quiet::HeldBack,
        false => Quiet::Coming,
    })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Quiet {
    HeldBack,
    #[default]
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
    fn what_mako_is_holding_is_read_whole() {
        let Ok(held) = read(TWO);

        assert_eq!(held.len(), 2);
        assert_eq!(held[0].id, 4);
        assert_eq!(held[0].summary, "Notifications fell over");
        assert_eq!(held[0].body, "console-notify.service stopped");
        assert_eq!(held[0].app, "Console");
    }

    #[test]
    fn the_order_is_left_as_mako_gave_it() {
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

    const PLAIN: &str = "\
Notification 4: Three: a summary with: colons
  App name: Console
  Urgency: normal
Notification 3: Two
  App name: Console
  Urgency: critical
Notification 2: One
  App name: Console
  Urgency: low";

    #[test]
    fn the_printed_form_is_read_as_the_same_notifications() {
        let Ok(held) = read(PLAIN);

        assert_eq!(held.len(), 3);
        assert_eq!(held[0].id, 4);
        assert_eq!(held[0].summary, "Three: a summary with: colons");
        assert_eq!(held[0].app, "Console");
        assert_eq!(held[1].urgency, Urgency::Critical);
        assert_eq!(held[2].urgency, Urgency::Low);
    }

    #[test]
    fn the_printed_form_carries_no_body_and_says_so_by_leaving_it_empty() {
        let Ok(held) = read(PLAIN);

        assert!(held.iter().all(|held| held.body.is_empty()));
    }

    #[test]
    fn a_summary_is_not_read_as_anything_but_a_summary() {
        let said = "Notification 1: Notification 2: gone\n  Urgency: low";
        let Ok(held) = read(said);

        assert_eq!(held.len(), 1);
        assert_eq!(held[0].summary, "Notification 2: gone");
    }

    #[test]
    fn what_is_written_under_a_notification_is_not_read_as_a_notification() {
        let said = "\
Notification 7: A download finished
  App name: Librewolf
  Urgency: normal
  Actions:
    App name: Open the folder";
        let Ok(held) = read(said);

        assert_eq!(held.len(), 1);
        assert_eq!(held[0].app, "Librewolf");
    }

    #[test]
    fn a_printed_notification_with_no_summary_is_still_one() {
        let Ok(held) = read("Notification 5:\n  Urgency: low");

        assert_eq!(held.len(), 1);
        assert_eq!(held[0].says(), Ok("Notification 5".to_string()));
    }

    #[test]
    fn the_mode_that_holds_them_back_is_read_by_name() {
        assert_eq!(held_back("default\ndo-not-disturb\n"), Ok(Quiet::HeldBack));
        assert_eq!(held_back("default\n"), Ok(Quiet::Coming));
        assert_eq!(held_back(""), Ok(Quiet::Coming));
        assert_eq!(held_back("do-not-disturb-later\n"), Ok(Quiet::Coming));
    }
}

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

use serde::Deserialize;

/// How loud a notification asked to be, which is the only thing that decides
/// whether it waits.
///
/// Everything `console-say` raises is critical and stays until it is seen.
/// Everything else here takes itself down after five seconds, which is why
/// what the panel is holding is nearly always something that went wrong.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

impl Urgency {
    /// What mako calls it, which is what it prints.
    fn named(said: Option<&str>) -> Self {
        match said {
            Some("low") => Urgency::Low,
            Some("critical") => Urgency::Critical,
            _ => Urgency::Normal,
        }
    }

    /// The word the panel puts beside a row, where there is one worth saying.
    ///
    /// Only a fault has one. Low and normal are both "something happened",
    /// which is what every row on the tab already means, and a column where
    /// two rows in three carry a word is a column somebody has to read past to
    /// find the one that matters. The whole reason for a word here is that a
    /// fault is the one notification worth opening.
    pub fn says(self) -> &'static str {
        match self {
            Urgency::Critical => "wrong",
            Urgency::Low | Urgency::Normal => "",
        }
    }
}

/// One notification, as much of it as mako keeps.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Notice {
    /// mako's own number for it, which is what it is dismissed by.
    pub id: u32,
    /// Who said it. "Console" for anything of ours.
    pub app: String,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
}

impl Notice {
    /// Whether this is one of the ones that waits until somebody has seen it.
    pub fn wrong(&self) -> bool {
        self.urgency == Urgency::Critical
    }

    /// What the row for it says, which is never nothing.
    ///
    /// A notification with no summary is allowed by the specification and is
    /// drawn by mako as a card with a body and no title. A row with nothing on
    /// it cannot be pressed on purpose, so the body stands in, and where there
    /// is neither the number does: something is there, and this is which one.
    pub fn says(&self) -> String {
        for said in [&self.summary, &self.body] {
            if !said.trim().is_empty() {
                return said.trim().to_string();
            }
        }
        format!("Notification {}", self.id)
    }
}

/// What one of them looks like on the way in, where everything can be missing.
#[derive(Deserialize)]
struct Said {
    id: u32,
    app_name: Option<String>,
    summary: Option<String>,
    body: Option<String>,
    urgency: Option<String>,
}

/// The notifications in an answer, whichever shape makoctl answered in.
///
/// JSON where it parses as JSON and the printed form otherwise. Nothing is
/// asked about which mako this is: an answer that is an array is the one, and
/// anything else is read the other way, which also makes an answer that is
/// neither into no notifications.
///
/// The order is mako's and is left alone. `sort=-time` in its configuration is
/// what puts the newest first, and that is the order the cards themselves are
/// stacked in on the screen, so the list reads down in the same direction the
/// screen does.
pub fn read(said: &str) -> Vec<Notice> {
    let Ok(held) = serde_json::from_str::<Vec<Said>>(said) else {
        return printed(said);
    };
    held.into_iter()
        .map(|said| Notice {
            id: said.id,
            app: word(said.app_name),
            summary: word(said.summary),
            body: word(said.body),
            urgency: Urgency::named(said.urgency.as_deref()),
        })
        .collect()
}

/// One of mako's `null`s, as the empty string it means.
fn word(said: Option<String>) -> String {
    said.unwrap_or_default()
}

/// The same, off the form makoctl prints when it cannot be asked for JSON.
///
/// A line per notification at the left margin, with its own lines indented
/// under it. The shape is checked rather than assumed, because a reader that
/// answers "nothing" while three notifications sit on the screen is worse than
/// no reader: an id has to be a number, and a line has to start at the margin.
fn printed(said: &str) -> Vec<Notice> {
    let mut held: Vec<Notice> = Vec::new();
    for line in said.lines() {
        if let Some((id, summary)) = heads_one(line) {
            held.push(Notice { id, summary: summary.trim().to_string(), ..Notice::default() });
            continue;
        }
        let Some(notice) = held.last_mut() else { continue };
        if let Some(app) = line.strip_prefix("  App name: ") {
            notice.app = app.trim().to_string();
        }
        if let Some(urgency) = line.strip_prefix("  Urgency: ") {
            notice.urgency = Urgency::named(Some(urgency.trim()));
        }
    }
    held
}

/// Whether a line is the first of a notification rather than one of its own,
/// and the id and summary on it if it is.
///
/// The summary is somebody else's words and lands on this line, so a line only
/// counts where what stands between `Notification` and the colon is a number.
/// Without that, a fault whose text happened to read "Notification 2: gone"
/// would be counted twice.
///
/// An indented line is never one of these. mako writes the keys under a
/// notification two spaces in and an action four, so the margin is what tells
/// a notification from everything written under it.
fn heads_one(line: &str) -> Option<(u32, &str)> {
    let (id, summary) = line.strip_prefix("Notification ")?.split_once(':')?;
    Some((id.parse().ok()?, summary))
}

/// The mode that keeps notifications off the screen without stopping them.
///
/// mako's own name for it, and the one its documentation uses. The panel's
/// switch adds and removes it; `files/home/@user@/.config/mako/config` is what
/// makes it mean anything, with an `invisible` under a criteria of this name.
pub const QUIET: &str = "do-not-disturb";

/// Whether notifications are being kept off the screen.
///
/// `makoctl mode` prints the modes one to a line, and `default` is always one
/// of them. Read by name rather than by counting, because a mode nobody here
/// added is a mode this has no opinion about.
pub fn held_back(said: &str) -> bool {
    said.lines().any(|line| line.trim() == QUIET)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `makoctl list -j` prints, in the shape makoctl.c writes it.
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

    /// What it prints when mako is holding nothing.
    const NONE: &str = "[\n]";

    #[test]
    fn what_mako_is_holding_is_read_whole() {
        let held = read(TWO);
        assert_eq!(held.len(), 2);
        assert_eq!(held[0].id, 4);
        assert_eq!(held[0].summary, "Notifications fell over");
        assert_eq!(held[0].body, "console-notify.service stopped");
        assert_eq!(held[0].app, "Console");
    }

    /// The order is mako's, which is the order the cards are stacked in.
    #[test]
    fn the_order_is_left_as_mako_gave_it() {
        assert_eq!(read(TWO).iter().map(|held| held.id).collect::<Vec<_>>(), [4, 3]);
    }

    /// A fault is the one kind that waits, and the panel colours it.
    #[test]
    fn a_fault_says_it_is_one_and_nothing_else_does() {
        let held = read(TWO);
        assert!(held[0].wrong());
        assert!(!held[1].wrong());
        assert_eq!(held[0].urgency.says(), "wrong");
    }

    /// Most of what arrives is ordinary, and a column of rows all saying so is
    /// a column with nothing in it to find. A fault is the one worth marking.
    #[test]
    fn only_a_fault_says_anything_beside_itself() {
        assert_eq!(Urgency::Normal.says(), "");
        assert_eq!(Urgency::Low.says(), "");
        assert_eq!(Urgency::Critical.says(), "wrong");
    }

    /// Every field but the id can be null, and a null is not a fault.
    #[test]
    fn a_notification_with_nothing_in_it_is_still_one() {
        let bare = r#"[{"id":9,"app_name":null,"summary":null,"body":null,"urgency":null}]"#;
        let held = read(bare);
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].urgency, Urgency::Normal);
        assert_eq!(held[0].says(), "Notification 9");
    }

    /// A card with a body and no title is drawn by mako and has to be a row
    /// here, so the body stands in for the name of it.
    #[test]
    fn a_notification_with_no_summary_is_named_by_its_body() {
        let held = read(r#"[{"id":2,"summary":"","body":"the microphone is on"}]"#);
        assert_eq!(held[0].says(), "the microphone is on");
    }

    /// A mako that is not running answers nothing, and so does one that
    /// answered with something this cannot read. The bell has to go quiet when
    /// the daemon dies rather than light up.
    #[test]
    fn an_answer_that_is_not_a_list_is_no_notifications() {
        for said in ["", NONE, "\n", "no", "{}", "[{}]", "  Urgency: critical"] {
            assert!(read(said).is_empty(), "{said:?}");
        }
    }

    // ------------------------------------------- what mako 1.10 prints instead

    /// Taken off the device, which is the one mako this has ever run against.
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

    /// The flag is ignored rather than refused by the mako that has no JSON,
    /// so this is what comes back and it has to be a list of notifications.
    #[test]
    fn the_printed_form_is_read_as_the_same_notifications() {
        let held = read(PLAIN);
        assert_eq!(held.len(), 3);
        assert_eq!(held[0].id, 4);
        assert_eq!(held[0].summary, "Three: a summary with: colons");
        assert_eq!(held[0].app, "Console");
        assert_eq!(held[1].urgency, Urgency::Critical);
        assert_eq!(held[2].urgency, Urgency::Low);
    }

    /// The one thing the printed form cannot say. A body it does not carry is
    /// an empty body, and the panel draws a notification without one.
    #[test]
    fn the_printed_form_carries_no_body_and_says_so_by_leaving_it_empty() {
        assert!(read(PLAIN).iter().all(|held| held.body.is_empty()));
    }

    /// A summary is somebody else's words, and one of these has colons in it
    /// on purpose. Counted by the number between the word and the colon, it is
    /// one notification; counted by the word alone it is two.
    #[test]
    fn a_summary_is_not_read_as_anything_but_a_summary() {
        let said = "Notification 1: Notification 2: gone\n  Urgency: low";
        let held = read(said);
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].summary, "Notification 2: gone");
    }

    /// mako writes what belongs to a notification two spaces in and an action
    /// four. An action is somebody else's words as well, so a notification
    /// offering one called "App name" must not rename who said it.
    #[test]
    fn what_is_written_under_a_notification_is_not_read_as_a_notification() {
        let said = "\
Notification 7: A download finished
  App name: Librewolf
  Urgency: normal
  Actions:
    App name: Open the folder";
        let held = read(said);
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].app, "Librewolf");
    }

    /// mako prints the line without a summary where a notification has none.
    #[test]
    fn a_printed_notification_with_no_summary_is_still_one() {
        let held = read("Notification 5:\n  Urgency: low");
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].says(), "Notification 5");
    }

    #[test]
    fn the_mode_that_holds_them_back_is_read_by_name() {
        assert!(held_back("default\ndo-not-disturb\n"));
        assert!(!held_back("default\n"));
        assert!(!held_back(""));
        assert!(!held_back("do-not-disturb-later\n"));
    }
}

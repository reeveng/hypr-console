//! What the panel holds, as a function of what mako said.
//!
//! Reading mako is one thing and knowing what to draw from it is another.
//! Everything here is the second, so the shape of both tabs can be asked
//! without a mako to ask.

use std::sync::Arc;

use console_panel::page::{Does, Row, Showing};

use crate::reading::Notice;

/// The two places this panel has, which are the two states a notification is
/// in: still on the screen, or already gone from it.
///
/// Two rather than one because they are answers to different questions.
/// "What is that bell about" is the first; "what did it say a minute ago,
/// while I was looking at something else" is the second, and until there was
/// somewhere to ask it the answer was the journal.
pub const TABS: [&str; 2] = ["Waiting", "Earlier"];

/// What a row hands back to whoever is holding where the panel is looking.
pub type Chosen = Arc<dyn Fn(&dyn Showing) + Send + Sync>;

/// The word beside a notification in the list.
///
/// A fault says so, and everything else says who raised it. Nearly everything
/// on this machine is ours and is either a fault or ordinary, so "wrong" is
/// what separates the row worth opening from the four that are not; an
/// application that is not ours is rare enough that its name is the
/// interesting thing about it, and there is nothing else to put there anyway.
pub fn aside(notice: &Notice) -> String {
    match notice.urgency.says() {
        "" => notice.app.clone(),
        said => said.to_string(),
    }
}

/// What is on the screen now, and the way to clear it.
///
/// Notifications and nothing else. The switch that keeps them off the screen
/// used to stand at the bottom of this tab, and on a desktop with nothing
/// waiting -- which is the desktop most of the time -- it was the only thing
/// here that could be pressed at all: the bell opened onto one grey line
/// saying nothing was waiting and one preference. A preference is not a
/// notification, and the place for it is where the other preferences are, so
/// it lives on the settings panel's own Notifications tab now.
pub fn waiting_rows(
    held: &[Notice],
    open: impl Fn(&Notice) -> Does,
    clear: Does,
) -> Vec<Row> {
    let mut rows: Vec<Row> = held
        .iter()
        .map(|notice| Row::new(&notice.says(), &aside(notice), open(notice)).opening())
        .collect();

    if rows.is_empty() {
        rows.push(Row::nothing("Nothing is waiting"));
    } else {
        // No question asked before it. Everything cleared here is in Earlier a
        // moment later, so this is a row that moves things rather than one
        // that throws them away, and a question about a press that can be
        // walked back is a question somebody learns to answer without reading.
        rows.push(Row::new("Clear them all", "", clear));
    }
    rows
}

/// One notification, whole.
///
/// The card it was drawn on is 320 by 140 and the body is what does not fit in
/// it, so this is the only place the whole of a fault can be read. The summary
/// and the body are drawn as the two halves of one block of text under the
/// name of whatever said them, which is how a notification is laid out
/// everywhere else it is drawn.
pub fn one_rows(notice: &Notice, back: &Chosen, dismiss: Does) -> Vec<Row> {
    let going = Arc::clone(back);
    let mut rows = vec![
        Row::back(TABS[0], move |showing| going(showing)),
        Row::said(&said_by(notice), &notice.says()),
    ];
    // Where the body is all the notification had, `says` is already showing
    // it, and a second row saying the same thing reads as the panel stuttering.
    if !notice.body.trim().is_empty() && notice.body.trim() != notice.says() {
        rows.push(Row::said("", notice.body.trim()));
    }
    rows.push(Row::new("Dismiss", "", dismiss));
    rows
}

/// Whose notification this is, said in the column the guide says a button in.
///
/// "Said" where nothing claimed it. An application is not obliged to name
/// itself and a row whose left half is empty reads as a row that failed to
/// load rather than as one with nothing to say there.
fn said_by(notice: &Notice) -> String {
    match notice.app.trim().is_empty() {
        true => "Said".to_string(),
        false => notice.app.trim().to_string(),
    }
}

/// A notification whose id mako no longer knows.
///
/// It can go while its own page is open: it runs out of seconds, or a thumb
/// takes the card down, or something else clears the lot. The page stays where
/// it is and says so rather than emptying itself, because a panel that steps
/// back out on its own is a panel that moved while somebody was reading it.
pub fn gone_rows(back: &Chosen) -> Vec<Row> {
    let going = Arc::clone(back);
    vec![
        Row::back(TABS[0], move |showing| going(showing)),
        Row::nothing("It has gone"),
    ]
}

/// What has been cleared, and what it said.
///
/// Read and never chosen: a notification mako has let go of cannot be
/// dismissed again, and there is nothing else to do to one. So both halves are
/// drawn at once, in the two columns a row that is only read is given, and the
/// whole tab can be gone down without opening anything.
pub fn earlier_rows(held: &[Notice]) -> Vec<Row> {
    if held.is_empty() {
        return vec![Row::nothing("Nothing has been cleared yet")];
    }
    held.iter()
        .map(|notice| Row::said(&notice.says(), &earlier_aside(notice)))
        .collect()
}

/// The body, or the urgency where there was no body.
fn earlier_aside(notice: &Notice) -> String {
    match notice.body.trim().is_empty() {
        true => aside(notice),
        false => notice.body.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reading::Urgency;

    fn nothing() -> Does {
        Does::and_stay(|_| ())
    }

    fn opening(_: &Notice) -> Does {
        nothing()
    }

    fn back() -> Chosen {
        Arc::new(|_: &dyn Showing| ())
    }

    fn fault() -> Notice {
        Notice {
            id: 4,
            app: "Console".to_string(),
            summary: "Notifications fell over".to_string(),
            body: "console-notify.service stopped".to_string(),
            urgency: Urgency::Critical,
        }
    }

    fn ordinary() -> Notice {
        Notice {
            id: 3,
            app: "Librewolf".to_string(),
            summary: "A download finished".to_string(),
            urgency: Urgency::Low,
            ..Notice::default()
        }
    }

    /// The bell was tapped because something is waiting, so the first press of
    /// A has to land on that rather than on the switch under it.
    #[test]
    fn the_first_row_that_does_anything_is_a_notification() {
        let rows = waiting_rows(&[fault()], opening, nothing());
        let first = rows.iter().position(Row::acts).expect("a row that acts");
        assert_eq!(rows[first].says, "Notifications fell over");
    }

    /// Every one of them opens onto the whole of what it said, and says so.
    #[test]
    fn a_notification_says_that_it_opens() {
        let rows = waiting_rows(&[fault()], opening, nothing());
        assert!(rows[0].opens);
    }

    /// A fault is the one worth finding in a list, so it is the one the list
    /// marks.
    #[test]
    fn a_fault_is_marked_and_an_ordinary_notification_is_named_by_its_app() {
        assert_eq!(aside(&fault()), "wrong");
        assert_eq!(aside(&ordinary()), "Librewolf");
    }

    /// Nothing to clear means no row for clearing it. A row that does nothing
    /// is worse than a missing one: it is pressed, and then the panel is
    /// broken.
    #[test]
    fn there_is_nothing_to_clear_when_nothing_is_waiting() {
        let rows = waiting_rows(&[], opening, nothing());
        assert!(!rows.iter().any(|row| row.says == "Clear them all"));
        assert_eq!(rows[0].says, "Nothing is waiting");
    }

    #[test]
    fn what_is_waiting_can_be_cleared_in_one_press() {
        let rows = waiting_rows(&[fault(), ordinary()], opening, nothing());
        assert!(rows.iter().any(|row| row.says == "Clear them all" && row.acts()));
    }

    /// Nothing but notifications. The switch that keeps them off the screen is
    /// a preference and lives with the preferences: on a desktop with nothing
    /// waiting it was the only thing on this tab anybody could press, which
    /// made the bell open onto a page about itself.
    #[test]
    fn the_tab_holds_notifications_and_no_preferences() {
        for held in [Vec::new(), vec![fault()]] {
            let rows = waiting_rows(&held, opening, nothing());
            assert!(
                !rows.iter().any(|row| row.says.contains("off the screen")),
                "a preference is still standing on the notifications tab"
            );
        }
    }

    /// The whole of a fault: who said it, what it said, and the line the card
    /// was too small to show.
    #[test]
    fn opening_one_shows_the_body_the_card_could_not_fit() {
        let rows = one_rows(&fault(), &back(), nothing());
        let said: Vec<&str> = rows.iter().map(|row| row.aside.as_str()).collect();
        assert!(said.contains(&"console-notify.service stopped"), "{said:?}");
        assert_eq!(rows[1].says, "Console");
    }

    /// Row nought is the way back, wherever you are.
    #[test]
    fn the_way_back_is_the_first_row_of_a_notification() {
        for rows in [one_rows(&fault(), &back(), nothing()), gone_rows(&back())] {
            assert!(rows[0].says.ends_with(TABS[0]), "{}", rows[0].says);
            assert!(rows[0].acts());
        }
    }

    /// A notification with nothing but a summary is one row of text, not one
    /// row of text and an empty one under it.
    #[test]
    fn a_notification_with_no_body_is_not_given_an_empty_line() {
        let rows = one_rows(&ordinary(), &back(), nothing());
        assert!(!rows.iter().any(|row| row.says.is_empty() && row.aside.is_empty()));
        assert_eq!(rows.len(), 3);
    }

    /// A card with a body and no title is drawn by mako, and this panel would
    /// otherwise say it twice: once as the name of the row and once under it.
    #[test]
    fn a_notification_that_is_only_a_body_says_it_once() {
        let bodied = Notice { body: "the microphone is on".to_string(), ..Notice::default() };
        let rows = one_rows(&bodied, &back(), nothing());
        let times = rows.iter().filter(|row| row.aside == "the microphone is on").count();
        assert_eq!(times, 1);
    }

    /// Every notification opened has a way to take it off the screen, which is
    /// the one thing there is to do to one.
    #[test]
    fn a_notification_can_be_dismissed_where_it_is_read() {
        let rows = one_rows(&fault(), &back(), nothing());
        assert!(rows.iter().any(|row| row.says == "Dismiss" && row.acts()));
    }

    /// What has been cleared is read and not chosen, so both halves of it are
    /// drawn at once rather than behind a press.
    #[test]
    fn what_was_cleared_is_read_where_it_stands() {
        let rows = earlier_rows(&[fault()]);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].acts());
        assert_eq!(rows[0].says, "Notifications fell over");
        assert_eq!(rows[0].aside, "console-notify.service stopped");
    }

    #[test]
    fn an_empty_history_says_so_rather_than_drawing_nothing() {
        assert_eq!(earlier_rows(&[]).len(), 1);
        assert!(!earlier_rows(&[])[0].acts());
    }
}

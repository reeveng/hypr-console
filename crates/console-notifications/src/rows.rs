//! What the panel holds, as a function of what is held.
//!
//! Reading what the daemon wrote is one thing and knowing what to draw from it
//! is another. Everything here is the second, so the shape of both tabs can be
//! asked with no daemon running and no screen to draw on.

use std::sync::Arc;

use console_core_never::Never;
use console_panel::page::{Aside, Handler, Row, Showing};

use crate::reading::Notification;

pub const TABS: [&str; 2] = ["New", "Earlier"];

pub fn tab(at: u32) -> Result<&'static str, Never> {
    let Ok(at) = console_core_number_conversion::index(at);

    Ok(match TABS.get(at).copied() {
        Some(tab) => tab,
        None => NO_SUCH_TAB,
    })
}

const NO_SUCH_TAB: &str = "";

pub type Chosen = Arc<dyn Fn(&dyn Showing) + Send + Sync>;

pub fn aside(notification: &Notification) -> Result<String, Never> {
    let Ok(says) = notification.urgency.says();

    Ok(match says {
        "" => notification.app.clone(),
        said => said.to_string(),
    })
}

pub fn waiting_rows(
    held: &[Notification],
    open: impl Fn(&Notification) -> Handler,
    clear: Handler,
) -> Result<Vec<Row>, Never> {
    let mut rows: Vec<Row> = held
        .iter()
        .map(|notification| {
            let Ok(said) = aside(notification);
            let Ok(says) = notification.says();
            let Ok(row) = Row::new(&says, Aside(&said), open(notification));
            let Ok(opens) = row.opening();

            opens
        })
        .collect();

    match rows.is_empty() {
        true => {
            let Ok(row) = Row::nothing("No Notifications");

            rows.push(row);
        }
        false => {
            let Ok(row) = Row::new("Clear all", Aside(""), clear);

            rows.push(row);
        }
    }

    Ok(rows)
}

pub fn one_rows(notification: &Notification, back: &Chosen, dismiss: Handler) -> Result<Vec<Row>, Never> {
    let going = Arc::clone(back);
    let Ok(first) = tab(0);
    let Ok(way_back) = Row::back(first, move |showing| going(showing));
    let Ok(by) = said_by(notification);
    let Ok(says) = notification.says();
    let Ok(heading) = Row::said(&by, Aside(&says));
    let mut rows = vec![way_back, heading];

    let worth_a_row = !notification.body.trim().is_empty() && notification.body.trim() != says;

    match worth_a_row {
        true => {
            let Ok(body) = Row::said("", Aside(notification.body.trim()));

            rows.push(body);
        }
        false => {}
    }

    let Ok(row) = Row::new("Clear", Aside(""), dismiss);

    rows.push(row);

    Ok(rows)
}

fn said_by(notification: &Notification) -> Result<String, Never> {
    Ok(match notification.app.trim().is_empty() {
        true => "Notification".to_string(),
        false => notification.app.trim().to_string(),
    })
}

pub fn cleared_rows(back: &Chosen) -> Result<Vec<Row>, Never> {
    let going = Arc::clone(back);
    let Ok(first) = tab(0);
    let Ok(way_back) = Row::back(first, move |showing| going(showing));
    let Ok(cleared) = Row::nothing("Cleared");

    Ok(vec![way_back, cleared])
}

pub fn earlier_rows(held: &[Notification]) -> Result<Vec<Row>, Never> {
    match held.is_empty() {
        true => {
            let Ok(row) = Row::nothing("No Earlier Notifications");

            return Ok(vec![row]);
        }
        false => {}
    }

    Ok(held
        .iter()
        .map(|notification| {
            let Ok(said) = earlier_aside(notification);
            let Ok(says) = notification.says();
            let Ok(row) = Row::said(&says, Aside(&said));

            row
        })
        .collect())
}

fn earlier_aside(notification: &Notification) -> Result<String, Never> {
    Ok(match notification.body.trim().is_empty() {
        true => {
            let Ok(said) = aside(notification);

            said
        }
        false => notification.body.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use console_panel::page::Action;
    use super::*;
    use crate::reading::Urgency;

    fn nothing() -> Handler {
        let Ok(does) = Handler::and_stay(|_| ());

        does
    }

    fn opening(_: &Notification) -> Handler {
        nothing()
    }

    fn back() -> Chosen {
        Arc::new(|_: &dyn Showing| ())
    }

    fn waiting(held: &[Notification]) -> Vec<Row> {
        let Ok(rows) = waiting_rows(held, opening, nothing());

        rows
    }

    fn one(notification: &Notification) -> Vec<Row> {
        let Ok(rows) = one_rows(notification, &back(), nothing());

        rows
    }

    fn cleared() -> Vec<Row> {
        let Ok(rows) = cleared_rows(&back());

        rows
    }

    fn earlier(held: &[Notification]) -> Vec<Row> {
        let Ok(rows) = earlier_rows(held);

        rows
    }

    fn acts(row: &Row) -> Action {
        let Ok(acts) = row.acts();

        acts
    }

    fn beside(notification: &Notification) -> String {
        let Ok(said) = aside(notification);

        said
    }

    fn fault() -> Notification {
        Notification {
            id: 4,
            app: "Console".to_string(),
            summary: "Notifications fell over".to_string(),
            body: "console-notify.service stopped".to_string(),
            urgency: Urgency::Critical,
        }
    }

    fn ordinary() -> Notification {
        Notification {
            id: 3,
            app: "Librewolf".to_string(),
            summary: "A download finished".to_string(),
            urgency: Urgency::Low,
            ..Notification::default()
        }
    }

    #[test]
    fn the_first_row_that_does_anything_is_a_notification() {
        let rows = waiting(&[fault()]);
        let first = rows.iter().find(|row| acts(row) == Action::Yes).expect("a row that acts");
        assert_eq!(first.says, "Notifications fell over");
    }

    #[test]
    fn a_notification_says_that_it_opens() {
        let rows = waiting(&[fault()]);
        assert!(rows[0].opens);
    }

    #[test]
    fn a_fault_is_marked_and_an_ordinary_notification_is_named_by_its_app() {
        assert_eq!(beside(&fault()), "Urgent");
        assert_eq!(beside(&ordinary()), "Librewolf");
    }

    #[test]
    fn there_is_nothing_to_clear_when_nothing_is_waiting() {
        let rows = waiting(&[]);
        assert!(!rows.iter().any(|row| row.says == "Clear all"));
        assert_eq!(rows[0].says, "No Notifications");
    }

    #[test]
    fn what_is_waiting_can_be_cleared_in_one_press() {
        let rows = waiting(&[fault(), ordinary()]);
        assert!(rows.iter().any(|row| row.says == "Clear all" && acts(row) == Action::Yes));
    }

    #[test]
    fn the_tab_holds_notifications_and_no_preferences() {
        for held in [Vec::new(), vec![fault()]] {
            let rows = waiting(&held);
            assert!(
                !rows.iter().any(|row| row.says.contains("off the screen")),
                "a preference is still standing on the notifications tab"
            );
        }
    }

    #[test]
    fn opening_one_shows_the_body_the_card_could_not_fit() {
        let rows = one(&fault());
        let said: Vec<&str> = rows.iter().map(|row| row.aside.as_str()).collect();
        assert!(said.contains(&"console-notify.service stopped"), "{said:?}");
        assert_eq!(rows[1].says, "Console");
    }

    #[test]
    fn the_way_back_is_the_first_row_of_a_notification() {
        for rows in [one(&fault()), cleared()] {
            assert!(rows[0].says.ends_with(TABS[0]), "{}", rows[0].says);
            assert_eq!(acts(&rows[0]), Action::Yes);
        }
    }

    #[test]
    fn a_notification_with_no_body_is_not_given_an_empty_line() {
        let rows = one(&ordinary());
        assert!(!rows.iter().any(|row| row.says.is_empty() && row.aside.is_empty()));
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn a_notification_that_is_only_a_body_says_it_once() {
        let bodied = Notification { body: "the microphone is on".to_string(), ..Notification::default() };
        let rows = one(&bodied);
        assert_eq!(rows.iter().filter(|row| row.aside == "the microphone is on").count(), 1);
    }

    #[test]
    fn a_notification_can_be_dismissed_where_it_is_read() {
        let rows = one(&fault());
        assert!(rows.iter().any(|row| row.says == "Clear" && acts(row) == Action::Yes));
    }

    #[test]
    fn what_was_cleared_is_read_where_it_stands() {
        let rows = earlier(&[fault()]);
        assert_eq!(rows.len(), 1);
        assert_eq!(acts(&rows[0]), Action::None);
        assert_eq!(rows[0].says, "Notifications fell over");
        assert_eq!(rows[0].aside, "console-notify.service stopped");
    }

    #[test]
    fn an_empty_history_says_so_rather_than_drawing_nothing() {
        assert_eq!(earlier(&[]).len(), 1);
        assert_eq!(acts(&earlier(&[])[0]), Action::None);
    }
}

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
        "" => notification.application.clone(),
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
            let Ok(row) = Row::placeholder("No Notifications");

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
    let Ok(heading) = Row::text(&by, Aside(&says));
    let mut rows = vec![way_back, heading];

    let worth_a_row = !notification.body.trim().is_empty() && notification.body.trim() != says;

    match worth_a_row {
        true => {
            let Ok(body) = Row::text("", Aside(notification.body.trim()));

            rows.push(body);
        }
        false => {}
    }

    let Ok(row) = Row::new("Clear", Aside(""), dismiss);

    rows.push(row);

    Ok(rows)
}

fn said_by(notification: &Notification) -> Result<String, Never> {
    Ok(match notification.application.trim().is_empty() {
        true => "Notification".to_string(),
        false => notification.application.trim().to_string(),
    })
}

pub fn cleared_rows(back: &Chosen) -> Result<Vec<Row>, Never> {
    let going = Arc::clone(back);
    let Ok(first) = tab(0);
    let Ok(way_back) = Row::back(first, move |showing| going(showing));
    let Ok(cleared) = Row::placeholder("Cleared");

    Ok(vec![way_back, cleared])
}

pub fn earlier_rows(held: &[Notification]) -> Result<Vec<Row>, Never> {
    match held.is_empty() {
        true => {
            let Ok(row) = Row::placeholder("No Earlier Notifications");

            return Ok(vec![row]);
        }
        false => {}
    }

    Ok(held
        .iter()
        .map(|notification| {
            let Ok(said) = earlier_aside(notification);
            let Ok(says) = notification.says();
            let Ok(row) = Row::text(&says, Aside(&said));

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

    type NotFound = &'static str;

    fn nothing() -> Result<Handler, Never> {
        Handler::and_stay(|_| ())
    }

    fn back() -> Result<Chosen, Never> {
        Ok(Arc::new(|_: &dyn Showing| ()))
    }

    fn waiting(held: &[Notification]) -> Result<Vec<Row>, Never> {
        let Ok(clear) = nothing();

        waiting_rows(
            held,
            |_| {
                let Ok(does) = nothing();

                does
            },
            clear,
        )
    }

    fn one(notification: &Notification) -> Result<Vec<Row>, Never> {
        let Ok(back) = back();
        let Ok(dismiss) = nothing();

        one_rows(notification, &back, dismiss)
    }

    fn cleared() -> Result<Vec<Row>, Never> {
        let Ok(back) = back();

        cleared_rows(&back)
    }

    fn fault() -> Result<Notification, Never> {
        Ok(Notification {
            id: 4,
            application: "Console".to_string(),
            summary: "Notifications fell over".to_string(),
            body: "console-notify.service stopped".to_string(),
            urgency: Urgency::Critical,
        })
    }

    fn ordinary() -> Result<Notification, Never> {
        Ok(Notification {
            id: 3,
            application: "Librewolf".to_string(),
            summary: "A download finished".to_string(),
            urgency: Urgency::Low,
            ..Notification::default()
        })
    }

    #[test]
    fn the_first_row_that_does_anything_is_a_notification() -> Result<(), NotFound> {
        let Ok(fault) = fault();
        let Ok(rows) = waiting(&[fault]);
        let first = rows.iter().find(|row| matches!(row.acts(), Ok(Action::Yes))).ok_or("no row acts")?;

        assert_eq!(first.says, "Notifications fell over");

        Ok(())
    }

    #[test]
    fn a_notification_says_that_it_opens() {
        let Ok(fault) = fault();
        let Ok(rows) = waiting(&[fault]);

        assert!(rows.first().is_some_and(|row| row.opens));
    }

    #[test]
    fn a_fault_is_marked_and_an_ordinary_notification_is_named_by_its_app() {
        let Ok(fault) = fault();
        let Ok(ordinary) = ordinary();

        assert_eq!(aside(&fault), Ok("Urgent".to_string()));
        assert_eq!(aside(&ordinary), Ok("Librewolf".to_string()));
    }

    #[test]
    fn there_is_nothing_to_clear_when_nothing_is_waiting() {
        let Ok(rows) = waiting(&[]);

        assert!(!rows.iter().any(|row| row.says == "Clear all"));
        assert_eq!(rows.first().map(|row| row.says.as_str()), Some("No Notifications"));
    }

    #[test]
    fn what_is_waiting_can_be_cleared_in_one_press() {
        let Ok(fault) = fault();
        let Ok(ordinary) = ordinary();
        let Ok(rows) = waiting(&[fault, ordinary]);

        assert!(rows.iter().filter(|row| matches!(row.acts(), Ok(Action::Yes))).any(|row| row.says == "Clear all"));
    }

    #[test]
    fn the_tab_holds_notifications_and_no_preferences() {
        let Ok(fault) = fault();

        for held in [Vec::new(), vec![fault]] {
            let Ok(rows) = waiting(&held);

            assert!(
                !rows.iter().any(|row| row.says.contains("off the screen")),
                "a preference is still standing on the notifications tab"
            );
        }
    }

    #[test]
    fn opening_one_shows_the_body_the_card_could_not_fit() {
        let Ok(fault) = fault();
        let Ok(rows) = one(&fault);
        let said: Vec<&str> = rows.iter().map(|row| row.aside.as_str()).collect();

        assert!(said.contains(&"console-notify.service stopped"), "{said:?}");
        assert_eq!(rows.get(1).map(|row| row.says.as_str()), Some("Console"));
    }

    #[test]
    fn the_way_back_is_the_first_row_of_a_notification() -> Result<(), NotFound> {
        let Ok(fault) = fault();
        let Ok(opened) = one(&fault);
        let Ok(cleared) = cleared();
        let first_tab = TABS.first().ok_or("there are no tabs")?;

        for rows in [opened, cleared] {
            let way_back = rows.first().ok_or("there are no rows")?;

            assert!(way_back.says.ends_with(first_tab), "{}", way_back.says);
            assert_eq!(way_back.acts(), Ok(Action::Yes));
        }

        Ok(())
    }

    #[test]
    fn a_notification_with_no_body_is_not_given_an_empty_line() {
        let Ok(ordinary) = ordinary();
        let Ok(rows) = one(&ordinary);

        assert!(!rows.iter().any(|row| row.says.is_empty() && row.aside.is_empty()));
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn a_notification_that_is_only_a_body_says_it_once() {
        let bodied = Notification { body: "the microphone is on".to_string(), ..Notification::default() };
        let Ok(rows) = one(&bodied);

        assert_eq!(rows.iter().filter(|row| row.aside == "the microphone is on").count(), 1);
    }

    #[test]
    fn a_notification_can_be_dismissed_where_it_is_read() {
        let Ok(fault) = fault();
        let Ok(rows) = one(&fault);

        assert!(rows.iter().filter(|row| matches!(row.acts(), Ok(Action::Yes))).any(|row| row.says == "Clear"));
    }

    #[test]
    fn what_was_cleared_is_read_where_it_stands() -> Result<(), NotFound> {
        let Ok(fault) = fault();
        let Ok(rows) = earlier_rows(&[fault]);
        let row = rows.first().ok_or("there are no rows")?;

        assert_eq!(rows.len(), 1);
        assert_eq!(row.acts(), Ok(Action::None));
        assert_eq!(row.says, "Notifications fell over");
        assert_eq!(row.aside, "console-notify.service stopped");

        Ok(())
    }

    #[test]
    fn an_empty_history_says_so_rather_than_drawing_nothing() -> Result<(), NotFound> {
        let Ok(rows) = earlier_rows(&[]);
        let row = rows.first().ok_or("there are no rows")?;

        assert_eq!(rows.len(), 1);
        assert_eq!(row.acts(), Ok(Action::None));

        Ok(())
    }
}

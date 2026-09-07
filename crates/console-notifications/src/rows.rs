//! What the panel holds, as a function of what mako said.
//!
//! Reading mako is one thing and knowing what to draw from it is another.
//! Everything here is the second, so the shape of both tabs can be asked
//! without a mako to ask.

use std::sync::Arc;

use console_core_never::Never;
use console_panel::page::{Does, Row, Showing};

use crate::reading::Notice;

pub const TABS: [&str; 2] = ["Waiting", "Earlier"];

pub fn tab(at: usize) -> Result<&'static str, Never> {
    Ok(TABS.get(at).copied().unwrap_or(""))
}

pub type Chosen = Arc<dyn Fn(&dyn Showing) + Send + Sync>;

pub fn aside(notice: &Notice) -> Result<String, Never> {
    let Ok(says) = notice.urgency.says();

    Ok(match says {
        "" => notice.app.clone(),
        said => said.to_string(),
    })
}

pub fn waiting_rows(
    held: &[Notice],
    open: impl Fn(&Notice) -> Does,
    clear: Does,
) -> Result<Vec<Row>, Never> {
    let mut rows: Vec<Row> = held
        .iter()
        .map(|notice| {
            let Ok(said) = aside(notice);
            let Ok(says) = notice.says();
            let Ok(row) = Row::new(&says, &said, open(notice));
            let Ok(opens) = row.opening();

            opens
        })
        .collect();

    match rows.is_empty() {
        true => {
            let Ok(row) = Row::nothing("Nothing is waiting");

            rows.push(row);
        }
        false => {
            let Ok(row) = Row::new("Clear them all", "", clear);

            rows.push(row);
        }
    }

    Ok(rows)
}

pub fn one_rows(notice: &Notice, back: &Chosen, dismiss: Does) -> Result<Vec<Row>, Never> {
    let going = Arc::clone(back);
    let Ok(first) = tab(0);
    let Ok(way_back) = Row::back(first, move |showing| going(showing));
    let Ok(by) = said_by(notice);
    let Ok(says) = notice.says();
    let Ok(heading) = Row::said(&by, &says);
    let mut rows = vec![way_back, heading];

    let worth_a_row = !notice.body.trim().is_empty() && notice.body.trim() != says;

    match worth_a_row {
        true => {
            let Ok(body) = Row::said("", notice.body.trim());

            rows.push(body);
        }
        false => {}
    }

    let Ok(row) = Row::new("Dismiss", "", dismiss);

    rows.push(row);

    Ok(rows)
}

fn said_by(notice: &Notice) -> Result<String, Never> {
    Ok(match notice.app.trim().is_empty() {
        true => "Said".to_string(),
        false => notice.app.trim().to_string(),
    })
}

pub fn gone_rows(back: &Chosen) -> Result<Vec<Row>, Never> {
    let going = Arc::clone(back);
    let Ok(first) = tab(0);
    let Ok(way_back) = Row::back(first, move |showing| going(showing));
    let Ok(gone) = Row::nothing("It has gone");

    Ok(vec![way_back, gone])
}

pub fn earlier_rows(held: &[Notice]) -> Result<Vec<Row>, Never> {
    match held.is_empty() {
        true => {
            let Ok(row) = Row::nothing("Nothing has been cleared yet");

            return Ok(vec![row]);
        }
        false => {}
    }

    Ok(held
        .iter()
        .map(|notice| {
            let Ok(said) = earlier_aside(notice);
            let Ok(says) = notice.says();
            let Ok(row) = Row::said(&says, &said);

            row
        })
        .collect())
}

fn earlier_aside(notice: &Notice) -> Result<String, Never> {
    Ok(match notice.body.trim().is_empty() {
        true => {
            let Ok(said) = aside(notice);

            said
        }
        false => notice.body.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use console_panel::page::Acts;
    use super::*;
    use crate::reading::Urgency;

    fn nothing() -> Does {
        let Ok(does) = Does::and_stay(|_| ());

        does
    }

    fn opening(_: &Notice) -> Does {
        nothing()
    }

    fn back() -> Chosen {
        Arc::new(|_: &dyn Showing| ())
    }

    fn waiting(held: &[Notice]) -> Vec<Row> {
        let Ok(rows) = waiting_rows(held, opening, nothing());

        rows
    }

    fn one(notice: &Notice) -> Vec<Row> {
        let Ok(rows) = one_rows(notice, &back(), nothing());

        rows
    }

    fn gone() -> Vec<Row> {
        let Ok(rows) = gone_rows(&back());

        rows
    }

    fn earlier(held: &[Notice]) -> Vec<Row> {
        let Ok(rows) = earlier_rows(held);

        rows
    }

    fn acts(row: &Row) -> Acts {
        let Ok(acts) = row.acts();

        acts
    }

    fn beside(notice: &Notice) -> String {
        let Ok(said) = aside(notice);

        said
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

    #[test]
    fn the_first_row_that_does_anything_is_a_notification() {
        let rows = waiting(&[fault()]);
        let first =
            rows.iter().position(|row| acts(row) == Acts::Yes).expect("a row that acts");
        assert_eq!(rows[first].says, "Notifications fell over");
    }

    #[test]
    fn a_notification_says_that_it_opens() {
        let rows = waiting(&[fault()]);
        assert!(rows[0].opens);
    }

    #[test]
    fn a_fault_is_marked_and_an_ordinary_notification_is_named_by_its_app() {
        assert_eq!(beside(&fault()), "wrong");
        assert_eq!(beside(&ordinary()), "Librewolf");
    }

    #[test]
    fn there_is_nothing_to_clear_when_nothing_is_waiting() {
        let rows = waiting(&[]);
        assert!(!rows.iter().any(|row| row.says == "Clear them all"));
        assert_eq!(rows[0].says, "Nothing is waiting");
    }

    #[test]
    fn what_is_waiting_can_be_cleared_in_one_press() {
        let rows = waiting(&[fault(), ordinary()]);
        assert!(rows.iter().any(|row| row.says == "Clear them all" && acts(row) == Acts::Yes));
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
        for rows in [one(&fault()), gone()] {
            assert!(rows[0].says.ends_with(TABS[0]), "{}", rows[0].says);
            assert_eq!(acts(&rows[0]), Acts::Yes);
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
        let bodied = Notice { body: "the microphone is on".to_string(), ..Notice::default() };
        let rows = one(&bodied);
        let times = rows.iter().filter(|row| row.aside == "the microphone is on").count();
        assert_eq!(times, 1);
    }

    #[test]
    fn a_notification_can_be_dismissed_where_it_is_read() {
        let rows = one(&fault());
        assert!(rows.iter().any(|row| row.says == "Dismiss" && acts(row) == Acts::Yes));
    }

    #[test]
    fn what_was_cleared_is_read_where_it_stands() {
        let rows = earlier(&[fault()]);
        assert_eq!(rows.len(), 1);
        assert_eq!(acts(&rows[0]), Acts::Nothing);
        assert_eq!(rows[0].says, "Notifications fell over");
        assert_eq!(rows[0].aside, "console-notify.service stopped");
    }

    #[test]
    fn an_empty_history_says_so_rather_than_drawing_nothing() {
        assert_eq!(earlier(&[]).len(), 1);
        assert_eq!(acts(&earlier(&[])[0]), Acts::Nothing);
    }
}

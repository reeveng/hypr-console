//! What a tab is made of, once something has been looked for.
//!
//! The panel puts a line to type into at the top of each tab and hands back
//! whatever is in it; these are the rows under it. Subject of them is there
//! depends on three things and no more: what is being typed, what the last
//! search was for, and whether a search is out.
//!
//! The searching is not done as the letters arrive. Every letter would be a
//! question to a site, nine of the ten answers thrown away before they landed,
//! and a list moving under a thumb that is still typing. So the word is taken
//! and the row under the line asks for it, which is one press of A on the row
//! the d-pad walks onto anyway.

use console_core_external_programs::Program;
use console_core_never::Never;
use console_panel::page::{Aside, Handler, Row, Showing, YET};

use crate::looking::{Found, Looked};
use crate::store::Kind;

pub const ABOUT: &str = "Search for a song or video";

pub const ABOUT_BOOKS: &str = "Search Free Books";

pub const ONLY_FREE_BOOKS: &str = "Only books old enough to be free are here, from Standard Ebooks and Project Gutenberg";

pub fn about(kind: Kind) -> Result<&'static str, Never> {
    Ok(match kind {
        Kind::Sound | Kind::Film => ABOUT,
        Kind::Book => ABOUT_BOOKS,
    })
}

pub const LOOK_FOR: &str = "Search for";
pub const LOOKING: &str = "Searching for";

pub const NOTHING_YET: &str = "No Recent Searches";
pub const NOTHING_CAME_BACK: &str = "No Results for";

pub const IN_A_BROWSER: &str = "Open in Browser";

pub const LINE: u32 = 1;

pub const WAYS_START: u32 = 1;

pub fn rows(
    typed: &str,
    asking: Option<&str>,
    looked: &Looked,
    look: Handler,
    each: &dyn Fn(u32, &Found) -> Row,
) -> Result<Vec<Row>, Never> {
    let mut rows: Vec<Row> = Vec::new();
    let word = typed.trim();

    match asking {
        Some(out) => {
            let Ok(row) = Row::said(&format!("{LOOKING} {out}"), Aside(YET));

            rows.push(row);
        }
        None => match !word.is_empty() && word != looked.asked {
            true => {
                let Ok(row) = Row::new(&format!("{LOOK_FOR} {word}"), Aside(""), look);

                rows.push(row);
            }
            false => {}
        },
    }

    match looked.found.is_empty() {
        true => {},
        false => {
            let Ok(naming) = Row::naming(&looked.asked, Aside(""));

            rows.push(naming);

            for found in &looked.found {
                let Ok(many) = console_core_number_conversion::fitted::<_, u32>(rows.len());

                rows.push(each(many.saturating_add(LINE), found));
            }

            return Ok(rows);
        }
    }

    match (looked.fault.is_empty(), looked.asked.is_empty(), rows.is_empty()) {
        (false, _, _) => {
            let Ok(row) = Row::nothing(&looked.fault);

            rows.push(row);
        }
        (true, false, _) => {
            let Ok(row) = Row::nothing(&format!("{NOTHING_CAME_BACK} {}", looked.asked));

            rows.push(row);
        }
        (true, true, true) => {
            let Ok(row) = Row::nothing(NOTHING_YET);

            rows.push(row);
        }
        (true, true, false) => {},
    }

    Ok(rows)
}

pub fn noted(kind: Kind, rows: Vec<Row>) -> Result<Vec<Row>, Never> {
    let mut rows = rows;

    match kind {
        Kind::Sound | Kind::Film => {},
        Kind::Book => {
            let Ok(note) = Row::nothing(ONLY_FREE_BOOKS);

            rows.push(note);
        },
    }

    Ok(rows)
}

pub fn ways(
    found: &Found,
    back: impl Fn(&dyn Showing) + Send + Sync + 'static,
    other: Option<(Kind, Handler)>,
) -> Result<Vec<Row>, Never> {
    let Ok(way_back) = Row::back(&found.title, back);
    let Ok(in_a_browser) = Program::XdgOpen.name();
    let Ok(opens) = Handler::run(&[in_a_browser, &found.url]);
    let Ok(browser) = Row::new(IN_A_BROWSER, Aside(""), opens);

    let getting = match other {
        Some((other, get)) => {
            let Ok(as_well) = as_well(other);

            as_well.map(|as_well| Row::new(as_well, Aside(""), get))
        },
        None => None,
    };

    Ok(match getting {
        Some(Ok(getting)) => vec![way_back, getting, browser],
        None => vec![way_back, browser],
    })
}

pub fn as_well(other: Kind) -> Result<Option<&'static str>, Never> {
    Ok(match other {
        Kind::Sound => Some("Download Audio Too"),
        Kind::Film => Some("Download Video Too"),
        Kind::Book => None,
    })
}

#[cfg(test)]
mod tests {
    use console_panel::page::{Action, Heading};
    use super::*;

    fn found() -> Vec<Found> {
        vec![Found {
            id: "FTQbiNvZqaY".to_string(),
            title: "Toto - Africa".to_string(),
            ..Found::default()
        }]
    }

    fn looked(asked: &str) -> Looked {
        Looked { asked: asked.to_string(), fault: String::new(), found: found() }
    }

    fn nothing() -> Handler {
        let Ok(does) = Handler::and_stay(|_| {});

        does
    }

    fn plain(at: u32, found: &Found) -> Row {
        let Ok(row) = Row::said(&found.title, Aside(&at.to_string()));

        row
    }

    fn acts(row: &Row) -> Action {
        let Ok(acts) = row.acts();

        acts
    }

    fn heading(row: &Row) -> Heading {
        let Ok(heading) = row.heading();

        heading
    }

    fn said(rows: &[Row]) -> Vec<String> {
        rows.iter().map(|row| row.says.clone()).collect()
    }

    fn rows(
        typed: &str,
        asking: Option<&str>,
        looked: &Looked,
        look: Handler,
        each: &dyn Fn(u32, &Found) -> Row,
    ) -> Vec<Row> {
        let Ok(rows) = super::rows(typed, asking, looked, look, each);

        rows
    }

    fn ways(
        found: &Found,
        other: Kind,
        back: impl Fn(&dyn Showing) + Send + Sync + 'static,
        get: Handler,
    ) -> Vec<Row> {
        let Ok(ways) = super::ways(found, back, Some((other, get)));

        ways
    }

    fn as_well(other: Kind) -> Option<&'static str> {
        let Ok(as_well) = super::as_well(other);

        as_well
    }

    #[test]
    fn a_tab_nothing_has_been_typed_into_says_so() {
        let rows = rows("", None, &Looked::default(), nothing(), &plain);
        assert_eq!(said(&rows), [NOTHING_YET]);
    }

    #[test]
    fn a_word_that_has_not_been_looked_for_puts_the_row_that_looks_for_it_first()
    {
        let rows = rows("africa", None, &Looked::default(), nothing(), &plain);
        assert_eq!(rows[0].says, "Search for africa");
        assert_eq!(acts(&rows[0]), Action::Yes);
    }

    #[test]
    fn a_word_that_has_already_been_looked_for_asks_for_nothing() {
        let rows = rows("africa", None, &looked("africa"), nothing(), &plain);
        assert_eq!(said(&rows), ["africa", "Toto - Africa"]);
        assert_eq!(heading(&rows[0]), Heading::Yes, "what a list is about is not one of its rows");
    }

    #[test]
    fn while_a_search_is_out_the_row_says_so_and_the_last_one_stays_up() {
        let rows = rows("africa", Some("africa"), &looked("toto"), nothing(), &plain);
        assert_eq!(rows[0].says, "Searching for africa");
        assert_eq!(rows[0].aside, YET);
        assert!(said(&rows).contains(&"Toto - Africa".to_string()));
    }

    #[test]
    fn a_thing_is_told_which_row_it_will_be_drawn_on() {
        let rows = rows("", None, &looked("africa"), nothing(), &plain);
        assert_eq!(rows[1].aside, (1 + LINE).to_string());
    }

    #[test]
    fn a_search_that_answered_nothing_says_which_word_it_was_about() {
        let empty = Looked { asked: "asdfgh".to_string(), ..Looked::default() };
        let rows = rows("asdfgh", None, &empty, nothing(), &plain);
        assert_eq!(said(&rows), [format!("{NOTHING_CAME_BACK} asdfgh")]);
    }

    #[test]
    fn a_book_tab_says_that_newer_books_are_not_there_and_the_others_do_not() {
        let Ok(books) = noted(Kind::Book, rows("", None, &looked("meditations"), nothing(), &plain));
        let Ok(songs) = noted(Kind::Sound, rows("", None, &looked("africa"), nothing(), &plain));

        assert_eq!(books.last().map(|row| row.says.as_str()), Some(ONLY_FREE_BOOKS));
        assert!(books.last().is_some_and(|row| row.nothing), "the note is read, not pressed");
        assert!(!said(&songs).contains(&ONLY_FREE_BOOKS.to_string()));
    }

    #[test]
    fn what_went_wrong_is_a_row_like_anything_else() {
        let fault = Looked {
            asked: "africa".to_string(),
            fault: "There is no yt-dlp on this machine".to_string(),
            found: Vec::new(),
        };
        let rows = rows("africa", None, &fault, nothing(), &plain);
        assert_eq!(rows[0].says, "There is no yt-dlp on this machine");
    }

    #[test]
    fn y_offers_the_other_kind_of_the_same_thing_and_the_way_back() {
        let one = found()[0].clone();
        let rows = ways(&one, Kind::Sound, |_| {}, nothing());
        assert!(rows[0].says.ends_with(&one.title), "row zero is the way back");
        assert_eq!(rows.get(console_core_number_conversion::index(WAYS_START).unwrap()).unwrap().says, as_well(Kind::Sound).unwrap());
        assert_eq!(rows[2].says, IN_A_BROWSER);
    }
}

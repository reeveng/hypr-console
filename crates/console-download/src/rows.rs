//! What a tab is made of, once something has been looked for.
//!
//! The panel puts a line to type into at the top of each tab and hands back
//! whatever is in it; these are the rows under it. Which of them is there
//! depends on three things and no more: what is being typed, what the last
//! search was for, and whether a search is out.
//!
//! The searching is not done as the letters arrive. Every letter would be a
//! question to a site, nine of the ten answers thrown away before they landed,
//! and a list moving under a thumb that is still typing. So the word is taken
//! and the row under the line asks for it, which is one press of A on the row
//! the d-pad walks onto anyway.

use console_panel::page::{Does, Row, Showing, YET};

use crate::looking::{Found, Looked};
use crate::store::Kind;

/// What the line at the top of each tab says while nothing has been typed.
pub const ABOUT: &str = "Type what you are after, then take the row under it";

/// The row that asks, and the row that says it is being asked.
pub const LOOK_FOR: &str = "Look for";
pub const LOOKING: &str = "Looking for";

/// What a tab says when it has been given nothing to say.
pub const NOTHING_YET: &str = "Nothing has been looked for yet";
pub const NOTHING_CAME_BACK: &str = "Nothing came back for";

/// What Y's list calls fetching the other kind of the same thing.
pub const IN_A_BROWSER: &str = "Watch it in the browser";

/// The panel's own line stands above these, so a row this list calls its first
/// is the second thing on the screen.
pub const LINE: usize = 1;

/// Where the highlight lands in Y's list: past the way back, on the first thing
/// that can be done.
pub const WAYS_START: usize = 1;

/// The rows of one tab.
///
/// `asking` is the word a search is out for, while one is. `each` is handed the
/// row number a thing will stand on, because what Y does with it has to know
/// where to put the highlight back when it is left.
pub fn rows(
    typed: &str,
    asking: Option<&str>,
    looked: &Looked,
    look: Does,
    each: &dyn Fn(usize, &Found) -> Row,
) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    let word = typed.trim();
    match asking {
        // What is on the screen is the last search's, and the row says what the
        // next one will be about rather than emptying the list to say it.
        Some(out) => rows.push(Row::said(&format!("{LOOKING} {out}"), YET)),
        None if !word.is_empty() && word != looked.asked => {
            rows.push(Row::new(&format!("{LOOK_FOR} {word}"), "", look));
        }
        None => {}
    }
    if !looked.found.is_empty() {
        // What the rows under it are about, which is not one of them. Without
        // it a list found by one word is indistinguishable from a list found by
        // the word before it, which is what the line above is now saying.
        rows.push(Row::naming(&looked.asked, ""));
        for found in &looked.found {
            let at = rows.len() + LINE;
            rows.push(each(at, found));
        }
        return rows;
    }
    if !looked.fault.is_empty() {
        rows.push(Row::nothing(&looked.fault));
    } else if !looked.asked.is_empty() {
        rows.push(Row::nothing(&format!("{NOTHING_CAME_BACK} {}", looked.asked)));
    } else if rows.is_empty() {
        rows.push(Row::nothing(NOTHING_YET));
    }
    rows
}

/// What else can be done with one thing, which is Y's list.
///
/// Two things, and both of them are about the row it was pressed on: the same
/// thing in the other kind, and the thing itself on the site it came from. A
/// person in the Video tab who wanted the song is one press from it rather than
/// one shoulder, one search and one press.
pub fn ways(
    found: &Found,
    other: Kind,
    back: impl Fn(&dyn Showing) + Send + Sync + 'static,
    get: Does,
) -> Vec<Row> {
    vec![
        Row::back(&found.title, back),
        Row::new(as_well(other), "", get),
        Row::new(IN_A_BROWSER, "", Does::run(&["xdg-open", &found.url])),
    ]
}

/// What fetching the other kind is called, said as where it lands.
pub fn as_well(other: Kind) -> &'static str {
    match other {
        Kind::Sound => "Get the sound of it as well",
        Kind::Film => "Get the whole video as well",
    }
}

#[cfg(test)]
mod tests {
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

    fn nothing() -> Does {
        Does::and_stay(|_| {})
    }

    fn plain(at: usize, found: &Found) -> Row {
        Row::said(&found.title, &at.to_string())
    }

    fn said(rows: &[Row]) -> Vec<String> {
        rows.iter().map(|row| row.says.clone()).collect()
    }

    #[test]
    fn a_tab_nothing_has_been_typed_into_says_so() {
        let rows = rows("", None, &Looked::default(), nothing(), &plain);
        assert_eq!(said(&rows), [NOTHING_YET]);
    }

    /// One press of A on the row the d-pad walks onto anyway, rather than a
    /// question to a site for every letter.
    #[test]
    fn a_word_that_has_not_been_looked_for_puts_the_row_that_looks_for_it_first()
    {
        let rows = rows("africa", None, &Looked::default(), nothing(), &plain);
        assert_eq!(rows[0].says, "Look for africa");
        assert!(rows[0].acts());
    }

    #[test]
    fn a_word_that_has_already_been_looked_for_asks_for_nothing() {
        let rows = rows("africa", None, &looked("africa"), nothing(), &plain);
        assert_eq!(said(&rows), ["africa", "Toto - Africa"]);
        assert!(rows[0].heading(), "what a list is about is not one of its rows");
    }

    /// The list that is there stays on the screen while the next one is being
    /// fetched, because a tab that empties itself to say it is working is a tab
    /// that has thrown away the thing somebody was reading.
    #[test]
    fn while_a_search_is_out_the_row_says_so_and_the_last_one_stays_up() {
        let rows = rows("africa", Some("africa"), &looked("toto"), nothing(), &plain);
        assert_eq!(rows[0].says, "Looking for africa");
        assert_eq!(rows[0].aside, YET);
        assert!(said(&rows).contains(&"Toto - Africa".to_string()));
    }

    /// A row is handed where it stands, so that what Y opens can put the
    /// highlight back on the thing it was about.
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

    /// A search that answers nothing and says nothing is a panel that looks
    /// broken.
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
        assert!(rows[0].says.ends_with(&one.title), "row nought is the way back");
        assert_eq!(rows[WAYS_START].says, as_well(Kind::Sound));
        assert_eq!(rows[2].says, IN_A_BROWSER);
    }
}

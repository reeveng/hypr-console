//! The lines under a heading in brackets, which is the shape of every
//! configuration file this desktop reads.
//!
//! `.desktop` files and `desktop.conf` are the same format wearing two names:
//! a heading in square brackets, the lines that follow it, and the next
//! heading ending it. Four places read it and three of them had written the
//! walk themselves, each with a mutable flag saying whether the line being
//! looked at was inside the part that was wanted. They did not agree. One
//! required its heading, one took whatever group came first and could not say
//! that it had, and one answered a question about `[services]` by walking the
//! manifest rather than by asking the crate that reads it.
//!
//! So the walk is written once here and the disagreements are settled:
//!
//!   - a heading that is never found means no lines, not the whole file;
//!   - a blank line and a line opening with `#` are not content;
//!   - space either side of the `=` belongs to neither the key nor the value,
//!     which is what the desktop entry specification says and what only one of
//!     the three did;
//!   - the first value given for a key is the one kept, because a file that
//!     says a thing twice was written by somebody who thought they were saying
//!     it once, and the reading that quietly prefers the last is the one that
//!     is hard to see.
//!
//! `lines` is the walk and the other two are readings of it: `fields` for a
//! group whose lines are `key=value`, `field` for one key on its own, which
//! finds it without building a map to throw away. Asking for several keys one
//! at a time walks the group once each time, so a caller reading a handful of
//! them wants `fields`.
//!
//! `headings` is the file read from the other end, for a reader that does not
//! know what it will find: `words.conf` keeps a vocabulary under headings that
//! are the argument for the words beneath them, so the headings themselves are
//! content rather than a key somebody already has. It is the same walk as
//! `lines` looked at one line at a time, which is why it is here and not a
//! second reading of the format somewhere else.
//!
//! Nothing is written back. `console-manifest-engine` keeps its own fold over
//! `desktop.conf`, which reports where a fault is and refuses a heading it does
//! not know, and borrows `heading` and `without_a_comment` from here so what a
//! heading and what a comment are stay one answer each.
//!
//! `without_a_comment` is the trailing half of the same question and is not
//! applied by `lines`, which only drops a line that opens with one. That is
//! deliberate and it is about values rather than about comments: a `.desktop`
//! `Exec=` carries a `#` in a URL and a stylesheet carries one in front of
//! every colour, so a reader that stripped from the first `#` on every line
//! would quietly shorten both. `desktop.conf` and `machines.conf` are this
//! desktop's own and do put comments after a value, so they ask for it by name.

use std::collections::BTreeMap;

use console_core_never::Never;

pub fn heading(line: &str) -> Result<Option<&str>, Never> {
    Ok(line.trim().strip_prefix('[').and_then(|rest| rest.strip_suffix(']')))
}

pub fn without_a_comment(line: &str) -> Result<&str, Never> {
    Ok(match line.split_once(COMMENT) {
        Some((said, _the_rest_is_for_a_reader)) => said.trim(),
        None => line.trim(),
    })
}

pub const COMMENT: char = '#';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Under<'a>(pub &'a str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key<'a>(pub &'a str);

pub fn headings(said: &str) -> Result<Vec<&str>, Never> {
    Ok(said
        .lines()
        .filter_map(|line| {
            let Ok(found) = heading(line);

            found
        })
        .collect())
}

pub fn lines<'a>(said: &'a str, under: Under<'_>) -> Result<Vec<&'a str>, Never> {
    Ok(said
        .lines()
        .map(str::trim)
        .skip_while(|line| {
            let Ok(heading) = heading(line);

            heading != Some(under.0)
        })
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect())
}

pub fn fields<'a>(said: &'a str, under: Under<'_>) -> Result<BTreeMap<&'a str, &'a str>, Never> {
    let lines = lines(said, under)?;

    Ok(lines
        .into_iter()
        .filter_map(|line| line.split_once('='))
        .fold(BTreeMap::new(), |mut found, (key, value)| {
            let _ = found.entry(key.trim()).or_insert(value.trim());

            found
        }))
}

pub fn field<'a>(said: &'a str, under: Under<'_>, key: Key<'_>) -> Result<Option<&'a str>, Never> {
    let lines = lines(said, under)?;

    Ok(lines
        .into_iter()
        .filter_map(|line| line.split_once('='))
        .find_map(|(found, value)| (found.trim() == key.0).then_some(value.trim())))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAID: &str = "\
# a file that says what it is

[Desktop Entry]
Type=Application
Name=Firefox
Exec=firefox %u

[Desktop Action new-window]
Name=New Window
";

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    #[test]
    fn every_heading_is_found_in_the_order_the_file_puts_them_in() {
        assert_eq!(ok(headings(SAID)), vec!["Desktop Entry", "Desktop Action new-window"]);
    }

    #[test]
    fn a_file_with_no_headings_has_none() {
        assert_eq!(ok(headings("one\ntwo\n")), Vec::<&str>::new());
    }

    #[test]
    fn a_comment_after_a_value_is_not_part_of_the_value() {
        assert_eq!(ok(without_a_comment("pamac-aur  # manjaro's")), "pamac-aur");
    }

    #[test]
    fn a_line_with_nothing_on_it_but_a_comment_says_nothing() {
        assert_eq!(ok(without_a_comment("  # what this block is for")), "");
    }

    #[test]
    fn a_line_with_no_comment_on_it_is_itself() {
        assert_eq!(ok(without_a_comment("  freetube  ")), "freetube");
    }

    fn said<'a>(said: &'a str, under: &str) -> BTreeMap<&'a str, &'a str> {
        ok(fields(said, Under(under)))
    }

    #[test]
    fn a_heading_is_a_name_in_brackets() {
        assert_eq!(ok(heading("[services]")), Some("services"));
        assert_eq!(ok(heading("  [Desktop Entry]  ")), Some("Desktop Entry"));
        assert_eq!(ok(heading("Name=Firefox")), None);
        assert_eq!(ok(heading("[unclosed")), None);
    }

    #[test]
    fn a_group_is_the_lines_under_its_heading() {
        assert_eq!(ok(lines(SAID, Under("Desktop Entry"))), vec![
            "Type=Application",
            "Name=Firefox",
            "Exec=firefox %u"
        ]);
    }

    #[test]
    fn the_next_heading_ends_it() {
        assert_eq!(ok(lines(SAID, Under("Desktop Action new-window"))), vec!["Name=New Window"]);
        assert_eq!(said(SAID, "Desktop Entry").get("Name"), Some(&"Firefox"));
    }

    #[test]
    fn a_heading_that_is_not_there_holds_nothing() {
        assert_eq!(ok(lines(SAID, Under("Sound"))), Vec::<&str>::new());
        assert_eq!(said(SAID, "Sound"), BTreeMap::new());
    }

    #[test]
    fn what_is_said_before_the_first_heading_belongs_to_no_heading() {
        assert_eq!(said("Name=Nobody\n[Desktop Entry]\nName=Firefox\n", "Desktop Entry"), said(
            "[Desktop Entry]\nName=Firefox\n",
            "Desktop Entry"
        ));
    }

    #[test]
    fn a_comment_and_a_blank_line_are_not_content() {
        let held = "[Desktop Entry]\n\n# Name=Commented\nName=Firefox\n";

        assert_eq!(ok(lines(held, Under("Desktop Entry"))), vec!["Name=Firefox"]);
        assert_eq!(said(held, "Desktop Entry").get("# Name"), None);
    }

    #[test]
    fn space_either_side_of_the_equals_belongs_to_neither() {
        let held = "[Desktop Entry]\nName = Firefox \n";

        assert_eq!(said(held, "Desktop Entry").get("Name"), Some(&"Firefox"));
    }

    #[test]
    fn the_first_value_given_for_a_key_is_the_one_kept() {
        let held = "[Desktop Entry]\nName=First\nName=Second\n";

        assert_eq!(said(held, "Desktop Entry").get("Name"), Some(&"First"));
    }

    #[test]
    fn a_value_may_hold_anything_including_the_marks_this_reads_by() {
        let held = "[Desktop Entry]\nExec=sh -c 'echo [x] # y=z'\n";

        assert_eq!(said(held, "Desktop Entry").get("Exec"), Some(&"sh -c 'echo [x] # y=z'"));
    }

    #[test]
    fn one_key_can_be_asked_for_without_the_rest() {
        assert_eq!(ok(field(SAID, Under("Desktop Entry"), Key("Name"))), Some("Firefox"));
        assert_eq!(ok(field(SAID, Under("Desktop Entry"), Key("Nothing"))), None);
        assert_eq!(ok(field(SAID, Under("Sound"), Key("Name"))), None);
    }

    #[test]
    fn one_key_asked_for_alone_is_the_same_key_the_map_holds() {
        let held = "[Desktop Entry]\nName = First \nName=Second\n";

        assert_eq!(ok(field(held, Under("Desktop Entry"), Key("Name"))), Some("First"));
        assert_eq!(
            said(held, "Desktop Entry").get("Name").copied(),
            ok(field(held, Under("Desktop Entry"), Key("Name")))
        );
    }

    #[test]
    fn a_line_with_no_equals_is_a_line_rather_than_a_field() {
        let held = "[services]\nconsole.target\nhypridle.service\n";

        assert_eq!(ok(lines(held, Under("services"))), vec!["console.target", "hypridle.service"]);
        assert_eq!(said(held, "services"), BTreeMap::new());
    }
}

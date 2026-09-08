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
//! Nothing is written back. `console-manifest-engine` keeps its own fold over
//! `desktop.conf`, which reports where a fault is and refuses a heading it does
//! not know, and borrows `heading` from here so what a heading is stays one
//! answer.

use std::collections::BTreeMap;

use console_core_never::Never;

pub fn heading(line: &str) -> Result<Option<&str>, Never> {
    Ok(line.trim().strip_prefix('[').and_then(|rest| rest.strip_suffix(']')))
}

pub fn lines<'a>(said: &'a str, under: &str) -> Result<Vec<&'a str>, Never> {
    Ok(said
        .lines()
        .map(str::trim)
        .skip_while(|line| {
            let Ok(heading) = heading(line);

            heading != Some(under)
        })
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect())
}

pub fn fields<'a>(said: &'a str, under: &str) -> Result<BTreeMap<&'a str, &'a str>, Never> {
    let lines = lines(said, under)?;

    Ok(lines
        .into_iter()
        .filter_map(|line| line.split_once('='))
        .fold(BTreeMap::new(), |mut found, (key, value)| {
            let _ = found.entry(key.trim()).or_insert(value.trim());

            found
        }))
}

pub fn field<'a>(said: &'a str, under: &str, key: &str) -> Result<Option<&'a str>, Never> {
    let lines = lines(said, under)?;

    Ok(lines
        .into_iter()
        .filter_map(|line| line.split_once('='))
        .find_map(|(found, value)| (found.trim() == key).then_some(value.trim())))
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

    fn said<'a>(said: &'a str, under: &str) -> BTreeMap<&'a str, &'a str> {
        ok(fields(said, under))
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
        assert_eq!(ok(lines(SAID, "Desktop Entry")), vec![
            "Type=Application",
            "Name=Firefox",
            "Exec=firefox %u"
        ]);
    }

    #[test]
    fn the_next_heading_ends_it() {
        assert_eq!(ok(lines(SAID, "Desktop Action new-window")), vec!["Name=New Window"]);
        assert_eq!(said(SAID, "Desktop Entry").get("Name"), Some(&"Firefox"));
    }

    #[test]
    fn a_heading_that_is_not_there_holds_nothing() {
        assert_eq!(ok(lines(SAID, "Sound")), Vec::<&str>::new());
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

        assert_eq!(ok(lines(held, "Desktop Entry")), vec!["Name=Firefox"]);
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
        assert_eq!(ok(field(SAID, "Desktop Entry", "Name")), Some("Firefox"));
        assert_eq!(ok(field(SAID, "Desktop Entry", "Nothing")), None);
        assert_eq!(ok(field(SAID, "Sound", "Name")), None);
    }

    #[test]
    fn one_key_asked_for_alone_is_the_same_key_the_map_holds() {
        let held = "[Desktop Entry]\nName = First \nName=Second\n";

        assert_eq!(ok(field(held, "Desktop Entry", "Name")), Some("First"));
        assert_eq!(said(held, "Desktop Entry").get("Name").copied(), ok(field(held, "Desktop Entry", "Name")));
    }

    #[test]
    fn a_line_with_no_equals_is_a_line_rather_than_a_field() {
        let held = "[services]\nconsole.target\nhypridle.service\n";

        assert_eq!(ok(lines(held, "services")), vec!["console.target", "hypridle.service"]);
        assert_eq!(said(held, "services"), BTreeMap::new());
    }
}

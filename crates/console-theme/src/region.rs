//! Writing into a file somebody else owns.
//!
//! Four files on this machine cannot import a palette: KDE's ini format has no
//! include and neither has mako's, and the browser's `user.js` and a systemd
//! unit are both lists of literals. A fifth, the compositor's Lua, could but
//! must not, because a Lua file that fails to load takes the session with it.
//! Those five have a pair of markers in them and only what lies between the
//! markers is ours.

pub const BEGIN: &str = "console-theme:begin";
pub const END: &str = "console-theme:end";

/// What the file should hold, with `body` between its markers.
///
/// Takes the file's current text rather than its path, so that the splice can
/// be tested without a file existing anywhere.
pub fn spliced(held: &str, body: &str) -> Option<String> {
    let lines: Vec<&str> = held.split_inclusive('\n').collect();
    let only = |needle: &str| match lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .map(|(at, _)| at)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [at] => Some(*at),
        _ => None,
    };
    match (only(BEGIN), only(END)) {
        (Some(begin), Some(end)) if begin < end => Some(format!(
            "{}{}\n{}",
            lines[..=begin].concat(),
            body.trim_end_matches('\n'),
            lines[end..].concat()
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELD: &str = "before\n# console-theme:begin\nold\nlines\n# console-theme:end\nafter\n";

    #[test]
    fn what_lies_between_the_markers_is_replaced() {
        let got = spliced(HELD, "new").expect("one pair of markers");
        assert_eq!(
            got,
            "before\n# console-theme:begin\nnew\n# console-theme:end\nafter\n"
        );
    }

    #[test]
    fn what_lies_outside_them_is_left_alone() {
        let got = spliced(HELD, "new").expect("one pair of markers");
        assert!(got.starts_with("before\n"), "{got}");
        assert!(got.ends_with("after\n"), "{got}");
    }

    #[test]
    fn a_body_is_written_with_exactly_one_newline_after_it() {
        for body in ["new", "new\n", "new\n\n\n"] {
            let got = spliced(HELD, body).expect("one pair of markers");
            assert!(got.contains("new\n# console-theme:end"), "{body:?} gave {got:?}");
        }
    }

    #[test]
    fn an_empty_region_is_filled() {
        let held = "# console-theme:begin\n# console-theme:end\n";
        assert_eq!(
            spliced(held, "new").expect("one pair"),
            "# console-theme:begin\nnew\n# console-theme:end\n"
        );
    }

    #[test]
    fn a_file_with_no_markers_is_refused_rather_than_appended_to() {
        assert_eq!(spliced("nothing here\n", "new"), None);
    }

    #[test]
    fn a_second_pair_of_markers_is_refused_rather_than_guessed_at() {
        let twice = format!("{HELD}{HELD}");
        assert_eq!(spliced(&twice, "new"), None);
    }

    #[test]
    fn markers_in_the_wrong_order_are_refused() {
        let backwards = "# console-theme:end\nbody\n# console-theme:begin\n";
        assert_eq!(spliced(backwards, "new"), None);
    }

    #[test]
    fn a_half_marked_file_is_refused() {
        assert_eq!(spliced("# console-theme:begin\nbody\n", "new"), None);
        assert_eq!(spliced("# console-theme:end\nbody\n", "new"), None);
    }
}

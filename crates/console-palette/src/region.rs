//! Writing into a file somebody else owns.
//!
//! Four files on this machine cannot import a palette: KDE's ini format has no
//! include and neither has mako's, and the browser's `user.js` and a systemd
//! unit are both lists of literals. A fifth, the compositor's Lua, could but
//! must not, because a Lua file that fails to load takes the session with it.
//! Those five have a pair of markers in them and only what lies between the
//! markers is ours.

use console_core_never::Never;

pub const BEGIN: &str = "console-palette:begin";
pub const END: &str = "console-palette:end";

pub fn spliced(held: &str, body: &str) -> Result<Option<String>, Never> {
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

    let (Some(begin), Some(end)) = (only(BEGIN), only(END)) else {
        return Ok(None);
    };

    match begin < end {
        true => {
            let (Some(head), Some(tail)) = (lines.get(..=begin), lines.get(end..)) else {
                return Ok(None);
            };

            Ok(Some(format!(
                "{}{}\n{}",
                head.concat(),
                body.trim_end_matches('\n'),
                tail.concat()
            )))
        }
        false => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    const HELD: &str = "before\n# console-palette:begin\nold\nlines\n# console-palette:end\nafter\n";

    #[test]
    fn what_lies_between_the_markers_is_replaced() {
        let got = ok(spliced(HELD, "new")).expect("one pair of markers");
        assert_eq!(
            got,
            "before\n# console-palette:begin\nnew\n# console-palette:end\nafter\n"
        );
    }

    #[test]
    fn what_lies_outside_them_is_left_alone() {
        let got = ok(spliced(HELD, "new")).expect("one pair of markers");
        assert!(got.starts_with("before\n"), "{got}");
        assert!(got.ends_with("after\n"), "{got}");
    }

    #[test]
    fn a_body_is_written_with_exactly_one_newline_after_it() {
        for body in ["new", "new\n", "new\n\n\n"] {
            let got = ok(spliced(HELD, body)).expect("one pair of markers");
            assert!(got.contains("new\n# console-palette:end"), "{body:?} gave {got:?}");
        }
    }

    #[test]
    fn an_empty_region_is_filled() {
        let held = "# console-palette:begin\n# console-palette:end\n";
        assert_eq!(
            ok(spliced(held, "new")).expect("one pair"),
            "# console-palette:begin\nnew\n# console-palette:end\n"
        );
    }

    #[test]
    fn a_file_with_no_markers_is_refused_rather_than_appended_to() {
        assert_eq!(spliced("nothing here\n", "new"), Ok(None));
    }

    #[test]
    fn a_second_pair_of_markers_is_refused_rather_than_guessed_at() {
        let twice = format!("{HELD}{HELD}");
        assert_eq!(spliced(&twice, "new"), Ok(None));
    }

    #[test]
    fn markers_in_the_wrong_order_are_refused() {
        let backwards = "# console-palette:end\nbody\n# console-palette:begin\n";
        assert_eq!(spliced(backwards, "new"), Ok(None));
    }

    #[test]
    fn a_half_marked_file_is_refused() {
        assert_eq!(spliced("# console-palette:begin\nbody\n", "new"), Ok(None));
        assert_eq!(spliced("# console-palette:end\nbody\n", "new"), Ok(None));
    }
}

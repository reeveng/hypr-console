//! Writing into a file someone else owns.
//!
//! Three files on this machine cannot import a palette: KDE's ini format has no
//! include, and the browser's `user.js` and a systemd unit are both lists of
//! literals. A fourth, the compositor's Lua, could but must not, because a Lua
//! file that fails to load takes the session with it. Those four have a pair of
//! markers in them and only what lies between the markers is ours.

use console_core_never::Never;

pub const BEGIN: &str = "console-palette:begin";
pub const END: &str = "console-palette:end";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Body<'a>(pub &'a str);

pub fn spliced(held: &str, body: Body<'_>) -> Result<Option<String>, Never> {
    let body = body.0;
    let lines: Vec<&str> = held.split_inclusive('\n').collect();
    let holding = |needle: &str| lines.iter().filter(|line| line.contains(needle)).count();

    match (holding(BEGIN), holding(END)) {
        (1, 1) => {}
        _ => return Ok(None),
    }

    let head: Vec<&str> = lines
        .iter()
        .copied()
        .take_while(|line| !line.contains(BEGIN))
        .chain(lines.iter().copied().filter(|line| line.contains(BEGIN)))
        .collect();
    let tail: Vec<&str> = lines.iter().copied().skip_while(|line| !line.contains(END)).collect();

    match head.iter().any(|line| line.contains(END)) {
        true => Ok(None),
        false => Ok(Some(format!("{}{}\n{}", head.concat(), body.trim_end_matches('\n'), tail.concat()))),
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
        let got = ok(spliced(HELD, Body("new"))).expect("one pair of markers");
        assert_eq!(
            got,
            "before\n# console-palette:begin\nnew\n# console-palette:end\nafter\n"
        );
    }

    #[test]
    fn what_lies_outside_them_is_left_alone() {
        let got = ok(spliced(HELD, Body("new"))).expect("one pair of markers");
        assert!(got.starts_with("before\n"), "{got}");
        assert!(got.ends_with("after\n"), "{got}");
    }

    #[test]
    fn a_body_is_written_with_exactly_one_newline_after_it() {
        for body in ["new", "new\n", "new\n\n\n"] {
            let got = ok(spliced(HELD, Body(body))).expect("one pair of markers");
            assert!(got.contains("new\n# console-palette:end"), "{body:?} gave {got:?}");
        }
    }

    #[test]
    fn an_empty_region_is_filled() {
        let held = "# console-palette:begin\n# console-palette:end\n";
        assert_eq!(
            ok(spliced(held, Body("new"))).expect("one pair"),
            "# console-palette:begin\nnew\n# console-palette:end\n"
        );
    }

    #[test]
    fn a_file_with_no_markers_is_refused_rather_than_appended_to() {
        assert_eq!(spliced("nothing here\n", Body("new")), Ok(None));
    }

    #[test]
    fn a_second_pair_of_markers_is_refused_rather_than_guessed_at() {
        let twice = format!("{HELD}{HELD}");
        assert_eq!(spliced(&twice, Body("new")), Ok(None));
    }

    #[test]
    fn markers_in_the_wrong_order_are_refused() {
        let backwards = "# console-palette:end\nbody\n# console-palette:begin\n";
        assert_eq!(spliced(backwards, Body("new")), Ok(None));
    }

    #[test]
    fn a_half_marked_file_is_refused() {
        assert_eq!(spliced("# console-palette:begin\nbody\n", Body("new")), Ok(None));
        assert_eq!(spliced("# console-palette:end\nbody\n", Body("new")), Ok(None));
    }
}

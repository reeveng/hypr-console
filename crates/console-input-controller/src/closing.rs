//! Where closing the last window on a workspace leaves you.
//!
//! A window gets a workspace of its own, so closing one is closing a
//! workspace, and the compositor does not see it that way: it keeps the one
//! in front however empty it is, and only lets go of it once something else
//! is. So a desktop of five with the fourth closed still counted five, and the
//! one you were looking at was the blank one. Going to a neighbour is what
//! lets the compositor throw it away, and the one before is the neighbour,
//! because that is where the shoulders would have taken you back from.
//!
//! It is asked only of the window that was on the workspace in front. A
//! program in the background quitting says nothing about the empty workspace
//! somebody has just opened with the +, and taking them off it would be
//! closing a thing they made. So which workspace the window was on is read
//! from before it closed, because by the time the compositor says it closed
//! it may no longer answer for it.

use std::collections::BTreeMap;

use console_compositor::Window;
use console_core_never::Never;

pub fn placed(open: &[Window]) -> Result<BTreeMap<String, i64>, Never> {
    Ok(open.iter().map(|window| (window.address.clone(), window.workspace)).collect())
}

pub fn goes_to(was_on: Option<i64>, front: Option<i64>, open: &[Window], closed: &str) -> Result<Option<i64>, Never> {
    let front = match (was_on, front) {
        (Some(was_on), Some(front)) => match was_on == front && front > 0 {
            true => front,
            false => return Ok(None),
        },
        (None, _) | (_, None) => return Ok(None),
    };

    let others: Vec<i64> = open
        .iter()
        .filter(|window| window.address != closed && window.workspace > 0)
        .map(|window| window.workspace)
        .collect();

    let before = others.iter().copied().filter(|id| *id < front).max();
    let after = others.iter().copied().filter(|id| *id > front).min();

    Ok(match others.contains(&front) {
        true => None,
        false => before.or(after),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use console_compositor::{Filling, Floating, Pinned};

    fn on(address: &str, workspace: i64) -> Window {
        Window {
            address: address.to_string(),
            title: String::new(),
            first_class: String::new(),
            first_title: String::new(),
            workspace,
            workspace_named: workspace.to_string(),
            monitor: None,
            floating: Floating::No,
            pinned: Pinned::No,
            filling: Filling::None,
            at: (0, 0),
            size: (0, 0),
            pid: 0,
        }
    }

    fn five() -> Vec<Window> {
        (1..=5).map(|id| on(&format!("0x{id}"), id)).collect()
    }

    fn went(was_on: Option<i64>, front: Option<i64>, open: &[Window], closed: &str) -> Option<i64> {
        let Ok(to) = goes_to(was_on, front, open, closed);

        to
    }

    #[test]
    fn closing_the_fourth_of_five_goes_to_the_third() {
        assert_eq!(went(Some(4), Some(4), &five(), "0x4"), Some(3));
    }

    #[test]
    fn a_window_already_gone_from_the_list_is_answered_the_same() {
        let open: Vec<Window> = five().into_iter().filter(|window| window.address != "0x4").collect();

        assert_eq!(went(Some(4), Some(4), &open, "0x4"), Some(3));
    }

    #[test]
    fn closing_the_first_goes_to_the_one_after() {
        assert_eq!(went(Some(1), Some(1), &five(), "0x1"), Some(2));
    }

    #[test]
    fn closing_the_last_window_there_is_stays_where_it_is() {
        assert_eq!(went(Some(1), Some(1), &[on("0x1", 1)], "0x1"), None);
    }

    #[test]
    fn a_window_closing_somewhere_else_moves_nobody() {
        assert_eq!(went(Some(2), Some(6), &five(), "0x2"), None);
    }

    #[test]
    fn a_workspace_with_a_window_still_on_it_is_kept() {
        let mut open = five();
        open.push(on("0x44", 4));

        assert_eq!(went(Some(4), Some(4), &open, "0x4"), None);
    }

    #[test]
    fn a_window_nobody_saw_open_moves_nobody() {
        assert_eq!(went(None, Some(4), &five(), "0x9"), None);
    }

    #[test]
    fn a_special_workspace_is_not_somewhere_to_be_sent() {
        let open = vec![on("0x1", 1), on("0x99", -99)];

        assert_eq!(went(Some(1), Some(1), &open, "0x1"), None);
    }
}

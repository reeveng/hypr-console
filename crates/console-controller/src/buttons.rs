//! Whether a press is this daemon's to act on, and what it comes to.
//!
//! What a button means is `means`, in one table. This is the part that decides
//! whether we are the ones reading at all: the on-screen keyboard reads the
//! pad itself while it is up, and the card that asks which button that was
//! wants a machine where a press does nothing but answer it.
//!
//! Both of those used to be answered somewhere else -- by another program
//! sending this one SIGSTOP, and by a button simply not appearing in a table.
//! See `console_controller::mode`.

use console_pad::jobs::Layer;

use crate::doing::Doing;
use crate::means::{Job, Table};
use crate::mode::Mode;

/// The job a press lands on, if this daemon is reading and anything is bound.
pub fn job_for(
    table: &Table,
    mode: Mode,
    button: &str,
    layer: Layer,
) -> Option<&'static Job> {
    match mode.acts() {
        true => table.what(button, layer, mode),
        false => None,
    }
}

/// What that job does about a press, on the way down or on the way back up.
pub fn acted(job: &Job, down: bool) -> Option<Doing> {
    job.what.does(down)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::means::What;
    use console_pad::jobs::ALONE;

    fn table() -> Table {
        Table::ours()
    }

    fn what(mode: Mode, button: &str) -> Option<What> {
        job_for(&table(), mode, button, ALONE).map(|job| job.what)
    }

    #[test]
    fn a_back_button_runs_what_it_is_for() {
        assert_eq!(what(Mode::Desktop, "left-paddle-top"), Some(What::Menu));
        assert_eq!(what(Mode::Desktop, "legion-right"), Some(What::Settings));
    }

    /// While the on-screen keyboard is up the pad is its own, and while the
    /// card is asking which button that was the pad is nobody's. Both are the
    /// same answer: not ours to act on.
    #[test]
    fn nothing_is_acted_on_where_this_daemon_is_not_the_one_reading() {
        for mode in [Mode::Keyboard, Mode::Asking] {
            assert_eq!(what(mode, "left-paddle-top"), None, "{mode:?}");
            assert_eq!(what(mode, "a"), None, "{mode:?}");
        }
    }

    /// A button nothing is bound to is a button that does nothing, and not a
    /// button that does something else.
    #[test]
    fn a_button_with_nothing_on_it_does_nothing() {
        assert_eq!(what(Mode::Desktop, "l3"), None);
        assert_eq!(what(Mode::Desktop, "right-paddle-3"), None);
    }
}

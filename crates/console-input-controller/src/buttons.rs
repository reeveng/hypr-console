//! Whether a press is this daemon's to act on, and what it comes to.
//!
//! What a button means is `means`, in one table. This is the part that decides
//! whether we are the ones reading at all: the on-screen keyboard reads the
//! pad itself while it is up, and the card that asks which button that was
//! wants a machine where a press does nothing but answer it.
//!
//! Both of those used to be answered somewhere else -- by another program
//! sending this one SIGSTOP, and by a button simply not appearing in a table.
//! See `console_input_controller::mode`.
//!
//! Both of those programs take the devices themselves now, so nothing reaches
//! this daemon while either is up whether or not this says so. It says so
//! anyway, and the two are not the same answer: a claim is taken a moment after
//! a surface goes up and given back a moment after it comes down, and this is
//! what covers those two moments.

use console_input_gamepad::jobs::Layer;
use console_core_never::Never;

use crate::doing::Doing;
use crate::means::{Job, Press, Table};
use crate::mode::{Acts, Mode};

pub fn job_for(
    table: &Table,
    mode: Mode,
    button: &str,
    layer: Layer,
) -> Result<Option<&'static Job>, Never> {
    let Ok(acts) = mode.acts();

    match acts {
        Acts::OnPresses => table.what(button, layer, mode),
        Acts::NotReading => Ok(None),
    }
}

pub fn acted(job: &Job, down: Press) -> Result<Option<Doing>, Never> {
    job.what.does(down)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::means::What;
    use console_input_gamepad::jobs::ALONE;

    fn table() -> Table {
        let Ok(ours) = Table::ours();

        ours
    }

    fn what(mode: Mode, button: &str) -> Option<What> {
        let Ok(found) = job_for(&table(), mode, button, ALONE);

        found.map(|job| job.what)
    }

    #[test]
    fn a_back_button_runs_what_it_is_for() {
        assert_eq!(what(Mode::Desktop, "left-paddle-top"), Some(What::Menu));
        assert_eq!(what(Mode::Desktop, "legion-right"), Some(What::Settings));
    }

    #[test]
    fn nothing_is_acted_on_where_this_daemon_is_not_the_one_reading() {
        for mode in [Mode::Keyboard, Mode::Asking] {
            assert_eq!(what(mode, "left-paddle-top"), None, "{mode:?}");
            assert_eq!(what(mode, "a"), None, "{mode:?}");
        }
    }

    #[test]
    fn a_button_with_nothing_on_it_does_nothing() {
        assert_eq!(what(Mode::Desktop, "l3"), None);
        assert_eq!(what(Mode::Desktop, "right-paddle-3"), None);
    }
}

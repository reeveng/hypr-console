//! Where a panel keeps what it remembers about itself between openings.
//!
//! Under the state directory rather than the cache, because these are things
//! the desktop remembers about itself rather than things it can work out
//! again. One file each, named for the panel and for the note, so a panel that
//! learns to remember something new does not have to be taught where.
//!
//! None of it is required to work. A file that cannot be read or written is a
//! panel that opens the way it did before there was one.

use console_core_never::Never;
use std::path::PathBuf;

pub fn beside(program: &str, note: &str) -> Result<Option<PathBuf>, Never> {
    let state = match std::env::var("XDG_STATE_HOME") {
        Ok(state) => Some(state),
        Err(_) => None,
    };

    let home = match std::env::var("HOME") {
        Ok(home) => Some(home),
        Err(_) => None,
    };

    under(state.as_deref(), home.as_deref(), program, note)
}

fn under(
    state: Option<&str>,
    home: Option<&str>,
    program: &str,
    note: &str,
) -> Result<Option<PathBuf>, Never> {
    let state = match (state, home) {
        (Some(state), _) => PathBuf::from(state),
        (None, Some(home)) => PathBuf::from(home).join(".local/state"),
        (None, None) => return Ok(None),
    };

    Ok(Some(state.join("console/panel").join(format!("{program}.{note}"))))
}

pub fn write(program: &str, note: &str, said: &str) -> Result<(), Never> {
    let Ok(beside) = beside(program, note);

    let Some(path) = beside else { return Ok(()) };

    let Some(holding) = path.parent() else { return Ok(()) };

    match std::fs::create_dir_all(holding) {
        Ok(_) => {},
        Err(fault) => {
            eprintln!("console: {}: keeping a panel's note: {fault}", holding.display());

            return Ok(());
        }
    }

    let _ = std::fs::write(path, said);

    Ok(())
}

pub fn read(program: &str, note: &str) -> Result<Option<String>, Never> {
    let Ok(beside) = beside(program, note);

    let Some(path) = beside else { return Ok(None) };

    let Ok(said) = std::fs::read_to_string(path) else { return Ok(None) };

    Ok(Some(said))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_is_named_for_the_panel_and_for_itself() {
        assert_eq!(
            under(Some("/tmp/state"), None, "settings-panel", "tab"),
            Ok(Some(PathBuf::from("/tmp/state/console/panel/settings-panel.tab")))
        );
    }

    #[test]
    fn a_login_that_names_no_state_directory_keeps_it_under_the_home() {
        assert_eq!(
            under(None, Some("/home/someone"), "settings-panel", "tab"),
            Ok(Some(PathBuf::from("/home/someone/.local/state/console/panel/settings-panel.tab")))
        );
    }

    #[test]
    fn nothing_named_is_nothing_remembered() {
        assert_eq!(under(None, None, "settings-panel", "tab"), Ok(None));
    }
}

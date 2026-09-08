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
use console_core_places::{Base, OURS};
use std::path::{Path, PathBuf};

pub fn beside(program: &str, note: &str) -> Result<Option<PathBuf>, Never> {
    let state = Base::State.hers()?;

    under(state.as_deref(), program, note)
}

fn under(state: Option<&Path>, program: &str, note: &str) -> Result<Option<PathBuf>, Never> {
    Ok(state.map(|state| state.join(OURS).join("panel").join(format!("{program}.{note}"))))
}

pub fn write(program: &str, note: &str, said: &str) -> Result<(), Never> {
    let Ok(beside) = beside(program, note);

    let path = match beside {
        Some(path) => path,
        None => return Ok(()),
    };

    let holding = match path.parent() {
        Some(holding) => holding,
        None => return Ok(()),
    };

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

    let path = match beside {
        Some(path) => path,
        None => return Ok(None),
    };

    let said = match std::fs::read_to_string(path) {
        Ok(said) => said,
        Err(_fault) => return Ok(None),
    };

    Ok(Some(said))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_is_named_for_the_panel_and_for_itself() {
        assert_eq!(
            under(Some(Path::new("/tmp/state")), "settings-panel", "tab"),
            Ok(Some(PathBuf::from("/tmp/state/console/panel/settings-panel.tab")))
        );
    }

    #[test]
    fn nothing_named_is_nothing_remembered() {
        assert_eq!(under(None, "settings-panel", "tab"), Ok(None));
    }
}

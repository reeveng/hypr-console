//! Where a panel keeps what it remembers about itself between openings.
//!
//! Under the state directory rather than the cache, because these are things
//! the desktop remembers about itself rather than things it can work out
//! again. One file each, named for the panel and for the note, so a panel that
//! learns to remember something new does not have to be taught where.
//!
//! None of it is required to work. A file that cannot be read or written is a
//! panel that opens the way it did before there was one.

use std::path::PathBuf;

/// One note, for one panel.
pub fn beside(program: &str, note: &str) -> Option<PathBuf> {
    // Neither is a session with nowhere to keep a note, and a panel that
    // cannot write one down opens on its first tab every time rather than not
    // opening at all.
    let state = match (std::env::var("XDG_STATE_HOME"), std::env::var("HOME")) {
        (Ok(state), _) => PathBuf::from(state),
        (Err(_), Ok(home)) => PathBuf::from(home).join(".local/state"),
        (Err(_), Err(_)) => return None,
    };

    Some(state.join("console/panel").join(format!("{program}.{note}")))
}

/// Write one down, making room for it if this is the first.
pub fn write(program: &str, note: &str, said: &str) {
    let Some(path) = beside(program, note) else { return };

    let Some(holding) = path.parent() else { return };

    if let Err(fault) = std::fs::create_dir_all(holding) {
        eprintln!("console: {}: keeping a panel's note: {fault}", holding.display());

        return;
    }

    let _ = std::fs::write(path, said);
}

/// Read one back, if it has ever been written.
pub fn read(program: &str, note: &str) -> Option<String> {
    let path = beside(program, note)?;

    let Ok(said) = std::fs::read_to_string(path) else { return None };

    Some(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_is_named_for_the_panel_and_for_itself() {
        // SAFETY: single-threaded test, and the variable is read here alone.
        unsafe { std::env::set_var("XDG_STATE_HOME", "/tmp/state") };
        assert_eq!(
            beside("settings-panel", "tab"),
            Some(PathBuf::from("/tmp/state/console/panel/settings-panel.tab"))
        );
    }
}

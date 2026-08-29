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
    let state = std::env::var("XDG_STATE_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|home| PathBuf::from(home).join(".local/state")))?;
    Some(state.join("console/panel").join(format!("{program}.{note}")))
}

/// Write one down, making room for it if this is the first.
pub fn write(program: &str, note: &str, said: &str) {
    let Some(path) = beside(program, note) else { return };
    let Some(holding) = path.parent() else { return };
    if std::fs::create_dir_all(holding).is_err() {
        return;
    }
    let _ = std::fs::write(path, said);
}

/// Read one back, if it has ever been written.
pub fn read(program: &str, note: &str) -> Option<String> {
    beside(program, note).and_then(|path| std::fs::read_to_string(path).ok())
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

//! The tab a panel was left on.
//!
//! Opened with a tab named, a panel opens at that one: tapping the battery on
//! the bar arrives at Battery whatever was last looked at. Opened with nothing
//! named, it used to open at the first tab every time, which is the one answer
//! that is right for nobody: somebody who opened the settings for the Wi-Fi
//! twice running was taken to the battery twice running.
//!
//! So a panel opened with nothing named opens where it was left. A tab that
//! has since gone is a name nothing answers to, and that already opens the
//! first tab rather than nothing at all.

use crate::notes;

/// Which note this is.
const TAB: &str = "tab";

/// The tab this panel was left on, if it has been up before.
pub fn last(program: &str) -> Option<String> {
    read(&notes::read(program, TAB)?)
}

/// The file, as a tab name. A file saying nothing names no tab, which opens
/// the first one.
fn read(said: &str) -> Option<String> {
    let said = said.trim();
    (!said.is_empty()).then(|| said.to_string())
}

/// Remember the tab it is on now.
///
/// Written only when it changed, so a panel walked back and forth all day
/// writes once for each tab it stops on.
pub fn keep(program: &str, title: &str) {
    if title.is_empty() || last(program).as_deref() == Some(title) {
        return;
    }
    notes::write(program, TAB, &format!("{title}\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_on_the_tab_is_what_is_read() {
        assert_eq!(read("Wi-Fi\n"), Some("Wi-Fi".to_string()));
        assert_eq!(read("  Game Mode  "), Some("Game Mode".to_string()));
    }

    /// Which opens the first tab, the same as a panel that has never been up.
    #[test]
    fn a_file_saying_nothing_names_no_tab() {
        assert_eq!(read(""), None);
        assert_eq!(read("\n  \n"), None);
    }
}

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
use console_core_never::Never;

const TAB: &str = "tab";

pub fn last(program: &str) -> Result<Option<String>, Never> {
    let Ok(said) = notes::read(program, TAB);

    let said = match said {
        Some(said) => said,
        None => return Ok(None),
    };

    read(&said)
}

fn read(said: &str) -> Result<Option<String>, Never> {
    let said = said.trim();

    Ok((!said.is_empty()).then(|| said.to_string()))
}

pub fn keep(program: &str, title: &str) -> Result<(), Never> {
    let Ok(last) = last(program);

    match title.is_empty() || last.as_deref() == Some(title) {
        true => return Ok(()),
        false => {},
    }

    let Ok(()) = notes::write(program, TAB, &format!("{title}\n"));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_on_the_tab_is_what_is_read() {
        assert_eq!(read("Wi-Fi\n"), Ok(Some("Wi-Fi".to_string())));
        assert_eq!(read("  Game Mode  "), Ok(Some("Game Mode".to_string())));
    }

    #[test]
    fn a_file_saying_nothing_names_no_tab() {
        assert_eq!(read(""), Ok(None));
        assert_eq!(read("\n  \n"), Ok(None));
    }
}

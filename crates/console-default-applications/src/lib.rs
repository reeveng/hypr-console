//! What this desktop opens things with.
//!
//! Which browser a link means, which engine a question is asked of, and where
//! the battery starts saying something. All three used to be written into a
//! program. A setting nobody can reach is a setting somebody has to be asked
//! to change, and there is nobody to ask on a machine with one person on it.
//!
//! The browser is xdg-settings', because every program on the machine asks
//! that and a second copy here would be a second answer. The other two have no
//! such place, so there is a file, and this is what reads and writes it.

pub mod battery;
pub mod browsers;
pub mod clock;
pub mod engines;
pub mod policies;

use std::path::PathBuf;

use console_core_atomic_writes::Held;
use console_core_never::Never;

fn held(at: &std::path::Path) -> Result<String, String> {
    let Ok(held) = console_core_atomic_writes::read(at);

    match held {
        Held::Said(said) => Ok(said),
        Held::Nothing => Ok(String::new()),
        Held::Unreadable(fault) => Err(format!("{}: {fault}", at.display())),
    }
}

pub const NAMED: &str = "defaults";

pub fn where_() -> Result<Option<PathBuf>, Never> {
    let ours = console_core_places::Base::Config.ours()?;

    Ok(ours.map(|ours| ours.join(NAMED)))
}

pub fn under(home: &std::path::Path) -> Result<PathBuf, Never> {
    let Ok(ours) = console_core_places::Base::Config.ours_under(home);

    Ok(ours.join(NAMED))
}

pub fn read(said: &str) -> Result<Vec<(String, String)>, Never> {
    Ok(said
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .filter(|(key, _)| !key.is_empty())
        .collect())
}

pub fn written(said: &str, key: &str, value: &str) -> Result<String, Never> {
    let mut settings = read(said)?;

    match settings.iter_mut().find(|(named, _)| named == key) {
        Some(found) => found.1 = value.to_string(),
        None => settings.push((key.to_string(), value.to_string())),
    }

    settings.sort_by(|one, two| one.0.cmp(&two.0));

    Ok(settings.iter().map(|(key, value)| format!("{key}={value}\n")).collect())
}

pub fn setting(key: &str) -> Result<Option<String>, Never> {
    let at = where_()?;

    let said = match at.as_ref().map(std::fs::read_to_string) {
        Some(Ok(said)) => said,
        Some(Err(_the_settings_are_unreadable)) => return Ok(None),
        None => return Ok(None),
    };

    let settings = read(&said)?;

    Ok(settings.into_iter().find(|(named, _)| named == key).map(|(_, value)| value))
}

pub fn set(key: &str, value: &str) -> Result<(), Never> {
    let keeping = where_()?;

    let at = match keeping {
        Some(at) => at,
        None => {
            eprintln!("console-default-applications: no home to keep {key} in; leaving it as it is");

            return Ok(());
        }
    };

    match at.parent() {
        Some(parent) => {
            let _ = std::fs::create_dir_all(parent);
        }
        None => {}
    }

    let said = match held(&at) {
        Ok(said) => said,
        Err(fault) => {
            eprintln!("console-default-applications: {fault}; leaving it as it is");

            return Ok(());
        }
    };

    let written = written(&said, key, value)?;

    match std::fs::write(&at, written) {
        Ok(()) => {}
        Err(fault) => eprintln!("console-default-applications: {}: {fault}", at.display()),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setting_is_a_key_and_a_value() {
        assert_eq!(
            read("search=startpage\n"),
            Ok(vec![("search".to_string(), "startpage".to_string())])
        );
    }

    #[test]
    fn a_file_written_by_hand_is_read_the_same() {
        let said = "# which engine\n  search = startpage  \n\nnonsense\n";

        assert_eq!(read(said), Ok(vec![("search".to_string(), "startpage".to_string())]));
    }

    #[test]
    fn setting_one_leaves_the_others_where_they_were() {
        let said = "browser=librewolf.desktop\nsearch=duckduckgo\n";

        assert_eq!(
            written(said, "search", "startpage"),
            Ok("browser=librewolf.desktop\nsearch=startpage\n".to_string())
        );
    }

    #[test]
    fn setting_one_that_was_never_there_writes_it() {
        assert_eq!(written("", "search", "wikipedia"), Ok("search=wikipedia\n".to_string()));
    }
}

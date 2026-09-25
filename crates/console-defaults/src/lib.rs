//! The settings no one else owns, in one file of `key=value` lines.
//!
//! Which engine a question is asked of, where the battery starts saying
//! something, how the clock is written, which alphabets and which languages
//! are offered: each belongs to a crate that knows what the value means, and
//! none of them has a file of its own to keep it in. So there is one, named
//! for what Apple calls the same thing, and this reads and writes it. What a
//! key means is the caller's; this knows a key, a value and where they live.
//!
//! Settings, the bar and the browser all write here, and a write is a read of
//! the whole file, one line changed and the whole file put back. Two of those
//! close together each put back what they read before the other wrote, and one
//! setting is lost without a word. So a write holds the directory's lock from
//! the read to the rename. And it changes the one line it was asked to and
//! leaves every other line as it was, comments and all, because a file that
//! says it may be written by hand is one somebody did.

use std::fmt;
use std::path::PathBuf;

use console_core_atomic_writes::Stored;
use console_core_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unread {
    pub at: PathBuf,
    pub fault: String,
}

impl fmt::Display for Unread {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(to, "{}: {}", self.at.display(), self.fault)
    }
}

impl std::error::Error for Unread {}

fn held(at: &std::path::Path) -> Result<String, Unread> {
    let Ok(held) = console_core_atomic_writes::read(at);

    match held {
        Stored::Text(said) => Ok(said),
        Stored::Absent => Ok(String::new()),
        Stored::Failed(fault) => Err(Unread { at: at.to_path_buf(), fault }),
    }
}

pub const NAMED: &str = "defaults";

pub fn where_() -> Result<Option<PathBuf>, Never> {
    let ours = console_core_places::Base::Configuration.ours()?;

    Ok(ours.map(|ours| ours.join(NAMED)))
}

pub fn under(home: &std::path::Path) -> Result<PathBuf, Never> {
    let Ok(ours) = console_core_places::Base::Configuration.ours_under(home);

    Ok(ours.join(NAMED))
}

fn pair(line: &str) -> Result<Option<(String, String)>, Never> {
    let line = line.trim();

    Ok(match line.starts_with('#') {
        true => None,
        false => line
            .split_once('=')
            .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
            .filter(|(key, _)| !key.is_empty()),
    })
}

pub fn read(said: &str) -> Result<Vec<(String, String)>, Never> {
    Ok(said
        .lines()
        .filter_map(|line| {
            let Ok(pair) = pair(line);
            pair
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setting<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

pub fn written(said: &str, setting: Setting<'_>) -> Result<String, Never> {
    let Setting { key, value } = setting;
    let mut lines: Vec<String> = said.lines().map(str::to_string).collect();
    let line = format!("{key}={value}");

    let found = lines.iter_mut().find(|held| {
        let Ok(pair) = pair(held);
        pair.is_some_and(|(named, _)| named == key)
    });

    match found {
        Some(found) => *found = line,
        None => lines.push(line),
    }

    Ok(lines.iter().map(|line| format!("{line}\n")).collect())
}

pub struct Choice<T: 'static> {
    pub setting: &'static str,
    pub unless_told: &'static str,
    pub known: fn(&str) -> Result<Option<&'static T>, Never>,
}

pub fn chosen<T>(choice: Choice<T>) -> Result<String, Never> {
    let told = setting(choice.setting)?;

    let said = match told {
        Some(said) => said,
        None => String::new(),
    };
    let known = (choice.known)(&said)?;

    Ok(match known {
        Some(_) => said,
        None => choice.unless_told.to_string(),
    })
}

pub fn setting(key: &str) -> Result<Option<String>, Never> {
    let at = where_()?;

    let said = match at.as_deref().map(held) {
        Some(Ok(said)) => said,
        Some(Err(fault)) => {
            eprintln!("console-defaults: {fault}; answering {key} as never chosen");

            return Ok(None);
        }
        None => return Ok(None),
    };

    let settings = read(&said)?;

    Ok(settings.into_iter().find(|(named, _)| named == key).map(|(_, value)| value))
}

pub fn set(setting: Setting<'_>) -> Result<(), Never> {
    let keeping = where_()?;

    match keeping {
        Some(at) => set_in(&at, setting),
        None => {
            eprintln!("console-defaults: no home to keep {} in; leaving it as it is", setting.key);

            Ok(())
        }
    }
}

pub fn set_in(at: &std::path::Path, setting: Setting<'_>) -> Result<(), Never> {
    let parent = match at.parent() {
        Some(parent) => parent,
        None => {
            eprintln!("console-defaults: {} is in no directory; leaving it as it is", at.display());

            return Ok(());
        }
    };

    let _ = std::fs::create_dir_all(parent);

    let alone = match std::fs::File::open(parent).and_then(|directory| {
        rustix::fs::flock(&directory, rustix::fs::FlockOperation::LockExclusive)
            .map(|()| directory)
            .map_err(std::io::Error::from)
    }) {
        Ok(alone) => alone,
        Err(fault) => {
            eprintln!("console-defaults: {}: waiting to be the only writer: {fault}; leaving it as it is", parent.display());

            return Ok(());
        }
    };

    let said = match held(at) {
        Ok(said) => said,
        Err(fault) => {
            eprintln!("console-defaults: {fault}; leaving it as it is");

            return Ok(());
        }
    };

    let written = written(&said, setting)?;

    match console_core_atomic_writes::whole(at, written.as_bytes()) {
        Ok(()) => {}
        Err(fault) => eprintln!("console-defaults: {}: {fault}", at.display()),
    }

    drop(alone);

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
            written(said, Setting { key: "search", value: "startpage" }),
            Ok("browser=librewolf.desktop\nsearch=startpage\n".to_string())
        );
    }

    #[test]
    fn setting_one_that_was_never_there_writes_it() {
        assert_eq!(
            written("", Setting { key: "search", value: "wikipedia" }),
            Ok("search=wikipedia\n".to_string())
        );
    }

    #[test]
    fn setting_one_keeps_what_was_written_by_hand() {
        let said = "# which engine\n  search = duckduckgo  \n\nnonsense\n";

        assert_eq!(
            written(said, Setting { key: "search", value: "startpage" }),
            Ok("# which engine\nsearch=startpage\n\nnonsense\n".to_string())
        );
    }

    #[test]
    fn two_settings_written_at_once_are_both_kept() {
        let here = std::env::temp_dir().join(format!("console-defaults-two-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&here);
        let at = here.join(NAMED);

        std::thread::scope(|scope| {
            for key in ["one", "two", "three", "four"] {
                let at = &at;
                scope.spawn(move || {
                    for round in 0..50 {
                        let Ok(()) = set_in(at, Setting { key, value: &round.to_string() });
                    }
                });
            }
        });

        let said = std::fs::read_to_string(&at).expect("the settings");
        let Ok(mut settings) = read(&said);
        settings.sort();

        let every = ["four", "one", "three", "two"].map(|key| (key.to_string(), "49".to_string()));

        assert_eq!(settings, every.to_vec(), "a setting written beside another was lost");
    }
}

//! What a .desktop file says about an application.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_core_never::Never;

use crate::words::without_field_codes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Application {
    pub name: String,
    pub command: String,
    pub terminal: bool,
    pub icon: String,
}

fn fields(said: &str) -> Result<BTreeMap<String, String>, Never> {
    let mut found = BTreeMap::new();
    let mut inside = false;

    for line in said.lines().map(str::trim) {
        match line.starts_with('[') {
            true => {
                inside = line == "[Desktop Entry]";
                continue;
            }
            false => {},
        }

        match inside {
            true => {},
            false => continue,
        }

        match line.split_once('=') {
            Some((key, value)) => {
                found.entry(key.to_string()).or_insert_with(|| value.to_string());
            }
            None => {},
        }
    }

    Ok(found)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installed {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Worth {
    Drawing,
    Skipping,
}

fn worth_drawing(fields: &BTreeMap<String, String>) -> Result<Worth, Never> {
    let says = |key: &str| fields.get(key).map(|said| said.to_lowercase());
    let drawn = fields.get("Type").is_some_and(|kind| kind == "Application")
        && says("NoDisplay").as_deref() != Some("true")
        && says("Hidden").as_deref() != Some("true");

    Ok(match drawn {
        true => Worth::Drawing,
        false => Worth::Skipping,
    })
}

pub fn read(
    said: &str,
    here: impl Fn(&str) -> Result<Installed, Never>,
) -> Result<Option<Application>, Never> {
    let fields = fields(said)?;

    let worth = worth_drawing(&fields)?;

    match worth {
        Worth::Skipping => return Ok(None),
        Worth::Drawing => {},
    }

    match fields.get("TryExec") {
        Some(wanted) => {
            let installed = here(wanted)?;

            match installed {
                Installed::No => return Ok(None),
                Installed::Yes => {},
            }
        }
        None => {},
    }

    let (Some(name), Some(command)) = (fields.get("Name"), fields.get("Exec")) else {
        return Ok(None);
    };

    match name.is_empty() || command.is_empty() {
        true => return Ok(None),
        false => {},
    }

    let command = without_field_codes(command)?;

    Ok(Some(Application {
        name: name.clone(),
        command,
        terminal: fields.get("Terminal").map(|said| said.to_lowercase()).as_deref() == Some("true"),
        icon: fields.get("Icon").cloned().unwrap_or_default(),
    }))
}

pub fn files(home: &Path, data_dirs: &str) -> Result<Vec<PathBuf>, Never> {
    let mut roots = vec![home.join(".local/share/applications")];
    roots.extend(data_dirs.split(':').filter(|dir| !dir.is_empty()).map(|dir| Path::new(dir).join("applications")));
    let mut found: BTreeMap<String, PathBuf> = BTreeMap::new();

    for root in roots {
        let under = under(&root)?;

        for path in under {
            let Some(name) = path.file_name().map(|name| name.to_string_lossy().to_string()) else {
                continue;
            };

            found.entry(name).or_insert(path);
        }
    }

    Ok(found.into_values().collect())
}

fn under(root: &Path) -> Result<Vec<PathBuf>, Never> {
    let Ok(reading) = std::fs::read_dir(root) else { return Ok(Vec::new()) };

    let mut found: Vec<PathBuf> = Vec::new();
    let mut names: Vec<PathBuf> = reading.filter_map(Result::ok).map(|entry| entry.path()).collect();
    names.sort();

    for path in names {
        match path.is_dir() {
            true => {
                let deeper = under(&path)?;

                found.extend(deeper);
            }
            false => match path.extension().is_some_and(|kind| kind == "desktop") {
                true => found.push(path),
                false => {},
            },
        }
    }

    Ok(found)
}

pub const DATA_DIRS: &str = "/usr/local/share:/usr/share";

#[cfg(test)]
mod tests {
    use super::*;

    const SAID: &str = "\
[Desktop Entry]
Type=Application
Name=Firefox
Exec=firefox %u
Icon=firefox
Terminal=false

[Desktop Action new-window]
Name=New Window
Exec=firefox --new-window
";

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    fn anything(_: &str) -> Result<Installed, Never> {
        Ok(Installed::Yes)
    }

    fn also(line: &str) -> String {
        SAID.replace("Terminal=false", &format!("Terminal=false\n{line}"))
    }

    #[test]
    fn an_entry_is_a_name_a_command_and_an_icon() {
        assert_eq!(
            ok(read(SAID, anything)),
            Some(Application {
                name: "Firefox".to_string(),
                command: "firefox".to_string(),
                terminal: false,
                icon: "firefox".to_string(),
            })
        );
    }

    #[test]
    fn only_the_entry_itself_is_read() {
        assert_eq!(ok(read(SAID, anything)).expect("firefox").name, "Firefox");
    }

    #[test]
    fn something_that_asks_not_to_be_seen_is_not_drawn() {
        for asking in ["NoDisplay=true", "Hidden=TRUE"] {
            assert_eq!(ok(read(&also(asking), anything)), None, "{asking}");
        }
    }

    #[test]
    fn something_that_is_not_an_application_is_not_drawn() {
        assert_eq!(ok(read("[Desktop Entry]\nType=Directory\nName=Games\n", anything)), None);
    }

    #[test]
    fn something_that_names_a_program_this_machine_has_not_got_is_not_drawn() {
        let said = also("TryExec=firefox");
        assert_eq!(ok(read(&said, |_| Ok(Installed::No))), None);
        assert!(ok(read(&said, anything)).is_some());
    }

    #[test]
    fn an_entry_with_nothing_to_run_is_not_drawn() {
        assert_eq!(ok(read("[Desktop Entry]\nType=Application\nName=Nothing\n", anything)), None);
    }
}

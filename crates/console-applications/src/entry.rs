//! What a .desktop file says about an application.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_core_ini_files::fields;
use console_core_never::Never;

use crate::words::without_field_codes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Application {
    pub name: String,
    pub command: String,
    pub terminal: bool,
    pub icon: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installed {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worth {
    Drawing,
    Skipping,
}

const GROUP: &str = "Desktop Entry";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DesktopEntry<'a> {
    pub kind: Option<&'a str>,
    pub name: Option<&'a str>,
    pub command: Option<&'a str>,
    pub icon: Option<&'a str>,
    pub terminal: Option<&'a str>,
    pub try_exec: Option<&'a str>,
    pub no_display: Option<&'a str>,
    pub hidden: Option<&'a str>,
    pub mime: Option<&'a str>,
}

impl<'a> DesktopEntry<'a> {
    pub fn read(said: &'a str) -> Result<Self, Never> {
        let fields = fields(said, GROUP)?;
        let of = |key| fields.get(key).copied();

        Ok(DesktopEntry {
            kind: of("Type"),
            name: of("Name"),
            command: of("Exec"),
            icon: of("Icon"),
            terminal: of("Terminal"),
            try_exec: of("TryExec"),
            no_display: of("NoDisplay"),
            hidden: of("Hidden"),
            mime: of("MimeType"),
        })
    }

    pub fn worth(&self) -> Result<Worth, Never> {
        let said = |value: Option<&str>| value.map(str::to_lowercase);
        let drawn = self.kind == Some("Application")
            && said(self.no_display).as_deref() != Some("true")
            && said(self.hidden).as_deref() != Some("true");

        Ok(match drawn {
            true => Worth::Drawing,
            false => Worth::Skipping,
        })
    }

    pub fn says(&self) -> Result<Option<&'a str>, Never> {
        Ok(self.name.filter(|name| !name.is_empty()))
    }

    pub fn opens(&self) -> Result<Vec<String>, Never> {
        Ok(self
            .mime
            .unwrap_or_default()
            .split(';')
            .filter(|kind| !kind.is_empty())
            .map(str::to_string)
            .collect())
    }
}

pub fn read(
    said: &str,
    here: impl Fn(&str) -> Result<Installed, Never>,
) -> Result<Option<Application>, Never> {
    let entry = DesktopEntry::read(said)?;

    let worth = entry.worth()?;

    match worth {
        Worth::Skipping => return Ok(None),
        Worth::Drawing => {},
    }

    match entry.try_exec {
        Some(wanted) => {
            let installed = here(wanted)?;

            match installed {
                Installed::No => return Ok(None),
                Installed::Yes => {},
            }
        }
        None => {},
    }

    let says = entry.says()?;

    let (name, command) = match (says, entry.command.filter(|command| !command.is_empty())) {
        (Some(name), Some(command)) => (name, command),
        (None, _) | (_, None) => return Ok(None),
    };

    let command = without_field_codes(command)?;
    let terminal = entry.terminal.map(str::to_lowercase).as_deref() == Some("true");

    Ok(Some(Application {
        name: name.to_string(),
        command,
        terminal,
        icon: entry.icon.unwrap_or_default().to_string(),
    }))
}

pub fn files(roots: &[PathBuf]) -> Result<Vec<PathBuf>, Never> {
    let mut found: BTreeMap<String, PathBuf> = BTreeMap::new();

    for root in roots {
        let under = under(root)?;

        for path in under {
            let name = match path.file_name().map(|name| name.to_string_lossy().to_string()) {
                Some(name) => name,
                None => continue,
            };

            found.entry(name).or_insert(path);
        }
    }

    Ok(found.into_values().collect())
}

fn under(root: &Path) -> Result<Vec<PathBuf>, Never> {
    let reading = match std::fs::read_dir(root) {
        Ok(reading) => reading,
        Err(_fault) => return Ok(Vec::new()),
    };

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

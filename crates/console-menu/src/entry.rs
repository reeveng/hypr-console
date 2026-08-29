//! What a .desktop file says about an application.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::words::without_field_codes;

/// One application, as far as a menu is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Application {
    pub name: String,
    pub command: String,
    /// Whether it wants a terminal opened round it.
    pub terminal: bool,
    pub icon: String,
}

/// The fields of the entry itself, which is the first section and no other.
///
/// A .desktop file holds actions in sections of their own, and the first
/// spelling of a key is the one that counts.
fn fields(said: &str) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let mut inside = false;
    for line in said.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == "[Desktop Entry]";
            continue;
        }
        if !inside {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            found.entry(key.to_string()).or_insert_with(|| value.to_string());
        }
    }
    found
}

/// Whether this is a thing to run, and whether it wants to be seen.
fn worth_drawing(fields: &BTreeMap<String, String>) -> bool {
    let says = |key: &str| fields.get(key).map(|said| said.to_lowercase());
    fields.get("Type").is_some_and(|kind| kind == "Application")
        && says("NoDisplay").as_deref() != Some("true")
        && says("Hidden").as_deref() != Some("true")
}

/// The application, or nothing where there is nothing to draw.
///
/// `here` says whether a program named in TryExec is on this machine, which is
/// how a .desktop file for something uninstalled asks to be left out.
pub fn read(said: &str, here: impl Fn(&str) -> bool) -> Option<Application> {
    let fields = fields(said);
    if !worth_drawing(&fields) {
        return None;
    }
    if let Some(wanted) = fields.get("TryExec")
        && !here(wanted)
    {
        return None;
    }
    let name = fields.get("Name")?;
    let command = fields.get("Exec")?;
    if name.is_empty() || command.is_empty() {
        return None;
    }
    Some(Application {
        name: name.clone(),
        command: without_field_codes(command),
        terminal: fields.get("Terminal").map(|said| said.to_lowercase()).as_deref() == Some("true"),
        icon: fields.get("Icon").cloned().unwrap_or_default(),
    })
}

/// Every application on the machine, later directories losing to earlier ones
/// by the name of the file.
pub fn files(home: &Path, data_dirs: &str) -> Vec<PathBuf> {
    let mut roots = vec![home.join(".local/share/applications")];
    roots.extend(data_dirs.split(':').filter(|dir| !dir.is_empty()).map(|dir| Path::new(dir).join("applications")));
    let mut found: BTreeMap<String, PathBuf> = BTreeMap::new();
    for root in roots {
        for path in under(&root) {
            let Some(name) = path.file_name().map(|name| name.to_string_lossy().to_string()) else {
                continue;
            };
            found.entry(name).or_insert(path);
        }
    }
    found.into_values().collect()
}

/// Every .desktop file under one directory, in order, however deep.
fn under(root: &Path) -> Vec<PathBuf> {
    let Ok(reading) = std::fs::read_dir(root) else { return Vec::new() };
    let mut found: Vec<PathBuf> = Vec::new();
    let mut names: Vec<PathBuf> = reading.filter_map(Result::ok).map(|entry| entry.path()).collect();
    names.sort();
    for path in names {
        if path.is_dir() {
            found.extend(under(&path));
        } else if path.extension().is_some_and(|kind| kind == "desktop") {
            found.push(path);
        }
    }
    found
}

/// Where a data directory is looked for, when nothing says.
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

    fn anything(_: &str) -> bool {
        true
    }

    /// The same file, with one more line in the entry itself.
    fn also(line: &str) -> String {
        SAID.replace("Terminal=false", &format!("Terminal=false\n{line}"))
    }

    #[test]
    fn an_entry_is_a_name_a_command_and_an_icon() {
        assert_eq!(
            read(SAID, anything),
            Some(Application {
                name: "Firefox".to_string(),
                command: "firefox".to_string(),
                terminal: false,
                icon: "firefox".to_string(),
            })
        );
    }

    /// The actions at the bottom of a file are other things the same
    /// application can do, and each has a Name of its own.
    #[test]
    fn only_the_entry_itself_is_read() {
        assert_eq!(read(SAID, anything).expect("firefox").name, "Firefox");
    }

    #[test]
    fn something_that_asks_not_to_be_seen_is_not_drawn() {
        for asking in ["NoDisplay=true", "Hidden=TRUE"] {
            assert_eq!(read(&also(asking), anything), None, "{asking}");
        }
    }

    #[test]
    fn something_that_is_not_an_application_is_not_drawn() {
        assert_eq!(read("[Desktop Entry]\nType=Directory\nName=Games\n", anything), None);
    }

    /// Which is how a .desktop file left behind by something uninstalled asks
    /// to be left out.
    #[test]
    fn something_that_names_a_program_this_machine_has_not_got_is_not_drawn() {
        let said = also("TryExec=firefox");
        assert_eq!(read(&said, |_| false), None);
        assert!(read(&said, anything).is_some());
    }

    #[test]
    fn an_entry_with_nothing_to_run_is_not_drawn() {
        assert_eq!(read("[Desktop Entry]\nType=Application\nName=Nothing\n", anything), None);
    }
}

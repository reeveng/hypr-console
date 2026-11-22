//! What a .desktop file says about an application.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use console_core_directory_listing::Descend;
use console_core_ini_files::{Under, fields};
use console_core_never::Never;

use crate::words::without_field_codes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Application {
  pub name: String,
  pub command: String,
  pub terminal: bool,
  pub icon: String,
}

pub use console_core_external_programs::Installed;

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
  pub categories: Option<&'a str>,
  pub startup_wm_class: Option<&'a str>,
}

impl<'a> DesktopEntry<'a> {
  pub fn read(said: &'a str) -> Result<Self, Never> {
    let fields = fields(said, Under(GROUP))?;
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
      categories: of("Categories"),
      startup_wm_class: of("StartupWMClass"),
    })
  }

  pub fn worth(&self) -> Result<Worth, Never> {
    let said = |value: Option<&str>| value.map(str::to_lowercase);
    let drawn = self.kind == Some("Application")
      && said(self.no_display).as_deref() != Some("true")
      && said(self.hidden).as_deref() != Some("true");

    Ok(match ::core::primitive::bool::then_some(drawn, ()) {
      ::core::option::Option::Some(()) => Worth::Drawing,
      ::core::option::Option::None => Worth::Skipping,
    })
  }

  pub fn says(&self) -> Result<Option<&'a str>, Never> {
    Ok(self.name.filter(|name| !name.is_empty()))
  }

  pub fn opens(&self) -> Result<Vec<String>, Never> {
    Ok(match self.mime {
      Some(mime) => mime
        .split(';')
        .filter(|kind| !kind.is_empty())
        .map(str::to_string)
        .collect(),
      None => Vec::new(),
    })
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
    Worth::Drawing => {}
  }

  match entry.try_exec {
    Some(wanted) => {
      let installed = here(wanted)?;

      match installed {
        Installed::No => return Ok(None),
        Installed::Yes => {}
      }
    }
    None => {}
  }

  let says = entry.says()?;

  let (name, command) = match (says, entry.command.filter(|command| !command.is_empty())) {
    (Some(name), Some(command)) => (name, command),
    (None, _) | (_, None) => return Ok(None),
  };

  let command_2 = without_field_codes(command)?;
  let terminal = entry.terminal.map(str::to_lowercase).as_deref() == Some("true");

  Ok(Some(Application {
    name: name.to_string(),
    command: command_2,
    terminal,
    icon: match entry.icon {
      Some(icon) => icon.to_string(),
      None => String::new(),
    },
  }))
}

pub fn serialize(application: &Application) -> Result<String, Never> {
  let one_line =
    |said: &str| -> String { said.chars().filter(|letter| !letter.is_control()).collect() };

  let terminal = match ::core::primitive::bool::then_some(application.terminal, ()) {
    ::core::option::Option::Some(()) => "true",
    ::core::option::Option::None => "false",
  };

  let icon = match ::core::primitive::bool::then_some(application.icon.is_empty(), ()) {
    ::core::option::Option::Some(()) => String::new(),
    ::core::option::Option::None => format!("Icon={}\n", one_line(&application.icon)),
  };

  Ok(format!(
    "[{GROUP}]\nType=Application\nName={}\nExec={}\n{icon}Terminal={terminal}\n",
    one_line(&application.name),
    one_line(&application.command),
  ))
}

pub fn files(roots: &[PathBuf]) -> Result<Vec<PathBuf>, Never> {
  let mut found: BTreeMap<OsString, PathBuf> = BTreeMap::new();

  for root in roots {
    let under = under(root)?;

    'over_files: for path in under {
      let name = match path.file_name().map(OsString::from) {
        Some(name) => name,
        None => continue 'over_files,
      };

      found.entry(name).or_insert(path);
    }
  }

  Ok(found.into_values().collect())
}

fn under(root: &Path) -> Result<Vec<PathBuf>, Never> {
  let Ok(listing) = console_core_directory_listing::recursive(root, |_| Descend::Into);
  let mut found: Vec<PathBuf> = listing
    .filter_map(|entry| match entry {
      Ok(path) => match path.extension() {
        Some(kind) => match bool::then_some(kind == "desktop", ()) {
          Some(()) => match path.metadata() {
            Ok(metadata) => metadata.is_file().then_some(path),
            Err(_file_no_longer_available) => None,
          },
          None => None,
        },
        None => None,
      },
      Err(_unreadable) => None,
    })
    .collect();

  found.sort_by_key(|path| (path.components().count(), path.clone()));

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

[Desktop Effect new-window]
Name=New Window
Exec=firefox --new-window
";

  fn anything(_: &str) -> Result<Installed, Never> {
    Ok(Installed::Yes)
  }

  fn also(line: &str) -> Result<String, Never> {
    Ok(SAID.replace("Terminal=false", &format!("Terminal=false\n{line}")))
  }

  #[test]
  fn an_entry_is_a_name_a_command_and_an_icon() {
    assert_eq!(
      read(SAID, anything),
      Ok(Some(Application {
        name: "Firefox".to_string(),
        command: "firefox".to_string(),
        terminal: false,
        icon: "firefox".to_string(),
      }))
    );
  }

  #[test]
  fn only_the_entry_itself_is_read() {
    let Ok(read) = read(SAID, anything);

    assert_eq!(read.map(|app| app.name), Some("Firefox".to_string()));
  }

  #[test]
  fn something_that_asks_not_to_be_seen_is_not_drawn() {
    for asking in ["NoDisplay=true", "Hidden=TRUE"] {
      let Ok(said) = also(asking);

      assert_eq!(read(&said, anything), Ok(None), "{asking}");
    }
  }

  #[test]
  fn something_that_is_not_an_application_is_not_drawn() {
    assert_eq!(
      read("[Desktop Entry]\nType=Directory\nName=Games\n", anything),
      Ok(None)
    );
  }

  #[test]
  fn something_that_names_a_program_this_machine_has_not_got_is_not_drawn() {
    let Ok(said) = also("TryExec=firefox");

    assert_eq!(read(&said, |_| Ok(Installed::No)), Ok(None));
    assert!(matches!(read(&said, anything), Ok(Some(_))));
  }

  #[test]
  fn what_is_written_is_what_is_read_back() {
    let application = Application {
      name: "Hacker News".to_string(),
      command: "xdg-open \"https://news.ycombinator.com/?q=a b\"".to_string(),
      terminal: false,
      icon: "/home/someone/.local/share/console/bookmark-icons/abc".to_string(),
    };
    let Ok(said) = serialize(&application);

    assert_eq!(read(&said, anything), Ok(Some(application)));
  }

  #[test]
  fn a_name_with_a_line_break_in_it_is_still_one_line() {
    let application = Application {
      name: "Two\nLines".to_string(),
      command: "xdg-open https://example.com".to_string(),
      terminal: false,
      icon: String::new(),
    };
    let Ok(said) = serialize(&application);

    assert_eq!(said.lines().count(), 5, "{said}");

    let Ok(read) = read(&said, anything);

    assert_eq!(read.map(|app| app.name), Some("TwoLines".to_string()));
  }

  #[test]
  fn an_entry_with_nothing_to_run_is_not_drawn() {
    assert_eq!(
      read(
        "[Desktop Entry]\nType=Application\nName=Nothing\n",
        anything
      ),
      Ok(None)
    );
  }
}

//! Folder and viewport state for each place in the Files window.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_core_arguments::{Command, Flag, Operands, Takes};
use console_core_file_urls::{path, url};
use console_core_never::Never;
use console_panel::page::PagePosition;

pub(crate) const RESUME: Flag = Flag {
  spelling: "--resume-view",
  takes: Takes::Value("STATE"),
  about: "reopen the folders and positions kept in a desktop session",
};

pub const COMMAND: Command = Command {
  name: env!("CARGO_PKG_NAME"),
  about: "the files, as an app",
  flags: &[RESUME],
  operands: Operands::Optional("FOLDER"),
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Tab {
  pub folder: PathBuf,
  pub position: PagePosition,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct View {
  pub active: Option<PathBuf>,
  pub tabs: BTreeMap<PathBuf, Tab>,
}

fn folder(value: &str) -> Result<Option<PathBuf>, Never> {
  let Ok(parsed) = path(value);
  Ok(parsed.filter(|path| path.is_absolute()))
}

impl View {
  pub fn encode(&self) -> Result<Option<String>, Never> {
    let active = match self.active.as_deref() {
      Some(active) => active,
      None => return Ok(None),
    };
    let Ok(mut text) = url(active);

    for (place, tab) in &self.tabs {
      let key = match tab.position.key.as_deref() {
        Some(selected) => {
          let Ok(encoded) = url(Path::new(selected));
          encoded
        }
        None => String::new(),
      };
      let Ok(place_url) = url(place);
      let Ok(folder_url) = url(&tab.folder);
      use std::fmt::Write;
      let _written = write!(
        text,
        "\n{place_url}\t{folder_url}\t{}\t{key}",
        tab.position.scroll
      );
    }

    Ok(Some(text))
  }

  pub fn decode(text: &str) -> Result<Option<Self>, Never> {
    let mut lines = text.lines();
    let active = match lines.next() {
      Some(line) => {
        let Ok(active) = folder(line);
        active
      }
      None => None,
    };
    let active_path = match active {
      Some(active_path) => active_path,
      None => return Ok(None),
    };
    let tabs = lines
      .filter_map(|line| {
        let mut words = line.split('\t');
        let place_field = words.next()?;
        let Ok(known_place) = folder(place_field);
        let place = known_place?;
        let folder_field = words.next()?;
        let Ok(known_folder) = folder(folder_field);
        let root = known_folder?;
        let scroll_field = words.next()?;
        let scroll = match scroll_field.parse::<i32>() {
          Ok(number) => number.max(0),
          Err(_invalid_scroll) => return None,
        };
        let key_field = words.next()?;
        let Ok(key_path) = folder(key_field);
        let key = key_path
          .filter(|selected| selected.parent() == Some(root.as_path()))
          .and_then(|selected| match selected.into_os_string().into_string() {
            Ok(key_text) => Some(key_text),
            Err(_non_utf8_path) => None,
          });
        root.starts_with(&place).then_some((
          place,
          Tab {
            folder: root,
            position: PagePosition { key, scroll },
          },
        ))
      })
      .collect();

    Ok(Some(Self {
      active: Some(active_path),
      tabs,
    }))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn each_tab_keeps_its_folder_and_highlight() {
    let home = PathBuf::from("/home/ada");
    let downloads = home.join("Downloads");
    let view = View {
      active: Some(downloads.clone()),
      tabs: BTreeMap::from([
        (
          home.clone(),
          Tab {
            folder: home.join("A project"),
            position: PagePosition {
              key: None,
              scroll: 0,
            },
          },
        ),
        (
          downloads.clone(),
          Tab {
            folder: downloads.clone(),
            position: PagePosition {
              key: Some("/home/ada/Downloads/a\nfile.txt".to_owned()),
              scroll: 240,
            },
          },
        ),
      ]),
    };

    let Ok(encoded) = view.encode();
    let decoded = match encoded {
      Some(text) => View::decode(&text),
      None => Ok(None),
    };
    assert_eq!(decoded, Ok(Some(view)));
  }
}

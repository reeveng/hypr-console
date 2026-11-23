//! Editors for file types handled by the Files and Settings panels.
//!
//! Krita also advertises formats through separate desktop entries for its import filters.

use std::path::{Path, PathBuf};

use console_applications::entry::{DesktopEntry, Worth};
use console_core_external_programs::Installed;
use console_core_never::Never;
use console_defaults::Setting;

const KRITA: &str = "org.kde.krita.desktop";
const KATE: &str = "org.kde.kate.desktop";

const KRITA_PICTURES: &[&str] = &[
  "application/x-krita",
  "image/avif",
  "image/gif",
  "image/heic",
  "image/heif",
  "image/jpeg",
  "image/png",
  "image/tiff",
  "image/webp",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
  Pictures,
  WebPages,
  Text,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Support {
  Yes,
  No,
}

impl Kind {
  pub fn of(mime: &str) -> Result<Option<Self>, Never> {
    Ok(match mime {
      "text/html" | "application/xhtml+xml" => Some(Self::WebPages),
      "application/x-krita" | "application/x-photoshop" | "application/x-gimp-image" => {
        Some(Self::Pictures)
      }
      "application/javascript"
      | "application/json"
      | "application/json5"
      | "application/toml"
      | "application/typescript"
      | "application/xml"
      | "application/yaml"
      | "application/x-shellscript" => Some(Self::Text),
      _ => match (
        mime.starts_with("image/"),
        mime.starts_with("text/") || mime.ends_with("+json") || mime.ends_with("+xml"),
      ) {
        (true, _) => Some(Self::Pictures),
        (false, true) => Some(Self::Text),
        (false, false) => None,
      },
    })
  }

  fn key(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Pictures => "picture-editor",
      Self::WebPages => "web-page-editor",
      Self::Text => "text-editor",
    })
  }

  fn shipped(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Pictures => KRITA,
      Self::Text | Self::WebPages => KATE,
    })
  }

  pub fn preferred<'a>(
    self,
    mime: &str,
    applications: &'a [Application],
  ) -> Result<Option<&'a Application>, Never> {
    let key = self.key()?;
    let saved = console_defaults::setting(key)?;

    self.preferred_from(mime, applications, saved.as_deref())
  }

  fn preferred_from<'a>(
    self,
    mime: &str,
    applications: &'a [Application],
    saved: Option<&str>,
  ) -> Result<Option<&'a Application>, Never> {
    let shipped = self.shipped()?;

    Ok([saved, Some(shipped)].into_iter().flatten().find_map(|id| {
      applications.iter().filter(|app| app.id == id).find(|app| {
        let Ok(support) = app.edits(self, mime);

        support == Support::Yes
      })
    }))
  }

  pub fn choose(self, application: &str) -> Result<(), Never> {
    let key = self.key()?;

    console_defaults::set(Setting {
      key,
      value: application,
    })
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Application {
  pub id: String,
  pub says: String,
  pub path: PathBuf,
  opens: Vec<String>,
  categories: String,
  launchable: bool,
}

impl Application {
  pub fn read(path: &Path, held: &str) -> Result<Option<Self>, Never> {
    let id = match path.file_name().and_then(|name| name.to_str()) {
      Some(id) => id,
      None => return Ok(None),
    };
    let entry = DesktopEntry::read(held)?;
    let worth = entry.worth()?;

    match worth {
      Worth::Drawing => {}
      Worth::Skipping => return Ok(None),
    }

    let name = entry.says()?;
    let says = match name {
      Some(says) => says,
      None => return Ok(None),
    };
    let opens = entry.opens()?;
    let launchable = entry.command.is_some_and(|command| !command.is_empty())
      && entry.try_exec.is_none_or(|program| {
        let Ok(installed) = console_core_external_programs::installed(program);

        installed == Installed::Yes
      });

    Ok(Some(Self {
      id: id.to_string(),
      says: says.to_string(),
      path: path.to_path_buf(),
      opens,
      categories: match entry.categories {
        Some(categories) => categories.to_string(),
        None => String::new(),
      },
      launchable,
    }))
  }

  pub fn opens(&self, mime: &str) -> Result<Support, Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.opens.iter().any(|claim| claim == mime), ()) {
        ::core::option::Option::Some(()) => Support::Yes,
        ::core::option::Option::None => Support::No,
      },
    )
  }

  pub fn edits(&self, kind: Kind, mime: &str) -> Result<Support, Never> {
    match ::core::primitive::bool::then_some(self.launchable, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(Support::No),
    }

    let Ok(opens) = self.opens(mime);
    let categories = || self.categories.split(';');

    let suitable = match kind {
      Kind::Pictures => {
        (self.id == KRITA && KRITA_PICTURES.contains(&mime))
          || (opens == Support::Yes
            && categories().any(|category| {
              matches!(
                category,
                "RasterGraphics" | "VectorGraphics" | "Photography"
              )
            })
            && !categories().any(|category| category == "Viewer"))
      }
      Kind::WebPages | Kind::Text => {
        categories().any(|category| matches!(category, "TextEditor" | "IDE"))
          && (opens == Support::Yes
            || self
              .opens
              .iter()
              .any(|claim| matches!(claim.as_str(), "text/plain" | "text/*")))
      }
    };

    Ok(match ::core::primitive::bool::then_some(suitable, ()) {
      ::core::option::Option::Some(()) => Support::Yes,
      ::core::option::Option::None => Support::No,
    })
  }
}

pub fn installed() -> Result<Vec<Application>, Never> {
  let roots = console_core_places::applications()?;
  let paths = console_applications::entry::files(&roots)?;
  let mut apps: Vec<Application> = paths
    .iter()
    .filter_map(|path| {
      let held = match std::fs::read_to_string(path) {
        Ok(held) => held,
        Err(_unreadable_desktop_entry) => return None,
      };
      let Ok(app) = Application::read(path, &held);

      app
    })
    .collect();

  apps.sort_by_key(|app| app.says.to_lowercase());

  Ok(apps)
}

pub fn choices(mime: &str) -> Result<Vec<Application>, Never> {
  let editor_kind = Kind::of(mime)?;
  let kind = match editor_kind {
    Some(kind) => kind,
    None => return Ok(Vec::new()),
  };
  let applications = installed()?;

  Ok(
    applications
      .into_iter()
      .filter(|app| {
        let Ok(support) = app.edits(kind, mime);

        support == Support::Yes
      })
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Debug)]
  enum FixtureError {
    NotAnApplication,
  }

  fn app((id, fields): (&str, &str)) -> Result<Application, FixtureError> {
    let held = format!("[Desktop Entry]\nType=Application\nName={id}\nExec=editor %f\n{fields}");
    let Ok(app) = Application::read(Path::new(id), &held);

    app.ok_or(FixtureError::NotAnApplication)
  }

  #[test]
  fn pictures_use_the_shipped_editor_when_the_saved_one_cannot_edit_this_format()
  -> Result<(), FixtureError> {
    let krita = app((
      KRITA,
      "Categories=Graphics;\nMimeType=application/x-krita;\n",
    ))?;
    let png = app((
      "png.desktop",
      "Categories=RasterGraphics;\nMimeType=image/png;\n",
    ))?;
    let viewer = app((
      "viewer.desktop",
      "Categories=Viewer;RasterGraphics;\nMimeType=image/png;\n",
    ))?;
    let applications = [krita, png, viewer];
    let chosen = |mime, saved| {
      let Ok(preferred) = Kind::Pictures.preferred_from(mime, &applications, saved);

      preferred.map(|app| app.id.as_str())
    };

    assert_eq!(chosen("image/png", None), Some(KRITA));
    assert_eq!(
      chosen("image/png", Some("png.desktop")),
      Some("png.desktop")
    );
    assert_eq!(chosen("image/jpeg", Some("png.desktop")), Some(KRITA));
    assert_eq!(chosen("image/png", Some("removed.desktop")), Some(KRITA));
    assert_eq!(chosen("image/png", Some("viewer.desktop")), Some(KRITA));

    Ok(())
  }

  #[test]
  fn a_plain_text_editor_can_edit_html_and_code() -> Result<(), FixtureError> {
    let kate = app((
      KATE,
      "Categories=Development;TextEditor;\nMimeType=text/plain;\n",
    ))?;
    let browser = app((
      "browser.desktop",
      "Categories=WebBrowser;\nMimeType=text/html;\n",
    ))?;
    let applications = [kate, browser];

    for (mime, kind) in [
      ("text/html", Kind::WebPages),
      ("application/xhtml+xml", Kind::WebPages),
      ("text/rust", Kind::Text),
    ] {
      assert_eq!(Kind::of(mime), Ok(Some(kind)));

      let Ok(preferred) = kind.preferred_from(mime, &applications, None);

      assert_eq!(preferred.map(|app| app.id.as_str()), Some(KATE));
    }

    Ok(())
  }
}

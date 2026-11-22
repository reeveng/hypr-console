//! What applications this machine has, and how to start one.
//!
//! The menu read all of this inside its own program for as long as it was the
//! only thing that wanted it. The home screen wants the same answer: it draws
//! a few of these on the wallpaper, and the card that puts one there lists
//! every one of them. Two programs reading the desktop files two ways would be
//! two answers to one question, and the second of them would be wrong in some
//! way no one had thought about -- a Steam icon found here and not there, a
//! terminal program run without its terminal.
//!
//! So it is here, once, and the menu is one of the callers rather than the
//! owner.
//!
//! An application whose icon cannot be found is given the theme's picture of
//! a program rather than none, because a square with nothing in it reads as
//! a hole in the home screen rather than as something to press.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use console_core_external_programs::{Program, installed};
use console_core_never::Never;
use console_core_places::Base;

use crate::entry::Application;
use crate::icons::{FALLBACKS, SteamAppId, UNPICTURED};
use crate::{cache, counts, entry, icons, words};

const HEAD: u32 = 65_536;

pub fn counts_at() -> Result<Option<PathBuf>, Never> {
  let ours = Base::State.ours()?;

  Ok(ours.map(|ours_2| ours_2.join("menu-counts")))
}

fn index_at() -> Result<Option<PathBuf>, Never> {
  let ours = Base::Cache.ours()?;

  Ok(ours.map(|ours_2| ours_2.join("icon-index")))
}

fn kept_at() -> Result<Option<PathBuf>, Never> {
  let ours = Base::Cache.ours()?;

  Ok(ours.map(|ours_2| ours_2.join("menu-apps")))
}

fn icon_roots() -> Result<Vec<PathBuf>, Never> {
  let home = console_core_places::home()?;

  let share = Base::Share.user()?;

  let mut roots = vec![
    PathBuf::from("/usr/share/icons"),
    PathBuf::from("/usr/share/pixmaps"),
  ];

  roots.extend(share.into_iter().map(|share_2| share_2.join("icons")));
  roots.extend(home.into_iter().map(|home_2| home_2.join(".icons")));

  Ok(roots)
}

fn steam_roots() -> Result<Vec<PathBuf>, Never> {
  let home = console_core_places::home()?;

  let share = Base::Share.user()?;

  Ok(
    share
      .into_iter()
      .map(|share_2| share_2.join("Steam"))
      .chain(home.into_iter().map(|home_2| home_2.join(".steam/steam")))
      .collect(),
  )
}

fn icons_changed_at() -> Result<std::time::SystemTime, Never> {
  let mut newest = std::time::UNIX_EPOCH;

  let roots = icon_roots()?;

  for root in roots {
    let about = match root.metadata() {
      Ok(about) => about,
      Err(_unreadable) => continue,
    };

    newest = newest.max(match about.modified() {
      Ok(when) => when,
      Err(_no_modified_time) => std::time::UNIX_EPOCH,
    });

    let reading = match std::fs::read_dir(&root) {
      Ok(reading) => reading,
      Err(_unreadable) => continue,
    };

    'over_child_2: for child in reading.filter_map(Result::ok) {
      match child.metadata() {
        Ok(about_2) => {
          newest = newest.max(match about_2.modified() {
            Ok(when) => when,
            Err(_no_modified_time) => std::time::UNIX_EPOCH,
          });
        }
        Err(_the_child_went_away) => continue 'over_child_2,
      }
    }
  }

  Ok(newest)
}

fn index() -> Result<BTreeMap<String, String>, Never> {
  let at = index_at()?;

  let changed = icons_changed_at()?;

  let kept = match at.as_ref() {
    Some(at_2) => match at_2.metadata().and_then(|about| about.modified()) {
      Ok(written) => written >= changed,
      Err(_index_timestamp_unavailable) => false,
    },
    None => false,
  };

  match ::core::primitive::bool::then_some(kept, ()) {
    ::core::option::Option::Some(()) => match at.as_ref().map(std::fs::read_to_string) {
      Some(Ok(said)) => {
        let held = icons::read(&said)?;

        return Ok(held);
      }
      Some(Err(fault)) => {
        console_core_logging::debug!("rebuilding the unreadable icon index: {fault}");
      }
      None => {}
    },
    ::core::option::Option::None => {}
  }

  let roots = icon_roots()?;

  let built = icons::built(&roots)?;

  match at {
    Some(at_3) => {
      let said = icons::serialize(&built)?;
      remember(&at_3, &said)?;
    }
    None => {}
  }

  Ok(built)
}

fn steam_icon(appid: SteamAppId<'_>) -> Result<Option<String>, Never> {
  let mut fallbacks: BTreeMap<String, String> = BTreeMap::new();

  let roots = steam_roots()?;

  for root in roots {
    let cache = root.join("appcache/librarycache").join(&appid);

    let reading = match std::fs::read_dir(&cache) {
      Ok(reading) => reading,
      Err(_unreadable) => continue,
    };

    let mut paths: Vec<PathBuf> = reading
      .filter_map(Result::ok)
      .map(|entry| entry.path())
      .collect();
    paths.sort();

    'over_pictures: for path in paths {
      let suffix = path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_lowercase);

      match suffix.as_deref() {
        Some("jpg" | "png") => {}
        Some(_) | None => continue 'over_pictures,
      }

      let written = match path.to_str() {
        Some(written) => written,
        None => continue 'over_pictures,
      };

      let head = match read_head(&path) {
        Ok(head) => head,
        Err(_unreadable) => continue 'over_pictures,
      };

      let size = match console_core_picture_sizes::size(&head) {
        Ok(size) => size,
        Err(_unmeasured) => continue 'over_pictures,
      };

      match ::core::primitive::bool::then_some(size.width == size.height, ()) {
        ::core::option::Option::Some(()) => return Ok(Some(written.to_owned())),
        ::core::option::Option::None => {}
      }

      let name = match path.file_name().and_then(OsStr::to_str) {
        Some(name) => name,
        None => continue 'over_pictures,
      };

      fallbacks
        .entry(name.to_owned())
        .or_insert_with(|| written.to_owned());
    }
  }

  Ok(
    FALLBACKS
      .iter()
      .find_map(|wanted| fallbacks.get(*wanted).cloned()),
  )
}

fn read_head(path: &Path) -> std::io::Result<Vec<u8>> {
  let resolved = path.canonicalize()?;

  console_core_atomic_writes::read_prefix(&resolved, HEAD)
}

fn icon_at(name: &str, index: &BTreeMap<String, String>) -> Result<Option<String>, Never> {
  match ::core::primitive::bool::then_some(name.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(None),
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(name.starts_with('/'), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(match std::fs::metadata(name) {
        Ok(about) => about.is_file().then(|| name.to_owned()),
        Err(_icon_unavailable) => None,
      });
    }
    ::core::option::Option::None => {}
  }

  match index.get(name) {
    Some(found) => return Ok(Some(found.clone())),
    None => {}
  }

  let appid = SteamAppId::from_icon(name)?;

  let appid_3 = match appid {
    Some(appid_2) => appid_2,
    None => return Ok(None),
  };

  steam_icon(appid_3)
}

fn icon_or_default(
  found: Option<String>,
  index: &BTreeMap<String, String>,
) -> Result<Option<String>, Never> {
  Ok(found.or_else(|| index.get(UNPICTURED).cloned()))
}

pub struct Found {
  pub applications: BTreeMap<String, Application>,
  pub icon: BTreeMap<String, String>,
}

pub fn machine() -> Result<Found, Never> {
  let index = index()?;

  let mut applications: BTreeMap<String, Application> = BTreeMap::new();
  let mut icon: BTreeMap<String, String> = BTreeMap::new();

  let roots = console_core_places::applications()?;

  let files = entry::files(&roots)?;

  for path in files {
    let said = match std::fs::read_to_string(&path) {
      Ok(said) => said,
      Err(_unreadable) => continue,
    };

    let application = entry::read(&said, installed)?;

    let application_3 = match application {
      Some(application_2) => application_2,
      None => continue,
    };

    match ::core::primitive::bool::then_some(applications.contains_key(&application_3.name), ()) {
      ::core::option::Option::Some(()) => continue,
      ::core::option::Option::None => {}
    }

    let found = icon_at(&application_3.icon, &index)?;
    let found_2 = icon_or_default(found, &index)?;

    match found_2 {
      Some(found_3) => {
        icon.insert(application_3.name.clone(), found_3);
      }
      None => {}
    }

    applications.insert(application_3.name.clone(), application_3);
  }

  keep(&applications, &icon)?;

  Ok(Found { applications, icon })
}

pub fn quickly() -> Result<Found, Never> {
  let mut applications: BTreeMap<String, Application> = BTreeMap::new();

  let roots = console_core_places::applications()?;

  let files = entry::files(&roots)?;

  for path in files {
    let said = match std::fs::read_to_string(&path) {
      Ok(said) => said,
      Err(_unreadable) => continue,
    };

    let application = entry::read(&said, installed)?;

    let application_3 = match application {
      Some(application_2) => application_2,
      None => continue,
    };

    applications
      .entry(application_3.name.clone())
      .or_insert(application_3);
  }

  Ok(Found {
    applications,
    icon: BTreeMap::new(),
  })
}

fn said_at(at: Option<PathBuf>) -> Result<String, Never> {
  let at_3 = match at {
    Some(at_2) => at_2,
    None => return Ok(String::new()),
  };

  match console_core_atomic_writes::text_or_empty(&at_3) {
    Ok(said) => Ok(said),
    Err(fault) => {
      console_core_logging::warn!("could not read saved applications or counts: {fault}");

      Ok(String::new())
    }
  }
}

pub fn load_cached() -> Result<Found, Never> {
  let mut applications: BTreeMap<String, Application> = BTreeMap::new();
  let mut icon: BTreeMap<String, String> = BTreeMap::new();

  let at = kept_at()?;

  let remembered = said_at(at)?;

  let held = cache::read(&remembered)?;

  for held_2 in held {
    match ::core::primitive::bool::then_some(held_2.picture.is_empty(), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        icon.insert(held_2.application.name.clone(), held_2.picture);
      }
    }

    applications.insert(held_2.application.name.clone(), held_2.application);
  }

  Ok(Found { applications, icon })
}

fn keep(
  applications: &BTreeMap<String, Application>,
  icon: &BTreeMap<String, String>,
) -> Result<(), Never> {
  let held = kept_at()?;

  let at = match held {
    Some(at) => at,
    None => return Ok(()),
  };

  let said = cache::serialize(applications, icon)?;

  match std::fs::read_to_string(&at) {
    Ok(before) => match before.cmp(&said) {
      core::cmp::Ordering::Equal => return Ok(()),
      core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {}
    },
    Err(fault) => console_core_logging::debug!("refreshing the saved application list: {fault}"),
  }

  remember(&at, &said)
}

fn remember(at: &Path, said: &str) -> Result<(), Never> {
  match console_core_atomic_writes::whole_with_folders(at, said.as_bytes()) {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("could not keep {}: {fault}", at.display()),
  }

  Ok(())
}

pub fn load_counts() -> Result<BTreeMap<String, u64>, Never> {
  let at = counts_at()?;

  let said = said_at(at)?;

  counts::read(&said)
}

pub fn command(application: &Application) -> Result<Option<Vec<String>>, Never> {
  bump(&application.name)?;

  let words = words::split(&application.command)?;

  let arguments = match words {
    Some(arguments) => arguments,
    None => {
      console_core_logging::warn!(
        "{}: {:?} is not a command",
        application.name, application.command
      );

      return Ok(None);
    }
  };

  let launched = match ::core::primitive::bool::then_some(application.terminal, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(alacritty) = Program::Alacritty.name();

      let mut terminal = vec![alacritty.to_string(), "-e".to_string()];
      terminal.extend(arguments);
      terminal
    }
    ::core::option::Option::None => arguments,
  };

  console_core_logging::info!("chose {}: {}", application.name, launched.join(" "));

  Ok(Some(launched))
}

pub fn bump(name: &str) -> Result<(), Never> {
  let held = counts_at()?;

  let at = match held {
    Some(at) => at,
    None => return Ok(()),
  };

  let counted = load_counts()?;

  let bumped = counts::increment(counted, name)?;

  let said = counts::serialize(&bumped)?;

  remember(&at, &said)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn index() -> Result<BTreeMap<String, String>, Never> {
    Ok(BTreeMap::from([
      ("firefox".to_string(), "/icons/firefox.png".to_string()),
      (UNPICTURED.to_string(), "/icons/program.png".to_string()),
    ]))
  }

  #[test]
  fn an_application_with_no_icon_file_is_given_the_picture_of_a_program() {
    let Ok(index) = index();

    assert_eq!(
      icon_or_default(None, &index),
      Ok(Some("/icons/program.png".to_string()))
    );
  }

  #[test]
  fn an_application_whose_icon_was_found_keeps_it() {
    let found = Some("/icons/firefox.png".to_string());
    let Ok(index) = index();

    assert_eq!(icon_or_default(found.clone(), &index), Ok(found));
  }
}

//! Common package catalogue and transactions for the supported managers.

use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

use console_core_external_programs::Installed;
use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::user::{self, Installable, Profile, ProfileEntry};
use crate::{flatpak, system, tools};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Manager {
  Pacman,
  Flatpak,
  Mise,
  Nix,
  Aur,
}

impl Manager {
  pub const ALL: [Self; 5] = [
    Self::Pacman,
    Self::Flatpak,
    Self::Mise,
    Self::Nix,
    Self::Aur,
  ];

  pub fn label(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Pacman => "pacman",
      Self::Flatpak => "Flatpak",
      Self::Mise => "mise",
      Self::Nix => "Nixpkgs",
      Self::Aur => "AUR",
    })
  }

  pub fn availability(self) -> Result<Installed, Never> {
    match self {
      Self::Nix => user::availability(),
      Self::Pacman => system::applications::availability(),
      Self::Flatpak => flatpak::availability(),
      Self::Mise => tools::applications::availability(),
      Self::Aur => system::applications::aur_availability(),
    }
  }

  pub fn search_remote(self, query: &str) -> io::Result<Matches> {
    match self {
      Self::Nix => {
        let packages = user::search(query)?;
        let Ok(results) = nix_results(packages, query);

        Ok(results)
      }
      Self::Aur => aur_results(query),
      Self::Pacman | Self::Flatpak | Self::Mise => Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "This manager uses the cached catalogue.",
      )),
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
  Nix(Installable),
  Pacman(String),
  Aur(String),
  Flatpak(flatpak::Reference),
  Mise {
    tool: String,
    version: Option<String>,
  },
}

impl Source {
  pub fn name(&self) -> Result<&str, Never> {
    Ok(match self {
      Self::Nix(package) => &package.attribute,
      Self::Pacman(name) | Self::Aur(name) => name,
      Self::Flatpak(package) => &package.id,
      Self::Mise { tool, .. } => tool,
    })
  }

  pub fn manager(&self) -> Result<Manager, Never> {
    Ok(match self {
      Self::Nix(_) => Manager::Nix,
      Self::Pacman(_) => Manager::Pacman,
      Self::Aur(_) => Manager::Aur,
      Self::Flatpak(_) => Manager::Flatpak,
      Self::Mise { .. } => Manager::Mise,
    })
  }

  pub fn label(&self) -> Result<&'static str, Never> {
    match self {
      Self::Nix(_) => Ok("Nix"),
      Self::Flatpak(reference) => Ok(match reference.installation {
        flatpak::Installation::User => "Flatpak (user)",
        flatpak::Installation::System => "Flatpak (system)",
      }),
      Self::Pacman(_) | Self::Aur(_) | Self::Mise { .. } => {
        let Ok(manager) = self.manager();
        manager.label()
      }
    }
  }

  pub fn install(
    &self,
    profile: &Profile,
    terminal: impl FnOnce() -> io::Result<Command>,
  ) -> io::Result<()> {
    match self {
      Self::Nix(installable) => profile.install(installable),
      Self::Pacman(name) => {
        system::applications::change(name, system::applications::Change::Install)
      }
      Self::Aur(name) => {
        let command = terminal()?;
        system::applications::aur_install(name, command)
      }
      Self::Flatpak(reference) => reference.install(),
      Self::Mise { tool, .. } => tools::applications::install(tool),
    }
  }

  pub fn remove(&self, profile: &Profile, entry: Option<&ProfileEntry>) -> io::Result<()> {
    match (self, entry) {
      (Self::Nix(_), Some(profile_entry)) => profile.remove(profile_entry),
      (Self::Nix(_), None) => Err(io::Error::other("No Nix profile entry was found.")),
      (Self::Pacman(name) | Self::Aur(name), _) => {
        system::applications::change(name, system::applications::Change::Remove)
      }
      (Self::Flatpak(reference), _) => reference.remove(),
      (
        Self::Mise {
          tool,
          version: Some(version),
        },
        _,
      ) => tools::applications::remove(tool, tools::applications::Version(version)),
      (Self::Mise { version: None, .. }, _) => {
        Err(io::Error::other("No installed mise version was selected."))
      }
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
  pub source: Source,
  pub title: String,
  pub description: String,
  pub version: String,
}

#[derive(Default)]
pub struct Matches {
  pub packages: Vec<Package>,
  pub total: u32,
}

pub struct Registered {
  pub package: Package,
  pub launcher: Option<PathBuf>,
}

#[derive(Default)]
pub struct Catalogue {
  managers: Vec<Manager>,
  pub installed: Vec<Registered>,
  cached: BTreeMap<Manager, Vec<(String, Package)>>,
  arch_packages: BTreeSet<String>,
}

impl Catalogue {
  pub fn detecting() -> Result<Self, Never> {
    let managers = Manager::ALL
      .into_iter()
      .filter(|source| {
        let Ok(available) = source.availability();

        available == Installed::Yes
      })
      .collect();

    Ok(Self {
      managers,
      ..Self::default()
    })
  }

  pub fn read() -> Result<Self, Never> {
    let Ok(mut catalogue) = Self::detecting();

    for manager in catalogue.managers.clone() {
      match manager {
        Manager::Pacman => {
          let Ok(()) = arch(&mut catalogue);
        }
        Manager::Flatpak => {
          let Ok(()) = flatpaks(&mut catalogue);
        }
        Manager::Mise => {
          let Ok(()) = mise_tools(&mut catalogue);
        }
        Manager::Nix | Manager::Aur => {}
      }
    }

    Ok(catalogue)
  }

  pub fn managers(&self) -> Result<&[Manager], Never> {
    Ok(&self.managers)
  }

  pub fn remote_managers(&self) -> Result<impl Iterator<Item = Manager> + '_, Never> {
    Ok(
      self
        .managers
        .iter()
        .copied()
        .filter(|manager| matches!(manager, Manager::Nix | Manager::Aur)),
    )
  }

  pub fn search(&self, manager: Manager, query: &str) -> io::Result<Matches> {
    match manager {
      Manager::Nix | Manager::Aur => manager.search_remote(query),
      Manager::Pacman | Manager::Flatpak | Manager::Mise => {
        let Ok(results) = self.cached_matches(query, Some(manager));

        Ok(results)
      }
    }
  }

  pub fn cached_matches(&self, query: &str, manager: Option<Manager>) -> Result<Matches, Never> {
    match query.trim() {
      "" => return Ok(Matches::default()),
      _ => {}
    }

    let lower = query.to_lowercase();
    let words: Vec<_> = lower.split_whitespace().collect();
    let candidates: Vec<_> = self
      .cached
      .iter()
      .filter(|(source, _)| manager.is_none_or(|requested| **source == requested))
      .flat_map(|(_, packages)| packages)
      .filter(|(keywords, _)| words.iter().all(|word| keywords.contains(word)))
      .map(|(_, package)| package)
      .collect();
    rank(candidates, query)
  }

  pub fn includes(&self, package: &Package) -> Result<Option<()>, Never> {
    let Ok(manager) = package.source.manager();

    Ok(
      (self.managers.contains(&manager)
        && match &package.source {
          Source::Aur(name) => !self.arch_packages.contains(name),
          Source::Nix(_) | Source::Pacman(_) | Source::Flatpak(_) | Source::Mise { .. } => true,
        })
      .then_some(()),
    )
  }

  fn index(&mut self, package: Package) -> Result<(), Never> {
    let Ok(name) = package.source.name();
    let Ok(manager) = package.source.manager();
    let keywords = format!("{name} {} {}", package.title, package.description).to_lowercase();
    self
      .cached
      .entry(manager)
      .or_default()
      .push((keywords, package));

    Ok(())
  }
}

pub fn name_in_profile(entry: &ProfileEntry) -> Result<Package, Never> {
  let Ok(installable) = Installable::nixpkgs(&entry.attribute);

  Ok(Package {
    source: Source::Nix(installable),
    title: entry.attribute.clone(),
    description: String::new(),
    version: String::new(),
  })
}

fn aur_results(query: &str) -> io::Result<Matches> {
  match (query.chars().count() >= 3).then_some(()) {
    Some(()) => {}
    None => return Ok(Matches::default()),
  }

  let words: Vec<_> = query.split_whitespace().collect();
  let names = system::applications::aur_search(&words)?;
  let Ok(installed) = system::installed();
  let mut arch: BTreeSet<_> = installed.into_iter().collect();

  match system::applications::repository_versions() {
    Ok(available) => arch.extend(available.into_keys()),
    Err(fault) => console_core_logging::debug!(
      "could not compare AUR results with pacman repositories: {fault}"
    ),
  }

  let packages: Vec<_> = names
    .into_iter()
    .filter(|name| !arch.contains(name))
    .map(|name| Package {
      source: Source::Aur(name.clone()),
      title: name,
      description: String::new(),
      version: String::new(),
    })
    .collect();
  let Ok(results) = rank(packages, query);

  Ok(results)
}

fn nix_results(entries: Vec<user::SearchMatch>, query: &str) -> Result<Matches, Never> {
  let packages: Vec<_> = entries
    .into_iter()
    .map(|entry| Package {
      source: Source::Nix(entry.installable),
      title: entry.name,
      description: entry.description,
      version: entry.version,
    })
    .collect();

  rank(packages, query)
}

fn rank<T: Borrow<Package>>(mut packages: Vec<T>, query: &str) -> Result<Matches, Never> {
  let lower = query.to_lowercase();
  packages.sort_by_cached_key(|package| {
    let row = package.borrow();
    let Ok(name) = row.source.name();
    let id = name.to_lowercase();
    let title = row.title.to_lowercase();

    (
      title != lower && id != lower,
      !title.starts_with(&lower) && !id.starts_with(&lower),
      title,
      id,
    )
  });
  let Ok(total) = fitted::<_, u32>(packages.len());
  packages.truncate(150);

  Ok(Matches {
    packages: packages
      .iter()
      .map(|package| package.borrow().clone())
      .collect(),
    total,
  })
}

struct Details {
  title: String,
  description: String,
  version: String,
}

fn package(source: Source, details: Details) -> Result<Package, Never> {
  Ok(Package {
    source,
    title: details.title,
    description: details.description,
    version: details.version,
  })
}

fn desktop_fields(entry: &str) -> Result<BTreeMap<&str, &str>, Never> {
  Ok(
    entry
      .lines()
      .skip_while(|line| line.trim() != "[Desktop Entry]")
      .skip(1)
      .take_while(|line| !line.trim_start().starts_with('['))
      .filter_map(|line| line.trim().split_once('='))
      .collect(),
  )
}

fn arch(external: &mut Catalogue) -> Result<(), Never> {
  let versions = match system::applications::installed_versions() {
    Ok(versions) => versions,
    Err(fault) => {
      console_core_logging::warn!("could not read pacman inventory: {fault}");
      BTreeMap::new()
    }
  };
  let Ok(explicit) = system::explicit();
  external.arch_packages.extend(versions.keys().cloned());
  let mut names: BTreeSet<_> = explicit.into_iter().collect();
  let Ok(mut desktops) = desktop_apps();
  names.extend(desktops.keys().cloned());

  for name in names {
    let version = match versions.get(&name) {
      Some(version) => version.clone(),
      None => continue,
    };
    let source = Source::Pacman(name.clone());
    let (title, description, launcher) = match desktops.remove(&name) {
      Some(app) => (app.title, app.description, Some(app.path)),
      None => (name, String::new(), None),
    };
    let Ok(package) = package(
      source,
      Details {
        title,
        description,
        version,
      },
    );
    let Ok(()) = external.index(package.clone());
    external.installed.push(Registered { package, launcher });
  }

  match system::applications::repository_versions() {
    Ok(available) => {
      external.arch_packages.extend(available.keys().cloned());

      for (name, version) in available {
        match versions.contains_key(&name).then_some(()) {
          Some(()) => continue,
          None => {}
        }

        let Ok(package) = package(
          Source::Pacman(name.clone()),
          Details {
            title: name,
            description: String::new(),
            version,
          },
        );
        let Ok(()) = external.index(package);
      }
    }
    Err(fault) => console_core_logging::warn!("could not read pacman repositories: {fault}"),
  }

  Ok(())
}

struct Desktop {
  title: String,
  description: String,
  path: std::path::PathBuf,
}

fn desktop_apps() -> Result<BTreeMap<String, Desktop>, Never> {
  let mut apps = BTreeMap::new();
  let files = match system::applications::desktop_entries() {
    Ok(files) => files,
    Err(fault) => {
      console_core_logging::warn!("could not read pacman desktop entries: {fault}");
      return Ok(apps);
    }
  };

  for (owner, path) in files {
    let text = match fs::read_to_string(&path) {
      Ok(text) => text,
      Err(fault) => {
        console_core_logging::debug!("{}: {fault}", path.display());
        continue;
      }
    };
    let Ok(fields) = desktop_fields(&text);
    let title = match fields.get("Name") {
      Some(title) => title,
      None => continue,
    };

    match (fields.get("Type") == Some(&"Application")
      && !["Hidden", "NoDisplay", "Terminal"]
        .iter()
        .any(|key| fields.get(key) == Some(&"true")))
    .then_some(())
    {
      Some(()) => {
        let description = match fields.get("Comment") {
          Some(comment) => (*comment).to_owned(),
          None => String::new(),
        };
        apps.entry(owner).or_insert(Desktop {
          title: (*title).to_owned(),
          description,
          path,
        });
      }
      None => {}
    }
  }

  Ok(apps)
}

fn flatpaks(external: &mut Catalogue) -> Result<(), Never> {
  for installation in [flatpak::Installation::User, flatpak::Installation::System] {
    let installed = match flatpak::applications(installation, flatpak::Listing::Installed) {
      Ok(installed) => installed,
      Err(fault) => {
        console_core_logging::warn!("could not read installed Flatpaks: {fault}");
        continue;
      }
    };
    let local: BTreeSet<_> = installed
      .iter()
      .map(|app| (&app.reference.id, &app.reference.branch))
      .collect();

    match flatpak::applications(installation, flatpak::Listing::Available) {
      Ok(available) => {
        let uninstalled = available
          .into_iter()
          .filter(|app| !local.contains(&(&app.reference.id, &app.reference.branch)));

        for app in uninstalled {
          let Ok(package) = flatpak_package(app);
          let Ok(()) = external.index(package);
        }
      }
      Err(fault) => console_core_logging::warn!("could not read Flatpak catalogue: {fault}"),
    }

    for app in installed {
      let Ok(package) = flatpak_package(app);
      let Ok(()) = external.index(package.clone());
      external.installed.push(Registered {
        package,
        launcher: None,
      });
    }
  }

  Ok(())
}

fn flatpak_package(app: flatpak::Application) -> Result<Package, Never> {
  let detail = format!("{} · {}", app.reference.remote, app.reference.branch);
  package(
    Source::Flatpak(app.reference),
    Details {
      title: app.title,
      description: detail,
      version: app.version,
    },
  )
}

fn mise_tools(external: &mut Catalogue) -> Result<(), Never> {
  let mut known = BTreeSet::new();

  match tools::applications::installed_versions() {
    Ok(installed) => {
      for (tool, version) in installed {
        let _new = known.insert(tool.clone());
        let source = Source::Mise {
          tool: tool.clone(),
          version: Some(version.clone()),
        };
        let Ok(package) = package(
          source,
          Details {
            title: tool,
            description: String::new(),
            version,
          },
        );
        let Ok(()) = external.index(package.clone());
        external.installed.push(Registered {
          package,
          launcher: None,
        });
      }
    }
    Err(fault) => console_core_logging::warn!("could not read installed mise tools: {fault}"),
  }

  match tools::applications::registry() {
    Ok(available) => {
      for tool in available {
        match known.contains(&tool).then_some(()) {
          Some(()) => continue,
          None => {}
        }

        let source = Source::Mise {
          tool: tool.clone(),
          version: None,
        };
        let Ok(package) = package(
          source,
          Details {
            title: tool,
            description: String::new(),
            version: String::new(),
          },
        );
        let Ok(()) = external.index(package);
      }
    }
    Err(fault) => console_core_logging::warn!("could not read mise registry: {fault}"),
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn providers_return_the_same_result_type_and_keep_their_package_identity() -> io::Result<()> {
    let mut catalogue = Catalogue::default();
    let pacman = Package {
      source: Source::Pacman("zen-browser-bin".to_owned()),
      title: "Zen Browser".to_owned(),
      description: String::new(),
      version: "1.24.2b-1".to_owned(),
    };
    let flatpak = Package {
      source: Source::Flatpak(flatpak::Reference {
        id: "app.zen_browser.zen".to_owned(),
        branch: "stable".to_owned(),
        remote: "flathub".to_owned(),
        installation: flatpak::Installation::User,
      }),
      title: "Zen Browser".to_owned(),
      description: String::new(),
      version: "1.24.2b".to_owned(),
    };
    let Ok(()) = catalogue.index(pacman.clone());
    let Ok(()) = catalogue.index(flatpak.clone());
    let system = catalogue.search(Manager::Pacman, "zen")?;
    let user = catalogue.search(Manager::Flatpak, "zen")?;
    let Ok(all) = catalogue.cached_matches("zen browser", None);

    assert_eq!(system.packages, [pacman]);
    assert_eq!(user.packages, [flatpak]);
    assert_eq!(all.total, 2);

    Ok(())
  }
}

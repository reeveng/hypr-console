//! Ownership checks against the current source and shipped configuration.
//!
//! The process audit checks direct literal arguments to process construction
//! and forwarding. Nix package declarations can name tools without running
//! them. Some test fixtures intentionally name the commands they exercise,
//! including the notification integration check's freedesktop tools. The
//! stock session helper and Console's installed binary share a basename.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use cargo_metadata::MetadataCommand;
use console_core_directory_listing::Descend;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_repository::sources::{self, Spelled, Word};

type Failure = Box<dyn std::error::Error>;

const OWNERS: &[(&str, &[Program])] = &[
  (
    "console-default-applications",
    &[Program::XdgOpen, Program::XdgMime, Program::XdgSettings],
  ),
  ("console-gio", &[Program::Gio]),
  (
    "console-compositor",
    &[Program::Hyprctl, Program::Hyprland, Program::StartHyprland],
  ),
  (
    "console-package-manager-pacman",
    &[Program::Pacman, Program::Yay],
  ),
  ("console-package-manager-flatpak", &[Program::Flatpak]),
  ("console-package-manager-nix", &[Program::Nix]),
  ("console-package-manager-mise", &[Program::Mise]),
  (
    "console-session-cachyos",
    &[
      Program::Steam,
      Program::SteamSetSession,
      Program::StockSessionSelect,
    ],
  ),
  ("console-boot-entries", &[Program::Bootctl]),
  ("console-operating-system-linux", &[Program::NotifySend]),
  ("console-bus", &[Program::Busctl, Program::Gdbus]),
];

const INLINE_TEST_MODULES: &[&str] = &[
  "console-core-external-programs/src/lib.rs",
  "console-panel/src/tasks.rs",
  "console-recovery/src/menu.rs",
];

const NOTIFICATION_CHECK: &str = "console-test-checks/src/notifications.rs";

struct OwnedProgram {
  owner: &'static str,
  program: Program,
  name: &'static str,
  qualified: String,
  aliased: String,
}

fn references() -> Result<Vec<OwnedProgram>, Never> {
  Ok(
    OWNERS
      .iter()
      .flat_map(|(owner, programs)| {
        programs.iter().map(move |program| {
          let Ok(name) = program.name();
          OwnedProgram {
            owner,
            program: *program,
            name,
            qualified: format!("Program::{program:?}"),
            aliased: format!("ExternalProgram::{program:?}"),
          }
        })
      })
      .collect(),
  )
}

struct CommandWords<'a> {
  exact: BTreeSet<&'a str>,
  basenames: BTreeSet<&'a str>,
}

impl<'a> CommandWords<'a> {
  fn from_words(words: impl IntoIterator<Item = &'a str>) -> Result<Self, Never> {
    let exact: BTreeSet<_> = words.into_iter().collect();
    let basenames = exact
      .iter()
      .filter_map(|word| word.rsplit('/').next())
      .collect();

    Ok(Self { exact, basenames })
  }

  fn names(&self, wanted: &OwnedProgram) -> Result<Spelled, Never> {
    let words = match (wanted.program == Program::StockSessionSelect).then_some(()) {
      Some(()) => &self.exact,
      None => &self.basenames,
    };
    let basename = wanted.name.rsplit('/').next();
    let found =
      self.exact.contains(wanted.name) || basename.is_some_and(|word| words.contains(word));

    Ok(match found.then_some(()) {
      Some(()) => Spelled::Yes,
      None => Spelled::No,
    })
  }
}

#[test]
fn only_the_backends_use_their_external_program_variants() -> Result<(), Failure> {
  let root = console_repository::root()?;
  let packages = root.join("crates");
  let Ok(references) = references();
  let without_tests: BTreeSet<_> = INLINE_TEST_MODULES.iter().map(Path::new).collect();
  let mut violations = BTreeSet::new();
  let crates = std::fs::read_dir(&packages)?;

  for entry in crates {
    let directory = entry?;
    let at = directory.path();
    let package = directory.file_name();
    let Ok(files) = sources::under(&at.join("src"));

    for source in files {
      let relative = source.strip_prefix(&packages)?;
      let text = std::fs::read_to_string(&source)?;
      let live = match without_tests.contains(relative).then_some(()) {
        Some(()) => text
          .split_once("\nmod tests {")
          .map_or(text.as_str(), |(code, _)| code),
        None => text.as_str(),
      };
      let checked = references.iter().filter(|owned| {
        let notification_fixture = relative == Path::new(NOTIFICATION_CHECK)
          && matches!(owned.program, Program::NotifySend | Program::Gdbus);
        !notification_fixture && package != owned.owner
      });
      let Ok(found) = program_violations(relative, live, checked);
      violations.extend(found);
    }
  }

  assert!(violations.is_empty(), "{violations:#?}");

  Ok(())
}

fn program_violations<'a>(
  file: &Path,
  contents: &str,
  references: impl Iterator<Item = &'a OwnedProgram> + Clone,
) -> Result<BTreeSet<String>, Never> {
  let mut violations = BTreeSet::new();

  for (line, code) in (1_u64..).zip(contents.lines().map(str::trim_start)) {
    match code.strip_prefix("//") {
      Some(_) => continue,
      None => {}
    }

    let Ok(literals) = literal_program_arguments(code);
    let typed = code.contains("Program::");

    match (!typed && literals.exact.is_empty()).then_some(()) {
      Some(()) => continue,
      None => {}
    }

    for owned in references.clone() {
      let qualified = typed
        && (sources::spells(code, Word(&owned.qualified)) == Ok(Spelled::Yes)
          || sources::spells(code, Word(&owned.aliased)) == Ok(Spelled::Yes));
      let Ok(named) = literals.names(owned);

      match (qualified.then_some(()), named) {
        (None, Spelled::No) => {}
        (Some(()), _) | (None, Spelled::Yes) => {
          let _ = violations.insert(format!(
            "{}:{line} uses {:?}. Use {}.",
            file.display(),
            owned.program,
            owned.owner,
          ));
        }
      }
    }
  }

  Ok(violations)
}

fn literal_program_arguments(code: &str) -> Result<CommandWords<'_>, Never> {
  let literals = [
    concat!("Command::", "new("),
    "OsString::from(",
    ".arg(",
    ".args([",
  ]
  .into_iter()
  .flat_map(|call| code.match_indices(call))
  .filter_map(|(at, called)| {
    code
      .get(at.saturating_add(called.len())..)
      .and_then(|arguments| arguments.trim_start().strip_prefix('"'))
      .and_then(|value| value.split_once('"'))
      .map(|(word, _)| word)
  });

  CommandWords::from_words(literals)
}

#[test]
fn the_wayland_backend_owns_direct_protocol_dependencies() -> Result<(), Failure> {
  let root = console_repository::root()?;
  let mut metadata = MetadataCommand::new();
  let workspace = metadata.current_dir(root).no_deps().exec()?;
  let mut violations = BTreeSet::new();

  for package in workspace.workspace_packages() {
    'over_dependency_2: for dependency in &package.dependencies {
      let name = dependency.name.as_str();
      let owner = match (name, dependency.optional.then_some(())) {
        ("wayland-scanner", Some(())) => "console-compositor",
        ("wayland-scanner", None) => {
          violations.insert(format!(
            "{} must make wayland-scanner optional",
            package.name
          ));
          continue 'over_dependency_2;
        }
        (_, _) => match name.strip_prefix("wayland-") {
          Some(_) => "console-wayland",
          None => continue 'over_dependency_2,
        },
      };

      match { ::core::primitive::bool::then_some(package.name.as_str() != owner, ()) } {
        ::core::option::Option::Some(()) => {
          violations.insert(format!(
            "{} depends on {}. Use {owner}.",
            package.name, dependency.name
          ));
        }
        ::core::option::Option::None => {}
      }
    }
  }

  assert!(violations.is_empty(), "{violations:#?}");

  Ok(())
}

#[test]
fn first_party_applications_get_their_ui_through_console_panel() -> Result<(), Failure> {
  let root = console_repository::root()?;
  let mut metadata = MetadataCommand::new();
  let workspace = metadata.current_dir(root).no_deps().exec()?;
  let mut violations = BTreeSet::new();

  for package in workspace
    .workspace_packages()
    .into_iter()
    .filter(|package| package.name.as_str().starts_with("console-application-"))
  {
    for dependency in &package.dependencies {
      match dependency.name.as_str() {
        "console-core-shapes" | "console-draw-painting" | "console-draw-surface" => {
          violations.insert(format!(
            "{} depends on {}. Use console-panel.",
            package.name, dependency.name
          ));
        }
        _ => {}
      }
    }
  }

  assert!(violations.is_empty(), "{violations:#?}");
  Ok(())
}

const BOOT_MENU_TIMEOUT: &[(&str, &str)] = &[
  (
    "files/etc/systemd/system/console-boot-complete.service",
    "/usr/bin/bootctl set-timeout-oneshot menu-disabled",
  ),
  (
    "files/etc/systemd/system/console-recovery.service",
    "-/usr/bin/bootctl set-timeout-oneshot \"\"",
  ),
];

#[test]
fn shipped_commands_use_the_same_backends() -> Result<(), Failure> {
  let root = console_repository::root()?;
  let mut files = Vec::new();
  configuration_files(&root.join("files"), &mut files)?;
  configuration_files(&root.join("nix"), &mut files)?;
  files.extend([root.join("configuration.nix"), root.join("flake.nix")]);
  let hooks: BTreeMap<_, _> = BOOT_MENU_TIMEOUT
    .iter()
    .map(|(file, text)| (Path::new(file), *text))
    .collect();
  let Ok(references) = references();
  let mut violations = BTreeSet::new();
  let mut timeout_hooks = BTreeSet::new();

  for file in files {
    let relative = file.strip_prefix(&root)?;
    let Ok(language) = ConfigurationLanguage::from_path(&file);
    let contents = std::fs::read_to_string(&file)?;

    'commands: for (line, text) in (1_u64..).zip(contents.lines()) {
      let Ok(candidate) = command_text(language, text, &references);
      let command = match candidate {
        Some(named) => named,
        None => continue 'commands,
      };

      match hooks
        .get(relative)
        .filter(|&allowed| *allowed == command.trim())
      {
        Some(_) => {
          let _ = timeout_hooks.insert(relative.to_path_buf());
          continue 'commands;
        }
        None => {}
      }

      let Ok(words) = CommandWords::from_words(
        command
          .split(|ch: char| !ch.is_ascii_alphanumeric() && !"/_-".contains(ch))
          .map(|word| word.trim_start_matches('-')),
      );

      for owned in &references {
        let Ok(named) = words.names(owned);

        match named {
          Spelled::No => {}
          Spelled::Yes => {
            let _ = violations.insert(format!(
              "{}:{line} uses {}. Use {}.",
              relative.display(),
              owned.name,
              owned.owner,
            ));
          }
        }
      }
    }
  }

  assert!(violations.is_empty(), "{violations:#?}");
  assert_eq!(
    timeout_hooks,
    hooks.into_keys().map(Path::to_path_buf).collect()
  );

  Ok(())
}

fn configuration_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), Failure> {
  let Ok(entries) = console_core_directory_listing::recursive(directory, |_| Descend::Into);

  for entry in entries {
    let path = entry?;
    let extension = path.extension().and_then(|ending| ending.to_str());

    match extension {
      Some("service" | "conf" | "desktop" | "lua" | "nix" | "sh") => {
        let kind = std::fs::symlink_metadata(&path)?;

        match kind.file_type().is_file().then_some(()) {
          Some(()) => files.push(path),
          None => {}
        }
      }
      Some(_) | None => {}
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum ConfigurationLanguage {
  Script,
  Nix,
  Directives,
}

impl ConfigurationLanguage {
  fn from_path(file: &Path) -> Result<Self, Never> {
    Ok(match file.extension().and_then(|ending| ending.to_str()) {
      Some("lua" | "sh") => Self::Script,
      Some("nix") => Self::Nix,
      Some(_) | None => Self::Directives,
    })
  }
}

fn command_text<'a>(
  language: ConfigurationLanguage,
  line: &'a str,
  references: &[OwnedProgram],
) -> Result<Option<&'a str>, Never> {
  let text = line.trim();

  match text.as_bytes() {
    [b'#' | b';', ..] | [b'-', b'-', ..] => return Ok(None),
    _ => {}
  }

  match language {
    ConfigurationLanguage::Script => return Ok(Some(text)),
    ConfigurationLanguage::Nix => {
      let Ok(shell) = starts_shell_command(text, references);

      match (text.contains("/bin/").then_some(()), shell) {
        (Some(()), _) | (None, Spelled::Yes) => return Ok(Some(text)),
        (None, Spelled::No) => {}
      }
    }
    ConfigurationLanguage::Directives => {}
  }

  Ok(text.split_once('=').and_then(|(key, value)| {
    let named = key.trim();
    (named.starts_with("Exec")
      || named == "TryExec"
      || named.starts_with("on-")
      || named.ends_with("_cmd")
      || named.starts_with("exec"))
    .then_some(value)
  }))
}

fn starts_shell_command(line: &str, references: &[OwnedProgram]) -> Result<Spelled, Never> {
  let first = line.split_whitespace().next();

  match first {
    Some("exec" | "command" | "env" | "trap" | "if") => Ok(Spelled::Yes),
    Some(command) => {
      let Ok(words) = CommandWords::from_words([command]);
      let recognized = references
        .iter()
        .any(|owned| words.names(owned) == Ok(Spelled::Yes));

      Ok(match recognized.then_some(()) {
        Some(()) => Spelled::Yes,
        None => Spelled::No,
      })
    }
    None => Ok(Spelled::No),
  }
}

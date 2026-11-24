//! What the manifest says, held against the tree it is a manifest of.
//!
//! These need the repository rather than a fixture, so they live out here
//! rather than beside the code. Everything that can be decided from a string
//! alone is tested next to the function that decides it.

mod reading;

use std::collections::{BTreeMap, BTreeSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use console_core_atomic_writes::Stored;
use console_core_directory_listing::Descend;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_manifest_engine::manifest::{BuildName, LiveFile};
use console_manifest_engine::modes;
use reading::{Failure, Section, read, root, section};

fn console(arguments: &[&str]) -> Result<Output, Failure> {
  let done = Command::new(env!("CARGO_BIN_EXE_console"))
    .args(arguments)
    .output()?;

  Ok(done)
}

#[test]
fn doctor_is_listed_in_help_and_checks_paths_without_a_manifest() -> Result<(), Failure> {
  let commands = console(&["--help"])?;
  let command_help = String::from_utf8(commands.stdout)?;
  assert!(commands.status.success());
  assert!(command_help.contains("doctor"));

  let help = console(&["doctor", "--help"])?;
  let profile_help = String::from_utf8(help.stdout)?;
  assert!(help.status.success());
  assert!(profile_help.contains("console doctor"));
  assert!(profile_help.contains("development"));
  assert!(profile_help.contains("--report-only"));

  let present = console(&["doctor", "path", "/dev/null"])?;
  let report = String::from_utf8(present.stdout)?;
  assert!(present.status.success());
  assert!(report.contains("present  path /dev/null"));

  let invalid = console(&["doctor", "unknown-profile"])?;
  let usage = String::from_utf8(invalid.stderr)?;
  assert_eq!(invalid.status.code(), Some(2));
  assert!(usage.contains("console doctor"));

  Ok(())
}

fn walked(from: &Path) -> Result<Vec<PathBuf>, Failure> {
  let Ok(listing) =
    console_core_directory_listing::recursive(
      from,
      |at| match ::core::primitive::bool::then_some(at.ends_with("__pycache__"), ()) {
        ::core::option::Option::Some(()) => Descend::Past,
        ::core::option::Option::None => Descend::Into,
      },
    );
  let mut found = Vec::new();

  for entry in listing {
    match entry {
      Ok(path) => {
        let kind = entry_kind(&path)?;

        match (path.ends_with("__pycache__"), kind) {
          (false, EntryKind::File) => found.push(path),
          (true, _) | (false, EntryKind::Directory | EntryKind::Missing) => {}
        }
      }
      Err(unlisted) => {
        match ::core::primitive::bool::then_some(unlisted.fault.kind() == ErrorKind::NotFound, ())
        {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => return Err(Failure::from(unlisted)),
        }
      }
    }
  }

  found.sort();

  Ok(found)
}

enum EntryKind {
  File,
  Directory,
  Missing,
}

fn entry_kind(path: &Path) -> Result<EntryKind, Failure> {
  match std::fs::metadata(path) {
    Ok(metadata) => Ok(match metadata.is_dir().then_some(()) {
      Some(()) => EntryKind::Directory,
      None => EntryKind::File,
    }),
    Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
      Some(()) => Ok(EntryKind::Missing),
      None => Err(Failure::from(fault)),
    },
  }
}

fn every_file() -> Result<Vec<(PathBuf, String)>, Failure> {
  let Ok(root) = root();
  let files = root.join("files");
  let mut carried = Vec::new();

  let walked_files = walked(&files)?;

  for path in walked_files {
    let inside = path.strip_prefix(&files)?;
    let relative = inside.to_str().ok_or("a packaged path in UTF-8")?;
    let live = format!("/{relative}");

    carried.push((path, live));
  }

  Ok(carried)
}

fn every((under, ending): (&str, &str)) -> Result<Vec<PathBuf>, Failure> {
  let carried = every_file()?;

  Ok(
    carried
      .into_iter()
      .filter(|(_, live)| live.contains(under) && live.ends_with(ending))
      .map(|(path, _)| path)
      .collect(),
  )
}

fn mode_of(live: &str, head: &[u8]) -> Result<u32, Never> {
  modes::of(live, head)
}

const SOMEONE: &str = "ada";

fn owner_of(live: &str) -> Result<&'static str, Never> {
  Ok(
    match ::core::primitive::bool::then_some(live.starts_with("/home/@user@/"), ()) {
      ::core::option::Option::Some(()) => SOMEONE,
      ::core::option::Option::None => "root",
    },
  )
}

fn named_by(unit: &str) -> Result<Vec<String>, Never> {
  Ok(
    unit
      .lines()
      .filter_map(|line| line.split_once('='))
      .filter(|(key, _)| key.starts_with("Exec"))
      .flat_map(|(_, command)| command.split_whitespace())
      .map(|word| word.trim_start_matches(['-', '@', ':', '+', '!']))
      .filter(|word| word.starts_with('/'))
      .map(str::to_owned)
      .collect(),
  )
}

fn carried_or_declared(held: &str) -> Result<BTreeSet<String>, Failure> {
  let files = section(held, Section::Files)?;
  let elsewhere = section(held, Section::Elsewhere)?;
  let built = section(held, Section::Build)?;

  Ok(
    files
      .into_iter()
      .chain(elsewhere)
      .chain(
        built
          .into_iter()
          .map(|name| format!("/usr/local/bin/{name}")),
      )
      .collect(),
  )
}

fn manifest() -> Result<String, Failure> {
  let held = read("desktop.conf")?;
  let machines = read("machines.conf")?;
  let Ok(every) = console_manifest_engine::machines::of_every(&machines);

  Ok(format!("{held}\n{every}"))
}

fn crate_tables() -> Result<Vec<(PathBuf, toml::Table)>, Failure> {
  let Ok(root) = root();
  let mut every = Vec::new();

  let entries = std::fs::read_dir(root.join("crates"))?;

  for entry in entries {
    let entry_2 = entry?;
    let at = entry_2.path();

    match console_core_atomic_writes::read(&at.join("Cargo.toml")) {
      Ok(Stored::Text(held)) => {
        let table = held.parse::<toml::Table>()?;

        every.push((at, table));
      }
      Ok(Stored::Absent) => {}
      Ok(Stored::Failed(why)) => return Err(Failure::from(why)),
    }
  }

  Ok(every)
}

fn named(at: &toml::Value) -> Result<Option<String>, Never> {
  Ok(
    at.get("name")
      .and_then(toml::Value::as_str)
      .map(str::to_owned),
  )
}

fn programs() -> Result<Vec<String>, Failure> {
  let crates = crate_tables()?;
  let mut made = Vec::new();

  for (_, held) in crates {
    let written = match held.get("bin").and_then(toml::Value::as_array) {
      Some(bins) => bins.clone(),
      None => held.get("package").cloned().into_iter().collect(),
    };

    for one in &written {
      let Ok(name) = named(one);

      made.extend(name);
    }
  }

  Ok(made)
}

#[test]
fn the_manifest_this_desktop_wears_is_one_the_engine_can_read() -> Result<(), Failure> {
  let Ok(root) = root();
  let at = root.to_str().ok_or("a root that is a string")?;
  let done = console(&["list", "--root", at])?;
  let said = std::str::from_utf8(&done.stdout)?;

  assert!(
    done.status.success(),
    "console list could not read desktop.conf:\n{said}"
  );

  for section in ["[packages]", "[build]", "[files]", "[services]", "[masked]"] {
    assert!(said.contains(section), "{section} is not in the manifest");
  }

  Ok(())
}

#[test]
fn the_paper_service_sets_a_ground_and_paints_no_picture_of_its_own() -> Result<(), Failure> {
  let held = read("files/etc/systemd/user/console-paper.service")?;
  let sets = held
    .lines()
    .filter(|line| line.starts_with("ExecStart="))
    .collect::<Vec<_>>();

  assert!(
    sets
      .iter()
      .any(|line| line.contains("console-paper serve ")),
    "the paper service sets no ground color, so the screen is black until \
         console-wallpaper paints: {sets:?}"
  );
  assert!(
    !sets.iter().any(|line| line.contains(".webp")),
    "the paper service paints a picture of its own: {sets:?}"
  );

  Ok(())
}

#[test]
fn the_keyboard_follows_nothing_because_it_takes_the_devices_itself() -> Result<(), Failure> {
  let held = read("files/etc/systemd/user/console-input-keyboard.service")?;
  let ordered: Vec<&str> = held
    .lines()
    .filter(|line| line.starts_with("After=") || line.starts_with("Requires="))
    .collect();

  assert!(
    ordered.is_empty(),
    "the keyboard is ordered against something again: {ordered:?}. It takes the pad and the \
         keyboard InputPlumber publishes when its surface goes up and hands them back when it \
         comes down, so there is nothing left for an ordering to protect."
  );

  Ok(())
}

#[test]
fn the_way_to_game_mode_shuts_steam_down_before_the_compositor() -> Result<(), Failure> {
  let held = read("crates/console-session-cachyos/src/lib.rs")?;
  let (_, game) = held
    .split_once("fn game()")
    .ok_or("nothing switches to Game Mode")?;
  let (before_leaving, _) = game
    .split_once("console_compositor::exit")
    .ok_or("nothing leaves the compositor")?;

  assert!(
    before_leaving.contains("settle()"),
    "the switch must ask Steam to go before leaving the compositor"
  );

  Ok(())
}

#[test]
fn everything_meant_to_be_run_will_be_installed_able_to_run() -> Result<(), Failure> {
  let carried = every_file()?;

  for (path, live) in carried {
    let whole = std::fs::read(&path)?;
    let head: Vec<u8> = whole.into_iter().take(4).collect();

    match head.as_slice() {
      [b'#', b'!', ..] | [0x7f, b'E', b'L', b'F', ..] => {
        assert_eq!(
          mode_of(&live, &head),
          Ok(0o755),
          "{live} is a program and would be installed unrunnable"
        );
      }
      _ => {}
    }
  }

  Ok(())
}

#[test]
fn files_in_the_users_home_are_installed_as_the_user() -> Result<(), Failure> {
  let held = read("desktop.conf")?;
  let files = section(&held, Section::Files)?;

  assert!(!files.is_empty(), "the manifest names no files");

  for path in files {
    let expected = match ::core::primitive::bool::then_some(path.starts_with("/home/@user@/"), ())
    {
      ::core::option::Option::Some(()) => SOMEONE,
      ::core::option::Option::None => "root",
    };

    assert_eq!(
      owner_of(&path),
      Ok(expected),
      "{path} would be installed as the wrong user"
    );
  }

  Ok(())
}

#[test]
fn every_program_the_device_builds_is_one_this_repository_holds() -> Result<(), Failure> {
  let held = read("desktop.conf")?;
  let machines = read("machines.conf")?;
  let Ok(overlays) = console_manifest_engine::machines::of_every(&machines);
  let programs = programs()?;
  let made: BTreeSet<String> = programs.into_iter().collect();
  let mut built = section(&held, Section::Build)?;
  let extra_builds = section(&overlays, Section::Build)?;
  built.extend(extra_builds);

  for name in built {
    assert!(
      made.contains(&name),
      "the manifest builds {name} and nothing here makes a program called that; \
             this repository makes {made:?}"
    );
  }

  Ok(())
}

#[test]
fn the_font_the_bar_draws_its_icons_in_is_one_the_manifest_installs() -> Result<(), Failure> {
  let asked = console_core_fonts::ICONS;

  assert!(
    asked.contains("Nerd Font Mono"),
    "the bar asks for {asked:?}. Only the Mono cut draws these glyphs centered in \
         their cell; in the others the ink overflows the advance and hangs off the right, \
         which puts every icon a different distance off center"
  );

  let manifest = manifest()?;
  let packages = section(&manifest, Section::Packages)?;

  assert!(
    packages.iter().any(|name| name == "ttf-fantasque-nerd"),
    "[packages] does not name the font the bar asks for"
  );

  Ok(())
}

#[test]
fn every_file_the_manifest_lists_is_in_the_tree() -> Result<(), Failure> {
  let Ok(root) = root();
  let files = root.join("files");
  let manifest = manifest()?;
  let listed = section(&manifest, Section::Files)?;

  for path in listed {
    let relative = LiveFile::parse(&path)?;
    let installed = files.join(relative);
    let metadata = std::fs::metadata(&installed)
      .map_err(|fault| format!("{path} is listed but cannot be read: {fault}"))?;

    assert!(metadata.is_file(), "{path} is listed and is not a file");
  }

  Ok(())
}

#[test]
fn every_file_in_the_tree_is_listed() -> Result<(), Failure> {
  let manifest = manifest()?;
  let files = section(&manifest, Section::Files)?;
  let listed: BTreeSet<String> = files.into_iter().collect();

  let carried = every_file()?;

  for (_, live) in carried {
    assert!(
      listed.contains(&live),
      "{live} is in the tree and nothing installs it"
    );
  }

  Ok(())
}

#[test]
fn every_service_has_a_unit_the_manifest_carries() -> Result<(), Failure> {
  let held = manifest()?;
  let files = section(&held, Section::Files)?;
  let services = section(&held, Section::Services)?;
  let listed: BTreeSet<String> = files.into_iter().collect();

  for service in services {
    assert!(
      listed.contains(&format!("/etc/systemd/user/{service}")),
      "{service} is enabled and its unit is not carried"
    );
  }

  Ok(())
}

#[test]
fn the_target_pulls_in_exactly_the_services_that_are_enabled() -> Result<(), Failure> {
  let held = manifest()?;
  let services = section(&held, Section::Services)?;
  let enabled: BTreeSet<String> = services.into_iter().collect();
  let mut wanted: BTreeSet<String> = BTreeSet::new();

  for service in &enabled {
    let unit = read(&format!("files/etc/systemd/user/{service}"))?;

    match ::core::primitive::bool::then_some(
      unit
        .lines()
        .any(|line| line.trim() == "WantedBy=console.target"),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        wanted.insert(service.clone());
      }
      ::core::option::Option::None => {}
    }
  }

  assert_eq!(wanted, enabled);

  Ok(())
}

#[test]
fn every_program_a_unit_starts_is_carried() -> Result<(), Failure> {
  let held = manifest()?;
  let listed = carried_or_declared(&held)?;

  let found = every(("/etc/systemd/user/", ""))?;

  for unit in found {
    let said = std::fs::read_to_string(&unit)?;
    let name = unit
      .file_name()
      .and_then(std::ffi::OsStr::to_str)
      .ok_or("a packaged filename in UTF-8")?;
    let Ok(started) = named_by(&said);

    for command in started
      .into_iter()
      .filter(|at| at.starts_with("/usr/local/"))
    {
      assert!(
        listed.contains(&command),
        "{name} starts {command}, which is not carried"
      );
    }
  }

  Ok(())
}

#[test]
fn every_program_a_carried_script_reaches_for_is_carried() -> Result<(), Failure> {
  let held = manifest()?;
  let listed = carried_or_declared(&held)?;

  let found = every(("/usr/local/bin/", ""))?;

  for path in found {
    let said = match std::fs::read_to_string(&path) {
      Ok(said) => said,
      Err(_not_text_this_can_read) => continue,
    };
    let name = path
      .file_name()
      .and_then(std::ffi::OsStr::to_str)
      .ok_or("a packaged filename in UTF-8")?;
    let Ok(reached) = reaches_for(&said);

    for at in reached {
      assert!(
        listed.contains(&at),
        "{name} runs {at}, which is not carried"
      );
    }
  }

  Ok(())
}

fn reaches_for(said: &str) -> Result<BTreeSet<String>, Never> {
  Ok(
    said
      .split("/usr/local/bin/")
      .skip(1)
      .map(|rest| {
        let name: String = rest
          .chars()
          .take_while(|letter| letter.is_alphanumeric() || *letter == '-' || *letter == '_')
          .collect();

        format!("/usr/local/bin/{name}")
      })
      .filter(|at| at.len() > "/usr/local/bin/".len())
      .collect(),
  )
}

#[test]
fn every_shell_script_parses() -> Result<(), Failure> {
  let carried = every_file()?;

  for (path, live) in carried {
    let said = match std::fs::read_to_string(&path) {
      Ok(said) => said,
      Err(_not_text_this_can_read) => continue,
    };
    let first = said.lines().next().map_or("", str::trim);

    match ::core::primitive::bool::then_some(
      first.starts_with("#!") && (first.contains("/sh") || first.contains("bash")),
      (),
    ) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => continue,
    }

    shell_parses(&path, &live)?;
  }

  let Ok(root) = root();
  let scripts = written_in(&root.join("crates"), &["sh"])?;

  for path in scripts.into_iter().filter(|path| {
    path
      .parent()
      .is_some_and(|directory| directory.ends_with("src/scripts"))
  }) {
    shell_parses(&path, &path.display().to_string())?;
  }

  Ok(())
}

fn shell_parses(path: &Path, name: &str) -> Result<(), Failure> {
  let Ok(mut asking) = Program::Sh.command();
  let done = asking.arg("-n").arg(path).output()?;
  let diagnostic = std::str::from_utf8(&done.stderr)?;

  assert!(done.status.success(), "{name}: {}", diagnostic.trim());

  Ok(())
}

#[test]
fn every_yaml_file_parses() -> Result<(), Failure> {
  let found = every(("", ".yaml"))?;

  for path in found {
    let said = std::fs::read_to_string(&path)?;

    serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&said)
      .map_err(|fault| format!("{}: {fault}", path.display()))?;
  }

  Ok(())
}

#[test]
fn every_json_file_parses() -> Result<(), Failure> {
  let found = every(("", ".json"))?;

  for path in found {
    let said = std::fs::read_to_string(&path)?;

    serde_json::from_str::<serde::de::IgnoredAny>(&said)
      .map_err(|fault| format!("{}: {fault}", path.display()))?;
  }

  Ok(())
}

#[test]
fn something_answers_when_a_password_is_asked_for() -> Result<(), Failure> {
  let held = manifest()?;
  let services = section(&held, Section::Services)?;

  assert!(services.iter().any(|name| name == "console-polkit.service"));

  let said = read("files/etc/systemd/user/console-polkit.service")?;
  let Ok(starts) = named_by(&said);

  assert!(
    starts.iter().any(|at| at.contains("polkit")),
    "the polkit service starts {starts:?}"
  );

  Ok(())
}

#[test]
fn nothing_matches_a_process_by_a_name_the_kernel_cannot_hold() -> Result<(), Failure> {
  const COMM: u32 = 15;

  let Ok(root) = root();

  let entries = std::fs::read_dir(root.join("files/usr/local/bin"))?;

  for found in entries {
    let found_2 = found?;
    let path = found_2.path();
    let held = match std::fs::read_to_string(&path) {
      Ok(held) => held,
      Err(_not_text_this_can_read) => continue,
    };
    let asking = held.lines().map(str::trim).filter(|line| {
      !line.starts_with('#') && (line.contains("pkill") || line.contains("pgrep"))
    });

    'lines: for line in asking {
      let words: Vec<&str> = line.split_whitespace().collect();

      match ::core::primitive::bool::then_some(
        words.contains(&"-x") && !words.contains(&"-f"),
        (),
      ) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => continue 'lines,
      }

      let pattern = words
        .iter()
        .skip_while(|word| **word != "-x")
        .nth(1)
        .map_or("", |word| *word);
      let Ok(long) = fitted::<_, u32>(pattern.len());

      assert!(
        long <= COMM,
        "{} matches a process by the name {pattern:?}, which is {long} characters. The \
                 kernel keeps {COMM}, so this matches nothing and fails silently. Match the \
                 path with -f instead.",
        path.display(),
      );
    }
  }

  Ok(())
}

#[test]
fn the_keyboard_the_desktop_asks_is_one_the_manifest_installs() -> Result<(), Failure> {
  let installed = format!(
    "/usr/local/bin/{}",
    console_input_controller::mode::KEYBOARD
  );
  let held = manifest()?;
  let files = section(&held, Section::Files)?;
  let built = section(&held, Section::Build)?;
  let carried = files.iter().any(|path| path == &installed);
  let made = built
    .iter()
    .any(|name| installed == format!("/usr/local/bin/{name}"));

  assert!(
    carried || made,
    "the manifest neither carries nor builds {installed}, so the toggle asks a program \
         no one has"
  );

  Ok(())
}

#[test]
fn the_compositors_file_binds_nothing() -> Result<(), Failure> {
  let lua = read("files/home/@user@/.config/console/hypr/hyprland.lua")?;
  let bound: Vec<&str> = lua
    .lines()
    .filter(|line| line.contains("hl.bind("))
    .collect();

  assert!(
    bound.is_empty(),
    "the table is the only place that says what a press does, and this file says it too: {}",
    bound.join("\n")
  );

  Ok(())
}

#[test]
fn putting_the_screen_back_puts_the_panel_on_before_it_reads_the_note() -> Result<(), Failure> {
  let said = read("crates/console-settings/src/bin/console-brightness.rs")?;
  let (_, from) = said.split_once("fn undim(").ok_or("undim")?;
  let body = from.split_once("\nfn ").map_or(from, |(body, _)| body);
  let (before_the_note, _) = body
    .split_once("remembered()")
    .ok_or("undim does not read the note")?;

  assert!(
    body.contains("panel_on()"),
    "undim does not put the panel on"
  );
  assert!(
    before_the_note.contains("panel_on()"),
    "the panel is put on only after a note that can send undim home early"
  );

  let unit = read("files/etc/systemd/user/console-idle.service")?;
  assert!(
    said.contains("console_compositor::DisplayPower::On.set()"),
    "undim does not use the shared command for putting the panel on"
  );
  assert!(
    unit
      .lines()
      .any(|line| line == "ExecStartPre=-/usr/local/bin/console-compositor display-on"),
    "the idle unit no longer puts the screen on through the compositor adapter"
  );
  assert!(
    unit
      .lines()
      .any(|line| line == "ExecStopPost=-/usr/local/bin/console-compositor display-on"),
    "the idle unit no longer restores the display when it stops"
  );

  assert!(unit.lines().any(|line| line
    == concat!(
      "ExecStartPre=/usr/local/bin/lock-screen idle-config ",
      "/etc/console/hypridle.conf %t/console/hypridle.conf"
    )));
  assert!(
    unit
      .lines()
      .any(|line| { line == "ExecStart=/usr/bin/hypridle --config %t/console/hypridle.conf" })
  );

  let idle = read("files/etc/console/hypridle.conf")?;
  assert!(
    idle.lines().any(|line| {
      line.trim() == "on-timeout = /usr/local/bin/console-compositor display-off"
    }),
    "hypridle no longer blanks the display through the compositor adapter"
  );

  Ok(())
}

fn written_in(at: &Path, endings: &[&str]) -> Result<Vec<PathBuf>, Failure> {
  let every = walked(at)?;
  let endings_2: BTreeSet<&str> = endings.iter().copied().collect();

  Ok(
    every
      .into_iter()
      .filter(|path| {
        path
          .extension()
          .and_then(|ending| ending.to_str())
          .is_some_and(|ending| endings_2.contains(ending))
      })
      .collect(),
  )
}

fn keys_in(asked: &str) -> Result<Vec<String>, Never> {
  Ok(
    asked
      .split('=')
      .rev()
      .skip(1)
      .map(|before| {
        before
          .trim_end()
          .rsplit(|letter: char| !(letter.is_ascii_alphanumeric() || letter == '_'))
          .next()
          .map_or(String::new(), str::to_owned)
      })
      .collect(),
  )
}

#[test]
fn every_key_a_dispatch_hands_the_compositor_is_one_it_reads() -> Result<(), Failure> {
  const READ: &[&str] = &[
    "action",
    "direction",
    "follow",
    "mode",
    "monitor",
    "relative",
    "window",
    "workspace",
    "x",
    "y",
  ];

  let Ok(root) = root();
  let mut written = written_in(&root.join("crates"), &["rs"])?;

  let configured = written_in(&root.join("files"), &["lua", "conf", "service"])?;

  written.extend(configured);

  let mut unread = Vec::new();

  for path in written {
    let said = match std::fs::read_to_string(&path) {
      Ok(said) => said,
      Err(_not_text_this_can_read) => continue,
    };

    'dispatches: for dispatch in said.split("hl.dsp.").skip(1) {
      let (named, table) = match dispatch.split_once('(') {
        Some(split) => split,
        None => continue 'dispatches,
      };

      match ::core::primitive::bool::then_some(
        named
          .chars()
          .all(|letter| letter.is_ascii_lowercase() || letter == '.' || letter == '_')
          && table.starts_with('{'),
        (),
      ) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => continue 'dispatches,
      }

      let asked = table.split_once(')').map_or(table, |(asked, _)| asked);
      let Ok(keys) = keys_in(asked);

      for key in keys {
        let known = READ.contains(&key.as_str())
          || (named == "send_key_state" && matches!(key.as_str(), "mods" | "key" | "state"));

        match ::core::primitive::bool::then_some(known, ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => unread.push(format!(
            "{}: {key} in hl.dsp.{named}({asked})",
            path.display()
          )),
        }
      }
    }
  }

  assert!(
    unread.is_empty(),
    "a dispatch names a key this compositor does not read, which it answers with `ok` and \
         does nothing -- the renaming sweep turned `action` into `effect` inside these strings \
         once, and the screen stopped coming back on. A key Hyprland does read goes in READ \
         once it has been seen working:\n{}",
    unread.join("\n")
  );

  Ok(())
}

#[test]
fn one_thing_puts_the_panel_back_on_when_the_machine_wakes() -> Result<(), Failure> {
  let held = read("files/etc/console/hypridle.conf")?;
  let waking: Vec<&str> = held
    .lines()
    .map(str::trim)
    .filter(|line| line.starts_with("on-resume"))
    .collect();

  assert!(
    !waking.is_empty(),
    "nothing in hypridle answers a machine waking up"
  );

  for line in waking {
    assert!(
      line.contains("console-brightness undim"),
      "{line}\nhypridle resumes every listener that timed out at the same instant, and \
             `console-brightness undim` already puts the panel on with this dispatch. A second \
             one races it: both say ok, the compositor reads as on, and the screen stays dark."
    );
  }

  Ok(())
}

fn crate_manifests() -> Result<Vec<(String, PathBuf)>, Failure> {
  let crates = crate_tables()?;
  let mut holds = Vec::new();

  for (at, held) in crates {
    let bins = match held.get("bin").and_then(toml::Value::as_array) {
      Some(bins) => bins.clone(),
      None => Vec::new(),
    };

    for one in bins {
      let name = one.get("name").and_then(toml::Value::as_str);
      let path = one.get("path").and_then(toml::Value::as_str);

      match (name, path) {
        (Some(name_2), Some(path_2)) => {
          let source = reading::beneath(&at, path_2)?;

          holds.push((name_2.to_owned(), source));
        }
        (Some(_) | None, None) | (None, Some(_)) => {}
      }
    }
  }

  Ok(holds)
}

fn identifier(from: &str) -> Result<String, Never> {
  Ok(
    from
      .chars()
      .take_while(|one| one.is_alphanumeric() || *one == '_')
      .collect(),
  )
}

fn modules(held: &str) -> Result<BTreeSet<(String, String)>, Never> {
  Ok(
    held
      .match_indices("console_")
      .filter_map(|(at, _)| {
        let rest = held.get(at..)?;
        let Ok(whole) = identifier(rest);
        let after = rest.get(whole.len()..)?;
        let after_2 = after.strip_prefix("::")?;
        let Ok(module) = identifier(after_2);

        match ::core::primitive::bool::then_some(
          module.is_empty() || module.starts_with(char::is_uppercase),
          (),
        ) {
          ::core::option::Option::Some(()) => None,
          ::core::option::Option::None => Some((whole.replace('_', "-"), module)),
        }
      })
      .collect(),
  )
}

fn reached_by(bin: &Path) -> Result<String, Failure> {
  let own = std::fs::read_to_string(bin)?;
  let Ok(root) = root();
  let crates = root.join("crates");
  let mut held = own.clone();
  let Ok(modules) = modules(&own);

  for (what, module) in modules {
    let crate_name = BuildName::parse(&what)?;
    let module_name = BuildName::parse(&module)?;
    let source = crates.join(crate_name).join("src").join(module_name);

    for at in [source.with_extension("rs"), source.join("mod.rs")] {
      match console_core_atomic_writes::read(&at) {
        Ok(Stored::Text(text)) => held.push_str(&text),
        Ok(Stored::Absent) => {}
        Ok(Stored::Failed(why)) => return Err(Failure::from(why)),
      }
    }
  }

  Ok(held)
}

const FORCES: [&str; 15] = [
  "LockPersonality",
  "MemoryDenyWriteExecute",
  "NoNewPrivileges",
  "PrivateDevices",
  "ProtectClock",
  "ProtectHostname",
  "ProtectKernelLogs",
  "ProtectKernelModules",
  "ProtectKernelTunables",
  "RestrictAddressFamilies",
  "RestrictNamespaces",
  "RestrictRealtime",
  "RestrictSUIDSGID",
  "SystemCallArchitectures",
  "SystemCallFilter",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bound {
  Seccomp,
  Free,
}

fn bound(value: &str) -> Result<Bound, Never> {
  Ok(match value.trim() {
    "" | "no" | "false" | "off" | "0" => Bound::Free,
    _ => Bound::Seccomp,
  })
}

fn read_for(unit: &Path) -> Result<Vec<PathBuf>, Failure> {
  let file_name = unit.file_name().ok_or("a unit name")?;
  let named = file_name.to_str().ok_or("a unit name in UTF-8")?;
  let whose = match named.strip_suffix(".service") {
    Some(whose) => whose,
    None => return Ok(Vec::new()),
  };
  let every = every(("etc/systemd/user", ".conf"))?;
  let mut theirs: Vec<PathBuf> = every
    .into_iter()
    .filter(|path| {
      let over = path
        .parent()
        .and_then(Path::file_name)
        .and_then(std::ffi::OsStr::to_str);

      match over.and_then(|name| name.strip_suffix(".service.d")) {
        Some(stem) => stem == whose || (stem.ends_with('-') && whose.starts_with(stem)),
        None => false,
      }
    })
    .collect();

  theirs.sort_by(|left, right| left.file_name().cmp(&right.file_name()));

  Ok(theirs)
}

fn still_seccomp(unit: &Path) -> Result<BTreeSet<String>, Failure> {
  let mut standing: BTreeMap<String, Bound> = BTreeMap::new();
  let read_for = read_for(unit)?;

  for path in std::iter::once(unit.to_path_buf()).chain(read_for) {
    let held = std::fs::read_to_string(&path)?;
    let Ok(service) =
      console_core_ini_files::lines(&held, console_core_ini_files::Under("Service"));

    'lines: for line in service {
      let (key, value) = match line.split_once('=') {
        Some((key, value)) => (key.trim(), value),
        None => continue 'lines,
      };

      match ::core::primitive::bool::then_some(FORCES.contains(&key), ()) {
        ::core::option::Option::Some(()) => {
          let Ok(how) = bound(value);

          standing.insert(key.to_owned(), how);
        }
        ::core::option::Option::None => {}
      }
    }
  }

  Ok(
    standing
      .into_iter()
      .filter(|(_, how)| *how == Bound::Seccomp)
      .map(|(key, _)| key)
      .collect(),
  )
}

fn crossing() -> Result<BTreeSet<String>, Failure> {
  let holds = crate_manifests()?;
  let mut reached: Vec<(String, String)> = Vec::new();

  for (name, bin) in holds {
    let near = reached_by(&bin)?;

    reached.push((name, near));
  }

  let crossing: BTreeSet<String> = reached
    .iter()
    .filter(|(_, near)| near.contains("console_session_cachyos::"))
    .map(|(name, _)| name.clone())
    .collect();

  assert!(
    !crossing.is_empty(),
    "nothing in the tree crosses to Game Mode"
  );

  let mut crosses = crossing.clone();

  for (name, near) in &reached {
    for far in &crossing {
      match ::core::primitive::bool::then_some(near.contains(far.as_str()), ()) {
        ::core::option::Option::Some(()) => {
          crosses.insert(name.clone());
        }
        ::core::option::Option::None => {}
      }
    }
  }

  Ok(crosses)
}

#[test]
fn every_unit_that_can_cross_to_game_mode_may_still_become_root() -> Result<(), Failure> {
  let switcher = read("crates/console-session-cachyos/src/lib.rs")?;

  assert!(
    switcher.contains("Program::Pkexec"),
    "the switcher no longer becomes root, and what is below is only about the fact that it does"
  );

  let confining = read("files/etc/systemd/user/console-.service.d/confining.conf")?;

  assert!(
    confining.contains("NoNewPrivileges=yes"),
    "nothing takes the setuid bit away any more, so nothing has to be given it back"
  );

  let crosses = crossing()?;

  let found = every(("etc/systemd/user", ".service"))?;

  for unit in found {
    let held = std::fs::read_to_string(&unit)?;
    let Ok(started) = named_by(&held);
    let starts: BTreeSet<String> = started
      .into_iter()
      .filter_map(|path| path.rsplit_once('/').map(|(_, name)| name.to_string()))
      .collect();

    match ::core::primitive::bool::then_some(starts.is_disjoint(&crosses), ()) {
      ::core::option::Option::Some(()) => continue,
      ::core::option::Option::None => {}
    }

    let file_name = unit.file_name().ok_or("a unit name")?;
    let named = file_name.to_str().ok_or("a unit name in UTF-8")?;
    let standing = still_seccomp(&unit)?;

    assert!(
      standing.is_empty(),
      "{named} starts a program that crosses to Game Mode, which is done with pkexec, and \
             after everything read for it {standing:?} is still set. Each of those is seccomp, \
             and a unit of an unprivileged manager carrying any seccomp at all is a process with \
             the no-new-privileges bit whatever NoNewPrivileges= says: pkexec fails with \
             \"pkexec must be setuid root\", the session is never recorded, and the button does \
             nothing."
    );
  }

  Ok(())
}

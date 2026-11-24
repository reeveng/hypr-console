//! Device check operations that need the device's kernel, files, or session.
//!
//! Deployment confirmation reserves exit status 1 for a declined prompt.
//! Session setup failures use a separate status so the caller can prompt in
//! the terminal. InputPlumber replies before releasing a chord, so the chord
//! command gives it time to release before a caller can repeat the chord.

use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use console_bus::connection::Bus;
use console_bus::messages::{Kind, Message, Value};
use console_compositor::session::Session;
use console_core_external_programs::Program;
use console_core_internal_programs::{CONFIRM_DOES, InternalProgram};
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_input_gamepad::front;
use console_repository::RelativePath;

#[must_use]
pub fn dispatch(words: &[String]) -> Result<Option<ExitCode>, Never> {
  let (verb, arguments) = match words.split_first() {
    Some(command) => command,
    None => return Ok(None),
  };

  let result = match (verb.as_str(), arguments) {
    ("device-session", [owner, program, arguments_2 @ ..]) => {
      let mut child = Command::new(program);
      child.args(arguments_2);
      let Ok(status) = in_session(owner, child);
      return Ok(Some(status));
    }
    ("device-confirm", [label, question]) => {
      let Ok(program) = InternalProgram::Confirm.path();
      let mut card = Command::new(program);
      card.env(CONFIRM_DOES, label).arg(question);
      let Ok(status) = in_session("-", card);
      return Ok(Some(status));
    }
    ("device-session", _) => Err(io::Error::from(InvalidRequest::Session)),
    (console_awake::heartbeat::COMMAND, []) => console_awake::heartbeat::run(
      io::stdin().lock(),
      io::stdout().lock(),
      console_awake::heartbeat::EVERY,
      std::time::Instant::now,
    ),
    ("device-inspect", arguments_3) => inspect(arguments_3),
    ("device-progress", arguments_4) => device_progress(arguments_4),
    ("migration-move-home", [old, new]) => {
      crate::migration_files::move_home(crate::migration_files::HomeMove { old, new })
    }
    ("migration-move-pictures", [old, new]) => {
      crate::migration_files::move_pictures(Path::new(old), Path::new(new))
    }
    ("migration-sweep", [into, patterns @ ..]) => {
      crate::migration_files::sweep(Path::new(into), patterns)
    }
    ("device-repeat-chord", [count, capabilities @ ..]) => {
      let capabilities_2: Vec<_> = capabilities.iter().map(String::as_str).collect();
      count
        .parse::<u32>()
        .map_err(|_| io::Error::from(InvalidRequest::ChordCount))
        .and_then(|count_2| repeat_chord(count_2, &capabilities_2))
    }
    ("device-runtime-file", [operation, at]) => runtime_file(operation, Path::new(at)),
    _ => return Ok(None),
  };

  Ok(Some(match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      console_core_logging::warn!("console {verb}: {fault}");
      ExitCode::FAILURE
    }
  }))
}

enum InvalidRequest {
  Session,
  ChordCount,
  SessionUid,
  Inspection,
  ProgressValue,
  ProgressRange,
  ProgressOperation,
  MissingRuntime,
  RuntimeOperation,
  RuntimeFileName,
}

impl From<InvalidRequest> for io::Error {
  fn from(fault: InvalidRequest) -> Self {
    let message = match fault {
      InvalidRequest::Session => "device-session OWNER PROGRAM [ARG ...]",
      InvalidRequest::ChordCount => "invalid chord count",
      InvalidRequest::SessionUid => "invalid session UID",
      InvalidRequest::Inspection => "unknown device inspection or arguments",
      InvalidRequest::ProgressValue => "invalid progress value",
      InvalidRequest::ProgressRange => "progress is beyond 100 percent",
      InvalidRequest::ProgressOperation => "invalid progress operation",
      InvalidRequest::MissingRuntime => "XDG_RUNTIME_DIR is missing",
      InvalidRequest::RuntimeOperation => "unknown runtime file operation",
      InvalidRequest::RuntimeFileName => "the runtime file must be a single name",
    };
    Self::new(io::ErrorKind::InvalidInput, message)
  }
}

#[must_use]
fn in_session(owner: &str, mut command: Command) -> Result<ExitCode, Never> {
  const SESSION_UNAVAILABLE: u8 = 97;
  let launch = |program: &mut Command| {
    let uid = match owner {
      "-" => rustix::process::getuid().as_raw(),
      name => {
        let Ok(mut id) = Program::Id.command();
        let text = read_program(id.args(["-u", "--", name]))?;
        text
          .trim()
          .parse::<u32>()
          .map_err(|_| InvalidRequest::SessionUid)?
      }
    };
    let account = uid.to_string();
    let relative = single_name(Path::new(&account))?;
    let session = Session::discover(Path::new("/run/user").join(relative).as_path())?;
    let Ok(()) = session.apply(program);

    Ok::<_, io::Error>(program.exec())
  };

  Ok(match launch(&mut command) {
    Ok(fault) => {
      console_core_logging::warn!(
        "console device-session: {:?}: {fault}",
        command.get_program()
      );
      ExitCode::from(SESSION_UNAVAILABLE)
    }
    Err(fault) => {
      console_core_logging::warn!("console device-session: {fault}");
      ExitCode::from(SESSION_UNAVAILABLE)
    }
  })
}

fn inspect(arguments: &[String]) -> io::Result<()> {
  let words: Vec<&str> = arguments.iter().map(String::as_str).collect();

  match words.as_slice() {
    ["boot-menu"] => boot_menu(),
    ["claimed-gamepad"] => claimed_gamepad(),
    ["input-holders"] => input_holders(),
    ["icons-missing", theme, icons @ ..] => {
      missing_icons(Path::new("/usr/share/icons"), theme, icons)
    }
    _ => Err(io::Error::from(InvalidRequest::Inspection)),
  }
}

fn device_progress(arguments: &[String]) -> io::Result<()> {
  use console_notifications::updating::{self, Progress};

  let write = |amount: &str, label: &str| {
    let permille: u16 = amount.parse().map_err(|_| InvalidRequest::ProgressValue)?;

    match ::core::primitive::bool::then_some(permille <= updating::WHOLE, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(io::Error::from(InvalidRequest::ProgressRange)),
    }

    let Ok(()) = updating::wrote(&Progress {
      permille,
      label: label.to_owned(),
    });
    Ok(())
  };

  let (operation, operands) = match arguments.split_first() {
    Some(parts) => parts,
    None => return Err(io::Error::from(InvalidRequest::ProgressOperation)),
  };

  match (operation.as_str(), operands) {
    ("clear", []) => {
      let Ok(()) = updating::clear();
      Ok(())
    }
    ("write", [amount, label]) => write(amount, label),
    ("then", [amount, label, program, words @ ..]) => {
      write(amount, label)?;
      Err(Command::new(program).args(words).exec())
    }
    _ => Err(io::Error::from(InvalidRequest::ProgressOperation)),
  }
}

fn runtime_file(operation: &str, at: &Path) -> io::Result<()> {
  let name = single_name(at)?;
  let Ok(runtime_directory) = console_core_places::runtime();
  let runtime = runtime_directory.ok_or(InvalidRequest::MissingRuntime)?;
  let path = runtime.join(name);

  let result = match operation {
    "read" => fs::read_to_string(path).map(|contents| print!("{contents}")),
    "remove" => fs::remove_file(path),
    _ => return Err(io::Error::from(InvalidRequest::RuntimeOperation)),
  };

  match result {
    Ok(()) => Ok(()),
    Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
      Some(()) => Ok(()),
      None => Err(fault),
    },
  }
}

fn single_name(path: &Path) -> io::Result<RelativePath<'_>> {
  let mut components = path.components();

  match (components.next(), components.next()) {
    (Some(Component::Normal(_)), None) => {
      RelativePath::new(path).map_err(|fault| io::Error::new(io::ErrorKind::InvalidInput, fault))
    }
    _ => Err(io::Error::from(InvalidRequest::RuntimeFileName)),
  }
}

fn read_program(command: &mut Command) -> io::Result<String> {
  let output = command.output()?;

  match ::core::primitive::bool::then_some(output.status.success(), ()) {
    ::core::option::Option::Some(()) => String::from_utf8(output.stdout)
      .map_err(|fault| io::Error::new(io::ErrorKind::InvalidData, fault)),
    ::core::option::Option::None => {
      let stderr = match std::str::from_utf8(&output.stderr) {
        Ok(text) => text.trim().to_owned(),
        Err(fault) => format!("stderr is not UTF-8: {fault}"),
      };
      Err(io::Error::other(format!(
        "{command:?} {}: {stderr}",
        output.status
      )))
    }
  }
}

fn boot_menu() -> io::Result<()> {
  const VARIABLE: &str =
    "/sys/firmware/efi/efivars/LoaderConfigTimeoutOneShot-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f";
  let mounts = fs::read_to_string(crate::snapshot::MOUNTS)?;
  let Ok(observed) = crate::snapshot::root_in(&mounts);
  let root = observed.ok_or_else(|| io::Error::other("root mount is missing"))?;
  let left = match fs::read(VARIABLE) {
    Ok(bytes) => {
      let contents = bytes
        .get(4..)
        .ok_or_else(|| io::Error::other("truncated EFI variable"))?;
      let (words, _) = contents.as_chunks::<2>();
      let text: Vec<u16> = words
        .iter()
        .copied()
        .map(u16::from_le_bytes)
        .take_while(|word| *word != 0)
        .collect();
      String::from_utf16(&text)
        .map_err(|fault| io::Error::new(io::ErrorKind::InvalidData, fault))?
    }
    Err(fault) => {
      match ::core::primitive::bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => String::new(),
        ::core::option::Option::None => return Err(fault),
      }
    }
  };
  println!("{root}\n{left}");
  Ok(())
}

fn input_holders() -> io::Result<()> {
  let mut holders = BTreeSet::new();

  let processes = fs::read_dir("/proc")?;

  for process in processes.flatten() {
    let name = process.file_name();

    match name
      .to_str()
      .and_then(|name_2| match name_2.parse::<u32>() {
        ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
        ::core::result::Result::Err(_invalid_integer_) => ::core::option::Option::None,
      }) {
      Some(_) => {}
      None => continue,
    }

    let opened = match fs::read_dir(process.path().join("fd")) {
      Ok(opened) => opened,
      Err(_process_descriptors_unavailable) => continue,
    };

    let devices: Vec<PathBuf> = opened
      .flatten()
      .filter_map(|fd| match fs::read_link(fd.path()) {
        Ok(path) => Some(path),
        Err(_descriptor_target_unavailable) => None,
      })
      .filter(|target| target.starts_with("/dev/input"))
      .collect();

    match ::core::primitive::bool::then_some(devices.is_empty(), ()) {
      ::core::option::Option::Some(()) => continue,
      ::core::option::Option::None => {}
    }

    let command = match fs::read_to_string(process.path().join("comm")) {
      Ok(command) => command,
      Err(_process_name_unavailable) => continue,
    };

    for path in devices {
      holders.insert((command.trim_end().to_owned(), path));
    }
  }

  for (command, path) in holders {
    println!("{command} {}", path.display());
  }

  Ok(())
}

fn claimed_gamepad() -> io::Result<()> {
  let Ok(mut bus) = front::composite_devices();
  let objects = read_program(&mut bus)?;
  let mut claimed = BTreeSet::new();
  let mut composites = objects
    .lines()
    .map(str::trim)
    .filter(|object| {
      let composite = object
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_prefix("CompositeDevice"));

      match composite {
        Some(instance) => match instance.parse::<u32>() {
          Ok(_instance) => true,
          Err(_invalid_integer) => false,
        },
        None => false,
      }
    })
    .peekable();

  match composites.peek() {
    Some(_) => {
      let mut system = Bus::system().map_err(io::Error::other)?;

      for object in composites {
        let Ok(query) = front::source_device_paths(object);
        let reply = system.call(&query).map_err(io::Error::other)?;
        let paths = source_paths(&reply)?;

        claimed.extend(paths.iter().filter_map(|path| match path {
          Value::Word(name) | Value::Path(name) => Some(name.clone()),
          Value::Byte(_)
          | Value::Truth(_)
          | Value::Signed16(_)
          | Value::Unsigned16(_)
          | Value::Signed32(_)
          | Value::Unsigned32(_)
          | Value::Signed64(_)
          | Value::Unsigned64(_)
          | Value::Fraction(_)
          | Value::Shape(_)
          | Value::List(_)
          | Value::Group(_)
          | Value::Variant { .. } => None,
        }));
      }
    }
    None => {}
  }

  let devices = fs::read_to_string(front::DEVICES)?;

  let Ok(nodes) = controller_nodes(&devices);

  for node in nodes {
    let status = match ::core::primitive::bool::then_some(
      claimed.contains(format!("/dev/input/{node}").as_str()),
      (),
    ) {
      ::core::option::Option::Some(()) => "claimed",
      ::core::option::Option::None => "unclaimed",
    };
    println!("{node} {status}");
  }

  Ok(())
}

fn source_paths(reply: &Message) -> io::Result<&[Value]> {
  match (reply.kind, reply.values.as_slice()) {
    (Kind::Answer, [Value::Variant { value, .. }]) => match value.as_ref() {
      Value::List(paths) => Ok(paths),
      Value::Byte(_)
      | Value::Truth(_)
      | Value::Signed16(_)
      | Value::Unsigned16(_)
      | Value::Signed32(_)
      | Value::Unsigned32(_)
      | Value::Signed64(_)
      | Value::Unsigned64(_)
      | Value::Fraction(_)
      | Value::Word(_)
      | Value::Path(_)
      | Value::Shape(_)
      | Value::Group(_)
      | Value::Variant { .. } => Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "InputPlumber returned no source device paths",
      )),
    },
    (Kind::ErrorReply, _) => Err(io::Error::other(format!(
      "InputPlumber SourceDevicePaths: {:?} {:?}",
      reply.fault, reply.values
    ))),
    (Kind::Answer | Kind::Signal | Kind::Call, _) => Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "InputPlumber returned no source device paths",
    )),
  }
}

#[must_use]
fn controller_nodes(devices: &str) -> Result<impl Iterator<Item = &str>, Never> {
  Ok(
    devices
      .split("\n\n")
      .filter(|device| {
        device.lines().any(|line| {
          line
            .strip_prefix("N: Name=\"Legion-Controller ")
            .and_then(|name| name.strip_suffix('"'))
            .is_some_and(|name| !name.is_empty() && !name.contains([' ', '"']))
        })
      })
      .flat_map(|device| {
        device
          .lines()
          .filter_map(|line| line.strip_prefix("H: Handlers="))
          .flat_map(str::split_whitespace)
          .filter(|name| name.starts_with("event"))
      }),
  )
}

fn missing_icons(root: &Path, theme: &str, icons: &[&str]) -> io::Result<()> {
  let canonical = root.canonicalize()?;
  let initial_themes = ThemeWalk {
    pending: VecDeque::from([theme.to_owned(), "hicolor".to_owned()]),
    seen: BTreeSet::new(),
    directories: VecDeque::new(),
  };
  let directories = iterate(initial_themes, |mut themes| {
    Ok(match themes.pending.pop_front() {
      Some(next) => {
        let Ok(()) = themes.visit(&canonical, &next);
        Step::Again(themes)
      }
      None => Step::Halt(themes.directories),
    })
  })
  .map_err(io::Error::other)?;
  let initial_scan = IconWalk {
    directories,
    walked: BTreeSet::new(),
    found: BTreeSet::new(),
  };
  let found = iterate(initial_scan, |mut scan| {
    Ok(match scan.directories.pop_front() {
      Some(directory) => {
        let Ok(()) = scan.visit(&canonical, &directory);
        Step::Again(scan)
      }
      None => Step::Halt(scan.found),
    })
  })
  .map_err(io::Error::other)?;

  for icon in icons.iter().filter(|icon| !found.contains(**icon)) {
    println!("{icon}");
  }

  Ok(())
}

struct ThemeWalk {
  pending: VecDeque<String>,
  seen: BTreeSet<String>,
  directories: VecDeque<PathBuf>,
}

impl ThemeWalk {
  fn visit(&mut self, root: &Path, theme: &str) -> Result<(), Never> {
    let name = match single_name(Path::new(theme)) {
      Ok(name) => name,
      Err(_invalid_theme_name) => return Ok(()),
    };

    match self.seen.insert(theme.to_owned()).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let at = match root.join(name).canonicalize() {
      Ok(resolved) => resolved,
      Err(_unavailable_theme) => return Ok(()),
    };

    match at.starts_with(root).then_some(()) {
      Some(()) => self.directories.push_back(at.clone()),
      None => return Ok(()),
    }

    let index = match fs::read_to_string(at.join("index.theme")) {
      Ok(index) => index,
      Err(_unavailable_index) => return Ok(()),
    };

    let Ok(inherited) = console_core_ini_files::field(
      &index,
      console_core_ini_files::Under("Icon Theme"),
      console_core_ini_files::Key("Inherits"),
    );

    match inherited {
      Some(themes_list) => self.pending.extend(
        themes_list
          .split(',')
          .map(|parent| parent.trim().to_owned()),
      ),
      None => {}
    }

    Ok(())
  }
}

struct IconWalk {
  directories: VecDeque<PathBuf>,
  walked: BTreeSet<PathBuf>,
  found: BTreeSet<String>,
}

impl IconWalk {
  fn visit(&mut self, root: &Path, directory: &Path) -> Result<(), Never> {
    let real = match directory.canonicalize() {
      Ok(resolved) => resolved,
      Err(_directory_unavailable) => return Ok(()),
    };

    match (real.starts_with(root) && self.walked.insert(real.clone())).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let entries = match fs::read_dir(real) {
      Ok(entries) => entries,
      Err(_directory_unreadable) => return Ok(()),
    };

    for entry in entries.flatten() {
      let path = entry.path();
      let Ok(()) = self.inspect(path);
    }

    Ok(())
  }

  fn inspect(&mut self, path: PathBuf) -> Result<(), Never> {
    let directory = match path.metadata() {
      Ok(metadata) => metadata.is_dir().then_some(()),
      Err(_metadata_unavailable) => None,
    };

    match directory {
      Some(()) => self.directories.push_back(path),
      None => match path.extension().and_then(|extension| extension.to_str()) {
        Some("svg" | "png") => {
          self.found.extend(
            path
              .file_stem()
              .and_then(|stem| stem.to_str())
              .map(str::to_owned),
          );
        }
        Some(_) | None => {}
      },
    }

    Ok(())
  }
}

fn repeat_chord(count: u32, capabilities: &[&str]) -> io::Result<()> {
  match count {
    0 => return Ok(()),
    1.. => {}
  }

  let mut bus = Bus::system().map_err(io::Error::other)?;
  let Ok(command) = front::chord_message(capabilities);

  chord_rounds(
    &mut bus,
    count,
    |connected| {
      let reply = connected.call(&command).map_err(io::Error::other)?;

      match reply.kind {
        Kind::Answer => Ok(()),
        Kind::ErrorReply => Err(io::Error::other(format!(
          "SendButtonChord: {:?} {:?}",
          reply.fault, reply.values
        ))),
        Kind::Call | Kind::Signal => Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "SendButtonChord returned an unexpected D-Bus message",
        )),
      }
    },
    |_: &mut Bus| wait_for_release(),
  )
}

fn chord_rounds<State>(
  state: &mut State,
  count: u32,
  send: impl Fn(&mut State) -> io::Result<()>,
  release: impl Fn(&mut State) -> Result<(), Never>,
) -> io::Result<()> {
  for _ in 0..count {
    send(state)?;
    let Ok(()) = release(state);
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit021_no_sleeping",
  allow(
    explicit021_no_sleeping,
    reason = "the device owns the release delay of each chord"
  )
)]
fn wait_for_release() -> Result<(), Never> {
  std::thread::sleep(Duration::from_millis(120));
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Debug, PartialEq, Eq)]
  enum ChordStep {
    Send,
    Release,
  }

  #[test]
  fn each_chord_releases_before_the_next_one_starts() -> io::Result<()> {
    use ChordStep::{Release, Send};

    let mut seen = Vec::new();
    chord_rounds(
      &mut seen,
      2,
      |steps| {
        steps.push(Send);
        Ok(())
      },
      |steps| {
        steps.push(Release);
        Ok(())
      },
    )?;

    assert_eq!(seen, [Send, Release, Send, Release]);
    Ok(())
  }

  #[test]
  fn source_devices_come_from_the_properties_get_variant() -> io::Result<()> {
    let paths = vec![Value::Word("/dev/input/event9".to_owned())];
    let Ok(property) = Value::variant("as", Value::List(paths.clone()));
    let reply = Message {
      kind: Kind::Answer,
      values: vec![property],
      ..Message::default()
    };

    let decoded = source_paths(&reply)?;
    assert_eq!(decoded, paths);
    Ok(())
  }
}

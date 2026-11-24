//! Selecting a Hyprland instance and invoking its command line tools.
//!
//! Session discovery can inspect another account's runtime directory. It opens
//! the lock relative to the instance directory and refuses symlinks and FIFOs.

use rustix::fs::{Mode, OFlags, open, openat};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_external_programs::Program;
use console_core_never::Never;

use crate::DispatchResult;
use crate::socket::{self, Socket};

pub const SIGNATURE: &str = "HYPRLAND_INSTANCE_SIGNATURE";
pub const DISPLAY: &str = "WAYLAND_DISPLAY";

pub fn control_name() -> Result<&'static str, Never> {
  Program::Hyprctl.name()
}

pub fn control() -> Result<Command, Never> {
  Program::Hyprctl.command()
}

pub fn verify_configuration(config: &Path) -> Result<Command, Never> {
  let Ok(mut command) = Program::Hyprland.command();
  command.args(["--verify-config", "-c"]).arg(config);

  Ok(command)
}

#[cfg_attr(
  dylint_lib = "explicit082_no_built_shell_line",
  expect(
    explicit082_no_built_shell_line,
    reason = "Hyprland's -c option takes a configuration file path"
  )
)]
pub fn launch_arguments(config: &str) -> Result<Vec<String>, Never> {
  Program::Hyprland.arguments(&["-c", config])
}

pub fn start_command(config: &Path) -> Result<Command, Never> {
  let Ok(mut command) = Program::StartHyprland.command();
  command.args(["--", "--config"]).arg(config);

  Ok(command)
}

pub fn create_headless(mut command: Command) -> Result<DispatchResult, Never> {
  crate::run_control(command.args(["output", "create", "headless"]))
}

pub fn responds(signature: &str) -> Result<InstanceStatus, Never> {
  let Ok(mut command) = control();

  Ok(match command.args(["-i", signature, "version"]).output() {
    Ok(output) => match output.status.success().then_some(()) {
      Some(()) => InstanceStatus::Responding,
      None => InstanceStatus::Unreachable,
    },
    Err(_unavailable) => InstanceStatus::Unreachable,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceStatus {
  Responding,
  Unreachable,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Session {
  pub runtime: PathBuf,
  pub signature: String,
  pub display: String,
}

impl Session {
  pub fn discover(runtime: &Path) -> io::Result<Self> {
    let flag = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW;
    let root = open(runtime, flag, Mode::empty())
      .and_then(|runtime_fd| openat(&runtime_fd, socket::DIRECTORY, flag, Mode::empty()))
      .map_err(|fault| {
        io::Error::new(
          io::Error::from(fault).kind(),
          format!(
            "looking for a Hyprland session in {}: {fault}",
            runtime.display()
          ),
        )
      })?;
    let mut candidates = Vec::new();
    let entries = fs::read_dir(runtime.join(socket::DIRECTORY)).map_err(|fault| {
      io::Error::new(
        fault.kind(),
        format!(
          "looking for a Hyprland session in {}: {fault}",
          runtime.display()
        ),
      )
    })?;

    for entry in entries.flatten() {
      let signature = match entry.file_name().into_string() {
        Ok(signature) => signature,
        Err(_non_unicode_signature) => continue,
      };
      let metadata = match entry.path().symlink_metadata() {
        Ok(metadata) => metadata,
        Err(_unavailable) => continue,
      };

      match metadata.is_dir().then_some(()) {
        Some(()) => {
          let modified = match metadata.modified() {
            Ok(modified) => Some(modified),
            Err(_modification_time_unavailable) => None,
          };
          candidates.push((modified, signature));
        }
        None => {}
      }
    }

    candidates.sort_unstable();

    for (_, signature) in candidates.into_iter().rev() {
      let directory = match socket::instance_directory(runtime, &signature) {
        Ok(directory) => directory,
        Err(_invalid_signature) => continue,
      };
      let instance = match openat(&root, &signature, flag, Mode::empty()) {
        Ok(instance) => instance,
        Err(_instance_unavailable) => continue,
      };
      let display = match display_at(&instance) {
        Ok(Some(name)) => name,
        Ok(None) => continue,
        Err(fault) => {
          console_core_logging::debug!("reading compositor instance {signature}: {fault}");

          continue;
        }
      };
      #[cfg_attr(
        dylint_lib = "explicit086_no_climbing_join",
        expect(
          explicit086_no_climbing_join,
          reason = "display_at only returns wayland- followed by ASCII digits"
        )
      )]
      let display_path = runtime.join(&display);
      let display_metadata = match display_path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(_display_unavailable) => continue,
      };

      match display_metadata.file_type().is_socket().then_some(()) {
        Some(()) => {}
        None => continue,
      }

      match connects(&directory.join(Socket::Requests)) {
        Ok(()) => {
          return Ok(Self {
            runtime: runtime.to_owned(),
            signature,
            display,
          });
        }
        Err(fault) => {
          console_core_logging::debug!("connecting to compositor instance {signature}: {fault}")
        }
      }
    }

    Err(io::Error::new(
      io::ErrorKind::NotFound,
      format!(
        "no Hyprland session with a Wayland socket in {}",
        runtime.display()
      ),
    ))
  }

  pub fn apply(&self, command: &mut Command) -> Result<(), Never> {
    command
      .env("XDG_RUNTIME_DIR", &self.runtime)
      .env(SIGNATURE, &self.signature)
      .env(DISPLAY, &self.display);

    Ok(())
  }
}

pub fn signatures(runtime: &Path) -> io::Result<BTreeSet<String>> {
  let entries = fs::read_dir(runtime.join(socket::DIRECTORY))?;

  Ok(
    entries
      .flatten()
      .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
      .filter_map(|entry| match entry.file_name().into_string() {
        Ok(signature) => Some(signature),
        Err(_non_unicode_signature) => None,
      })
      .collect(),
  )
}

pub fn display_in(directory: &Path) -> io::Result<Option<String>> {
  let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW;
  let directory_fd = open(directory, flags, Mode::empty())?;

  display_at(&directory_fd)
}

fn display_at(directory: impl std::os::fd::AsFd) -> io::Result<Option<String>> {
  let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
  let descriptor = openat(directory, LOCK_FILE, flags, Mode::empty())?;
  let file = File::from(descriptor);
  let metadata = file.metadata()?;

  match metadata.is_file().then_some(()) {
    Some(()) => {
      let said = console_core_bounded_reads::text(file, 4096)?;
      let Ok(display) = display_from_lock(&said);

      Ok(display)
    }
    None => Ok(None),
  }
}

pub fn connects(at: &Path) -> rustix::io::Result<()> {
  use rustix::net::{
    AddressFamily, SocketAddrUnix, SocketFlags, SocketType, connect, socket_with,
  };

  let address = SocketAddrUnix::new(at)?;
  let socket = socket_with(
    AddressFamily::UNIX,
    SocketType::STREAM,
    SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
    None,
  )?;

  connect(&socket, &address)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisplayName {
  ADisplay,
  SomethingElse,
}

fn classify_display(name: &str) -> Result<DisplayName, Never> {
  let rest = match name.strip_prefix("wayland-") {
    Some(rest) => rest,
    None => return Ok(DisplayName::SomethingElse),
  };

  Ok(
    match ::core::primitive::bool::then_some(
      !rest.is_empty() && rest.chars().all(|said| said.is_ascii_digit()),
      (),
    ) {
      ::core::option::Option::Some(()) => DisplayName::ADisplay,
      ::core::option::Option::None => DisplayName::SomethingElse,
    },
  )
}

pub const LOCK_FILE: &str = "hyprland.lock";

pub fn display_from_lock(said: &str) -> Result<Option<String>, Never> {
  let name = match said.lines().nth(1) {
    Some(name) => name.trim().to_string(),
    None => return Ok(None),
  };

  let Ok(names) = classify_display(&name);

  Ok(match names {
    DisplayName::ADisplay => Some(name),
    DisplayName::SomethingElse => None,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn installed_sessions_use_the_watchdog_and_keep_the_config_path_as_one_argument() {
    let at = Path::new("/a directory/monitor.lua");
    let Ok(command) = start_command(at);
    let Ok(watchdog) = Program::StartHyprland.name();

    assert_eq!(command.get_program(), watchdog);
    assert_eq!(
      command.get_args().collect::<Vec<_>>(),
      ["--", "--config", "/a directory/monitor.lua"]
    );
  }

  #[test]
  fn discovery_pairs_the_instance_with_its_own_display() -> Result<(), Box<dyn std::error::Error>>
  {
    use std::os::unix::net::UnixListener;

    let directory = console_core_temporary_directories::private("hyprland-discovery")?;
    let runtime = directory.as_path();
    let instance = runtime.join(socket::DIRECTORY).join("mine");
    fs::create_dir_all(&instance)?;
    fs::create_dir_all(runtime.join(socket::DIRECTORY).join("incomplete"))?;
    let record = instance.join(LOCK_FILE);
    console_core_atomic_writes::whole(&record, b"42\nwayland-8\n")?;
    let control = UnixListener::bind(instance.join(Socket::Requests))?;
    let _display = UnixListener::bind(runtime.join("wayland-8"))?;
    let _other = UnixListener::bind(runtime.join("wayland-9"))?;

    let discover = Session::discover(runtime)?;
    assert_eq!(
      discover,
      Session {
        runtime: runtime.to_owned(),
        signature: "mine".to_owned(),
        display: "wayland-8".to_owned(),
      }
    );

    drop(control);
    assert_eq!(
      Session::discover(runtime).unwrap_err().kind(),
      io::ErrorKind::NotFound,
      "a socket left on disk after the compositor dies cannot be selected",
    );

    fs::remove_dir_all(runtime)?;
    Ok(())
  }

  #[test]
  fn a_socket_named_after_a_display_is_not_the_display() {
    let named = |name: &str| {
      let Ok(named) = classify_display(name);

      named
    };

    assert_eq!(named("wayland-1"), DisplayName::ADisplay);
    assert_eq!(named("wayland-12"), DisplayName::ADisplay);
    assert_eq!(named("wayland-1.lock"), DisplayName::SomethingElse);
    assert_eq!(
      named("wayland-2-console-paper.sock"),
      DisplayName::SomethingElse,
      "the wallpaper daemon opens this beside the display it draws on, and it is \
             not the display"
    );
    assert_eq!(named("wayland-"), DisplayName::SomethingElse);
    assert_eq!(named("waylandish"), DisplayName::SomethingElse);
    assert_eq!(named("pipewire-0"), DisplayName::SomethingElse);
  }

  #[test]
  fn a_compositor_says_which_display_it_bound_on_the_second_line() {
    assert_eq!(
      display_from_lock("2015407\nwayland-1\n"),
      Ok(Some("wayland-1".to_string()))
    );
    assert_eq!(
      display_from_lock("2015407\nwayland-12"),
      Ok(Some("wayland-12".to_string()))
    );
  }

  #[test]
  fn a_lock_caught_half_written_names_no_display() {
    assert_eq!(display_from_lock(""), Ok(None));
    assert_eq!(
      display_from_lock("2015407\n"),
      Ok(None),
      "the pid is written before the display is"
    );
    assert_eq!(
      display_from_lock("2015407\nwayl"),
      Ok(None),
      "and the display is written a byte at a time"
    );
  }
}

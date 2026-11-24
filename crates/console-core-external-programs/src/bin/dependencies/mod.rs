//! Registered executables, exact deployment paths and additional build inputs.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;
use std::process::{ExitCode, Stdio};

use console_core_external_programs::{EVERY, Installed, Origin, Program, provide};
use console_core_never::Never;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Profile {
  Desktop,
  Handheld,
  Development,
  Optional,
}

impl Profile {
  fn name(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Desktop => "desktop",
      Self::Handheld => "handheld",
      Self::Development => "development",
      Self::Optional => "optional",
    })
  }

  fn of(origin: Origin) -> Result<Self, Never> {
    Ok(match origin {
      Origin::Package(_) | Origin::Arch => Self::Desktop,
      Origin::Handheld => Self::Handheld,
      Origin::Developing => Self::Development,
      Origin::Optional => Self::Optional,
    })
  }
}

#[derive(Clone, Copy)]
enum Kind {
  Executable,
  Path,
  PkgConfig,
}

use Kind::{Executable, Path as Resource, PkgConfig};

const DESKTOP: &[(Kind, &str)] = &[
  (Executable, "/usr/bin/chgrp"),
  (Executable, "/usr/bin/chmod"),
  (Executable, "/usr/bin/hypridle"),
  (Executable, "/usr/bin/hyprsunset"),
  (Executable, "/usr/bin/rm"),
  (Executable, "/usr/bin/setfacl"),
  (Executable, "/usr/bin/syncthing"),
  (Executable, "/usr/bin/systemctl"),
  (Executable, "/usr/lib/polkit-kde-authentication-agent-1"),
  (Executable, "inputplumber"),
  (Resource, "/dev/uinput"),
  (Resource, "/usr/share/X11/xkb/rules/evdev"),
  (Resource, "/usr/share/i18n/SUPPORTED"),
  (Resource, "/usr/share/iso-codes/json/iso_3166-1.json"),
  (Resource, "/usr/share/iso-codes/json/iso_639-3.json"),
  (Resource, "/usr/share/mime/globs2"),
  (Resource, "/usr/share/zoneinfo/zone1970.tab"),
];

const HANDHELD: &[(Kind, &str)] = &[
  (Executable, "/usr/bin/bootctl"),
  (Executable, "/usr/bin/chown"),
  (Executable, "/usr/bin/findmnt"),
  (Executable, "/usr/bin/sh"),
  (Executable, "/usr/bin/sudo"),
  (Executable, "/usr/bin/systemd-sysusers"),
  (Resource, "/usr/lib/systemd/user/gamescope-session.target"),
  (
    Resource,
    "/usr/share/wayland-sessions/gamescope-session.desktop",
  ),
];

const DEVELOPMENT: &[(Kind, &str)] = &[
  (Executable, "awk"),
  (Executable, "bash"),
  (Executable, "cargo-dylint"),
  (Executable, "cc"),
  (Executable, "dirname"),
  (Executable, "docker"),
  (Executable, "dylint-link"),
  (Executable, "find"),
  (Executable, "glslangValidator"),
  (Executable, "grep"),
  (Executable, "head"),
  (Executable, "install"),
  (Executable, "node"),
  (Executable, "npm"),
  (Executable, "realpath"),
  (Executable, "rustc"),
  (Executable, "rustfmt"),
  (Executable, "sed"),
  (Executable, "sort"),
  (Executable, "spirv-val"),
  (Executable, "tail"),
  (Executable, "timeout"),
  (Executable, "tr"),
  (PkgConfig, "libcrypt"),
  (PkgConfig, "pam"),
  (PkgConfig, "vulkan"),
  (PkgConfig, "wayland-client"),
  (Resource, "/usr/share/edk2/x64/OVMF.4m.fd"),
];

const OPTIONAL: &[(Kind, &str)] = &[
  (Executable, "obs"),
  (PkgConfig, "wpe-webkit-2.0 >= 2.54"),
  (PkgConfig, "wpe-platform-headless-2.0"),
  (
    Resource,
    "/usr/share/dbus-1/system-services/net.reactivated.Fprint.service",
  ),
];

const PROFILES: &[(Profile, &[(Kind, &str)])] = &[
  (Profile::Desktop, DESKTOP),
  (Profile::Handheld, HANDHELD),
  (Profile::Development, DEVELOPMENT),
  (Profile::Optional, OPTIONAL),
];

const USAGE: &str = "\
Usage: console-dependencies [--report-only] [desktop|handheld|development|optional|all]
       console-dependencies [--report-only] executable NAME...
       console-dependencies [--report-only] path PATH...
       console-dependencies [--report-only] pkg-config MODULE...

The default profile is desktop (Arch/CachyOS). Each check is read-only.
Use --report-only to report unavailable dependencies and exit successfully.
Otherwise, exit status 1 means a dependency is missing or could not be checked.";

enum OnUnavailable {
  Report,
  Fail,
}

pub(super) fn run() -> Result<ExitCode, Never> {
  let arguments: Vec<_> = std::env::args_os().skip(1).collect();
  let report_only = OsString::from("--report-only");
  let (mode, requested) = match arguments.strip_prefix(std::slice::from_ref(&report_only)) {
    Some(rest) => (OnUnavailable::Report, rest),
    None => (OnUnavailable::Fail, arguments.as_slice()),
  };

  match requested {
    [] => profiles(mode, [Profile::Desktop]),
    [profile] => match profile.to_str() {
      Some("-h" | "--help") => {
        println!("{USAGE}");
        Ok(ExitCode::SUCCESS)
      }
      Some("desktop") => profiles(mode, [Profile::Desktop]),
      Some("handheld") => profiles(mode, [Profile::Handheld]),
      Some("development") => profiles(mode, [Profile::Development]),
      Some("optional") => profiles(mode, [Profile::Optional]),
      Some("all") => profiles(mode, PROFILES.iter().map(|&(included, _)| included)),
      Some(_) | None => usage_error(),
    },
    [command, names @ ..] => {
      let kind = match command.to_str() {
        Some("executable") => Executable,
        Some("path") => Resource,
        Some("pkg-config") => PkgConfig,
        Some(_) | None => return usage_error(),
      };

      report(
        mode,
        names.iter().map(|name| (kind, name.as_os_str(), None)),
      )
    }
  }
}

fn usage_error() -> Result<ExitCode, Never> {
  eprintln!("{USAGE}");
  Ok(ExitCode::from(2))
}

fn profiles(
  mode: OnUnavailable,
  selection: impl IntoIterator<Item = Profile>,
) -> Result<ExitCode, Never> {
  let selected: Vec<_> = selection.into_iter().collect();
  let programs = EVERY.iter().filter_map(|program| {
    let Ok(origin) = program.origin();
    let Ok(profile) = Profile::of(origin);
    let Ok(name) = program.name();
    let Ok(label) = profile.name();
    let source = match origin {
      Origin::Package(package) => package,
      Origin::Arch | Origin::Handheld | Origin::Developing | Origin::Optional => label,
    };

    selected
      .contains(&profile)
      .then_some((Executable, OsStr::new(name), Some(source)))
  });
  let extra = PROFILES
    .iter()
    .filter(|(profile, _)| selected.contains(profile))
    .flat_map(|(profile, checks)| {
      let Ok(label) = profile.name();
      checks
        .iter()
        .map(move |&(kind, name)| (kind, OsStr::new(name), Some(label)))
    });

  report(mode, programs.chain(extra))
}

fn report<'a>(
  mode: OnUnavailable,
  checks: impl IntoIterator<Item = (Kind, &'a OsStr, Option<&'a str>)>,
) -> Result<ExitCode, Never> {
  let mut missing = 0_u64;
  let mut unchecked = 0_u64;

  for (kind, name, origin) in checks {
    let source = match origin {
      Some(label) => format!(" ({label})"),
      None => String::new(),
    };
    let label = match kind {
      Executable => "executable",
      Resource => "path",
      PkgConfig => "pkg-config",
    };
    let name_for_display = match name.to_str() {
      Some(text) => std::borrow::Cow::Borrowed(text),
      None => std::borrow::Cow::Owned(format!("{name:?}")),
    };

    match check(kind, name) {
      Ok(Installed::Yes) => println!("present  {label} {name_for_display}{source}"),
      Ok(Installed::No) => {
        println!("missing  {label} {name_for_display}{source}");
        missing = missing.saturating_add(1);
      }
      Err(error) => {
        println!("could not check {label} {name_for_display}{source} — {error}");
        unchecked = unchecked.saturating_add(1);
      }
    }
  }

  match (missing, unchecked) {
    (0, 0) => println!("\nAll selected dependencies are available."),
    (1, 0) => println!("\nOne selected dependency is missing."),
    (count, 0) => println!("\n{count} selected dependencies are missing."),
    (0, 1) => println!("\nOne selected dependency could not be checked."),
    (0, count) => println!("\n{count} selected dependencies could not be checked."),
    (absent, unconfirmed) => {
      println!(
        "\nSome dependencies are unavailable ({absent} missing, {unconfirmed} unchecked)."
      );
    }
  }

  Ok(match (missing, unchecked, mode) {
    (0, 0, OnUnavailable::Fail) | (_, _, OnUnavailable::Report) => ExitCode::SUCCESS,
    (_, _, OnUnavailable::Fail) => ExitCode::FAILURE,
  })
}

fn check(kind: Kind, name: &OsStr) -> io::Result<Installed> {
  match kind {
    Executable => presence(provide(name)),
    Resource => presence(std::fs::metadata(Path::new(name))),
    PkgConfig => {
      let mut command = Program::PkgConfig.provide()?;
      let status = command
        .args(["--exists", "--"])
        .arg(name)
        .stdin(Stdio::null())
        .status()
        .map_err(|error| {
          io::Error::new(error.kind(), format!("cannot run pkg-config: {error}"))
        })?;

      match status.code() {
        Some(0) => Ok(Installed::Yes),
        Some(1) => Ok(Installed::No),
        Some(_) | None => Err(io::Error::other(format!("pkg-config exited with {status}"))),
      }
    }
  }
}

fn presence<T>(observed: io::Result<T>) -> io::Result<Installed> {
  match observed {
    Ok(_) => Ok(Installed::Yes),
    Err(error) => match (error.kind() == io::ErrorKind::NotFound).then_some(()) {
      Some(()) => Ok(Installed::No),
      None => Err(error),
    },
  }
}

//! Registered executables, exact deployment paths and additional build inputs.

use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::path::Path;
use std::process::{ExitCode, Stdio};

use console_core_never::Never;

use super::{EVERY, Installed, Origin, Program, provide};

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

  fn about(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Desktop => "Arch/CachyOS desktop programs, device files, and runtime data (default)",
      Self::Handheld => "CachyOS Steam and Gamescope session switching, and boot helpers",
      Self::Development => "build and test tools, native library metadata, and QEMU firmware",
      Self::Optional => "OBS, Nix, fingerprint service registration, and WPE WebKit libraries",
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

#[derive(Clone, Copy)]
pub enum Invocation {
  Standalone,
  Doctor,
}

impl Invocation {
  fn usage(self, output: &mut impl Write) -> io::Result<()> {
    let command = match self {
      Self::Standalone => "console-dependencies",
      Self::Doctor => "console doctor",
    };
    writeln!(
      output,
      "\
Usage: {command} [--report-only] [PROFILE]
       {command} [--report-only] executable NAME...
       {command} [--report-only] path PATH...
       {command} [--report-only] pkg-config MODULE...

Profiles:"
    )?;

    for &(profile, _) in PROFILES {
      let Ok(name) = profile.name();
      let Ok(about) = profile.about();
      writeln!(output, "  {name:<13} {about}")?;
    }

    writeln!(
      output,
      "  all           Run every profile, including optional integrations

Individual checks:
  executable    Check a name on PATH or an exact executable path
  path          Check that a file, directory, device, or symlink target exists
  pkg-config    Check a module, optionally with a version constraint

Options:
  -h, --help    Show this help
  --report-only  Exit successfully when checks find unavailable dependencies

Examples:
  {command} development
  {command} --report-only all
  {command} pkg-config 'wpe-webkit-2.0 >= 2.54'

The checks are read-only and use the current account's environment.
Exit status 1 means a dependency is missing or could not be checked.
Exit status 2 means the arguments are invalid."
    )
  }
}

enum OnUnavailable {
  Report,
  Fail,
}

pub fn run(
  invocation: Invocation,
  arguments: &[OsString],
  output: &mut impl Write,
  errors: &mut impl Write,
) -> io::Result<ExitCode> {
  let (mode, requested) = match arguments
    .split_first()
    .filter(|(flag, _)| *flag == "--report-only")
  {
    Some((_, rest)) => (OnUnavailable::Report, rest),
    None => (OnUnavailable::Fail, arguments),
  };

  match requested {
    [] => profiles(invocation, mode, [Profile::Desktop], output),
    [argument] => match argument.to_str() {
      Some("-h" | "--help") => {
        invocation.usage(output)?;
        Ok(ExitCode::SUCCESS)
      }
      Some("all") => profiles(invocation, mode, PROFILES.iter().map(|&(p, _)| p), output),
      Some(name) => {
        let selected = PROFILES
          .iter()
          .find(|&&(candidate, _)| candidate.name() == Ok(name));

        match selected {
          Some(&(profile, _)) => profiles(invocation, mode, [profile], output),
          None => usage_error(invocation, errors),
        }
      }
      None => usage_error(invocation, errors),
    },
    [command, names @ ..] => {
      let kind = match command.to_str() {
        Some("executable") => Executable,
        Some("path") => Resource,
        Some("pkg-config") => PkgConfig,
        Some(_) | None => return usage_error(invocation, errors),
      };

      report(
        invocation,
        mode,
        names.iter().map(|name| (kind, name.as_os_str(), None)),
        output,
      )
    }
  }
}

fn usage_error(invocation: Invocation, errors: &mut impl Write) -> io::Result<ExitCode> {
  invocation.usage(errors)?;
  Ok(ExitCode::from(2))
}

fn profiles(
  invocation: Invocation,
  mode: OnUnavailable,
  selection: impl IntoIterator<Item = Profile>,
  output: &mut impl Write,
) -> io::Result<ExitCode> {
  let selected: Vec<_> = selection.into_iter().collect();
  let programs = EVERY.iter().filter_map(|program| {
    let Ok(origin) = program.origin();
    let Ok(profile) = Profile::of(origin);
    let Ok(name) = program.name();
    let source = match origin {
      Origin::Package(package) => Source::ArchPackage(package),
      Origin::Arch | Origin::Handheld | Origin::Developing | Origin::Optional => {
        Source::Profile(profile)
      }
    };

    selected
      .contains(&profile)
      .then_some((Executable, OsStr::new(name), Some(source)))
  });
  let extra = PROFILES
    .iter()
    .filter(|(profile, _)| selected.contains(profile))
    .flat_map(|(profile, checks)| {
      checks
        .iter()
        .map(move |&(kind, name)| (kind, OsStr::new(name), Some(Source::Profile(*profile))))
    });

  report(invocation, mode, programs.chain(extra), output)
}

enum Source {
  ArchPackage(&'static str),
  Profile(Profile),
}

fn report<'a>(
  invocation: Invocation,
  mode: OnUnavailable,
  checks: impl IntoIterator<Item = (Kind, &'a OsStr, Option<Source>)>,
  output: &mut impl Write,
) -> io::Result<ExitCode> {
  let mut missing = 0_u64;
  let mut unchecked = 0_u64;

  for (kind, name, origin) in checks {
    let source = match (invocation, origin) {
      (Invocation::Doctor, Some(Source::ArchPackage(package))) => {
        format!(" (Arch package {package})")
      }
      (Invocation::Standalone, Some(Source::ArchPackage(package))) => format!(" ({package})"),
      (_, Some(Source::Profile(profile))) => {
        let Ok(label) = profile.name();
        format!(" ({label})")
      }
      (_, None) => String::new(),
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
      Ok(Installed::Yes) => writeln!(output, "present  {label} {name_for_display}{source}")?,
      Ok(Installed::No) => {
        writeln!(output, "missing  {label} {name_for_display}{source}")?;
        missing = missing.saturating_add(1);
      }
      Err(error) => {
        writeln!(
          output,
          "could not check {label} {name_for_display}{source} — {error}"
        )?;
        unchecked = unchecked.saturating_add(1);
      }
    }
  }

  let summary = match (missing, unchecked) {
    (0, 0) => writeln!(output, "\nAll selected dependencies are available."),
    (1, 0) => writeln!(output, "\nOne selected dependency is missing."),
    (count, 0) => writeln!(output, "\n{count} selected dependencies are missing."),
    (0, 1) => writeln!(output, "\nOne selected dependency could not be checked."),
    (0, count) => writeln!(
      output,
      "\n{count} selected dependencies could not be checked."
    ),
    (absent, unconfirmed) => {
      writeln!(
        output,
        "\nSome dependencies are unavailable ({absent} missing, {unconfirmed} unchecked)."
      )
    }
  };
  summary?;

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

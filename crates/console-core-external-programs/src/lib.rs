//! Every program this desktop runs and did not write.
//!
//! There was a table of these in a test, with a scan of the source under it:
//! it looked for a string literal at the front of an arguments and asked the
//! machine whether a program by that name was installed. That is a net, and a
//! net is what gets built when the thing itself cannot be enumerated. This is
//! the thing itself. One variant per program, so the list is exhaustive
//! because the compiler says so, and a program no one thought about is a
//! variant that does not exist rather than a word that slipped past a scan.
//!
//! [`Origin`] is the other half, and is what makes the list worth holding. A
//! program has to be on the machine before it will run, `desktop.conf` is what
//! puts it there, and for a year three of these were on the device only
//! because something else had dragged them in. So every [`Origin::Package`]
//! below is checked against `[packages]` by a test. The other origins cover
//! base Arch, development tools, the handheld's preinstalled session tools,
//! and optional integrations that handle a missing executable in their UI.
//!
//! What the enum cannot reach is a program named inside another program's
//! arguments: `sh -c`, and the word after `stdbuf`. Those are written with
//! [`Program::name`], which is why it is public -- the program is still a
//! variant, it is simply spelled into an argument rather than run directly.
//! [`Program::command_from`] puts bare names under the specified directory and
//! keeps registered absolute paths. Neither case searches PATH.

use std::path::Path;
use std::process::Command;

use console_core_never::Never;

pub mod shell;

#[cfg(unix)]
pub mod dependencies;
#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::{Installed, installed, provide, provide_with};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
  Package(&'static str),
  Arch,
  Developing,
  Optional,
  Handheld,
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one table generates Program, EVERY and the exhaustive name and origin matches, so adding a program cannot leave any of those four inventories out of sync"
  )
)]
macro_rules! programs {
    ($($variant:ident, $name:literal, $origin:expr;)*) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum Program {
            $($variant,)*
        }

        pub const EVERY: &[Program] = &[$(Program::$variant,)*];

        impl Program {
            pub const fn name(self) -> Result<&'static str, Never> {
                Ok(match self {
                    $(Program::$variant => $name,)*
                })
            }

            pub const fn origin(self) -> Result<Origin, Never> {
                Ok(match self {
                    $(Program::$variant => $origin,)*
                })
            }
        }
    };
}

programs! {
    Alacritty, "alacritty", Origin::Package("alacritty");
    Bootctl, "bootctl", Origin::Arch;
    Busctl, "busctl", Origin::Arch;
    Cal, "cal", Origin::Arch;
    Cargo, "cargo", Origin::Package("rust");
    CargoNextest, "cargo-nextest", Origin::Developing;
    Cat, "cat", Origin::Arch;
    Chmod, "chmod", Origin::Arch;
    Chromium, "chromium", Origin::Package("chromium");
    Cmake, "cmake", Origin::Package("cmake");
    Cp, "cp", Origin::Arch;
    Curl, "curl", Origin::Package("curl");
    Date, "date", Origin::Arch;
    DbusDaemon, "dbus-daemon", Origin::Arch;
    Df, "df", Origin::Arch;
    Dot, "dot", Origin::Developing;
    Du, "du", Origin::Arch;
    Echo, "echo", Origin::Arch;
    Env, "env", Origin::Arch;
    Ffmpeg, "ffmpeg", Origin::Package("ffmpeg");
    Ffprobe, "ffprobe", Origin::Package("ffmpeg");
    Gdbus, "gdbus", Origin::Package("glib2");
    Getent, "getent", Origin::Arch;
    Gio, "gio", Origin::Package("glib2");
    Git, "git", Origin::Package("git");
    Grim, "grim", Origin::Package("grim");
    Hostname, "hostname", Origin::Developing;
    Hostnamectl, "hostnamectl", Origin::Arch;
    Hyprctl, "hyprctl", Origin::Package("hyprland");
    Hyprland, "Hyprland", Origin::Package("hyprland");
    Id, "id", Origin::Arch;
    Iw, "iw", Origin::Package("iw");
    Just, "just", Origin::Developing;
    Locale, "locale", Origin::Arch;
    LocaleGen, "locale-gen", Origin::Arch;
    Localectl, "localectl", Origin::Arch;
    LocalSend, "localsend", Origin::Package("localsend");
    Loginctl, "loginctl", Origin::Arch;
    Logger, "logger", Origin::Arch;
    Ls, "ls", Origin::Arch;
    Mise, "mise", Origin::Developing;
    Mkdir, "mkdir", Origin::Arch;
    Mktemp, "mktemp", Origin::Arch;
    Modprobe, "modprobe", Origin::Arch;
    Mv, "mv", Origin::Arch;
    Nix, "nix", Origin::Optional;
    Nmcli, "nmcli", Origin::Package("networkmanager");
    NotifySend, "notify-send", Origin::Package("libnotify");
    Pacman, "pacman", Origin::Arch;
    Pactl, "pactl", Origin::Package("libpulse");
    Pgrep, "pgrep", Origin::Arch;
    PkgConfig, "pkg-config", Origin::Developing;
    Pkexec, "pkexec", Origin::Handheld;
    Pkill, "pkill", Origin::Arch;
    Powerprofilesctl, "powerprofilesctl", Origin::Package("power-profiles-daemon");
    Ps, "ps", Origin::Arch;
    PwCat, "pw-cat", Origin::Package("pipewire-audio");
    PwMetadata, "pw-metadata", Origin::Package("pipewire");
    PwRecord, "pw-record", Origin::Package("pipewire-audio");
    QemuX86_64, "qemu-system-x86_64", Origin::Developing;
    Rm, "rm", Origin::Arch;
    Runuser, "runuser", Origin::Arch;
    Rustup, "rustup", Origin::Developing;
    Scp, "scp", Origin::Developing;
    SevenZip, "7z", Origin::Package("7zip");
    Sha256sum, "sha256sum", Origin::Arch;
    Sh, "sh", Origin::Arch;
    Sleep, "sleep", Origin::Arch;
    Ss, "ss", Origin::Arch;
    Snapper, "snapper", Origin::Package("snapper");
    Ssh, "ssh", Origin::Developing;
    StartHyprland, "start-hyprland", Origin::Package("hyprland");
    Stdbuf, "stdbuf", Origin::Arch;
    Steam, "steam", Origin::Handheld;
    SteamSetSession, "/usr/lib/steamos/steam-set-session", Origin::Handheld;
    StockSessionSelect, "/usr/bin/steamos-session-select", Origin::Handheld;
    Sudo, "sudo", Origin::Package("sudo");
    Syncthing, "syncthing", Origin::Package("syncthing");
    Systemctl, "systemctl", Origin::Arch;
    Tesseract, "tesseract", Origin::Package("tesseract");
    SystemdInhibit, "systemd-inhibit", Origin::Arch;
    SystemdRun, "systemd-run", Origin::Arch;
    Tar, "tar", Origin::Arch;
    Timedatectl, "timedatectl", Origin::Arch;
    True, "true", Origin::Arch;
    Udevadm, "udevadm", Origin::Arch;
    Usermod, "usermod", Origin::Arch;
    WhisperCli, "whisper-cli", Origin::Package("whisper-cpp");
    Wpctl, "wpctl", Origin::Package("wireplumber");
    Wtype, "wtype", Origin::Package("wtype");
    XdgMime, "xdg-mime", Origin::Package("xdg-utils");
    XdgOpen, "xdg-open", Origin::Package("xdg-utils");
    XdgSettings, "xdg-settings", Origin::Package("xdg-utils");
    YtDlp, "yt-dlp", Origin::Package("yt-dlp");
}

impl Program {
  #[doc = "Resolve a local executable before constructing its fallible launch command."]
  #[cfg(unix)]
  pub fn provide(self) -> std::io::Result<Command> {
    let Ok(name) = self.name();
    let executable = provide(name)?;

    Ok(Command::new(executable))
  }

  pub fn command(self) -> Result<Command, Never> {
    let Ok(name) = self.name();

    Ok(Command::new(name))
  }

  #[cfg_attr(
    dylint_lib = "explicit086_no_climbing_join",
    expect(
      explicit086_no_climbing_join,
      reason = "absolute paths in the closed program table are handled separately, and the table test checks that every remaining name is one normal component"
    )
  )]
  pub fn command_from(self, directory: &Path) -> Result<Command, Never> {
    let Ok(name) = self.name();
    let path = Path::new(name);

    Ok(match path.is_absolute().then_some(()) {
      Some(()) => Command::new(path),
      None => Command::new(directory.join(path)),
    })
  }

  pub fn arguments(self, rest: &[&str]) -> Result<Vec<String>, Never> {
    let Ok(name) = self.name();

    Ok(
      std::iter::once(name.to_string())
        .chain(rest.iter().map(|word| (*word).to_string()))
        .collect(),
    )
  }

  pub fn words(self, rest: Vec<String>) -> Result<Vec<String>, Never> {
    let Ok(name) = self.name();

    Ok(std::iter::once(name.to_string()).chain(rest).collect())
  }
}

#[derive(Debug)]
pub enum Unprinted {
  Absent,
  Unran(String, std::io::Error),
  NotText(String, std::string::FromUtf8Error),
}

impl std::fmt::Display for Unprinted {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Unprinted::Absent => write!(to, "there was no program to run"),
      Unprinted::Unran(program, fault) => write!(to, "{program} would not run: {fault}"),
      Unprinted::NotText(program, fault) => {
        write!(to, "{program} printed invalid UTF-8: {fault}")
      }
    }
  }
}

impl std::error::Error for Unprinted {}

pub fn capture_output<Word: AsRef<str>>(arguments: &[Word]) -> Result<String, Unprinted> {
  let (program, rest) = match arguments.split_first() {
    Some((program, rest)) => (program, rest),
    None => return Err(Unprinted::Absent),
  };

  let name = program.as_ref();
  #[cfg(unix)]
  let executable = provide(name).map_err(|fault| Unprinted::Unran(name.to_string(), fault))?;
  #[cfg(not(unix))]
  let executable = name;

  match Command::new(executable)
    .args(rest.iter().map(AsRef::as_ref))
    .output()
  {
    Ok(done) => {
      String::from_utf8(done.stdout).map_err(|fault| Unprinted::NotText(name.to_string(), fault))
    }
    Err(fault) => Err(Unprinted::Unran(name.to_string(), fault)),
  }
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  allow(
    explicit026_env_read_once,
    reason = "PATH is where the machine looks for a program, and a program this desktop did not write is what this crate is a list of. Four crates walked it to ask whether a program was installed and each said something different when it was unset, which is why `installed` is here beside it"
  )
)]
fn path_setting() -> Result<Option<std::ffi::OsString>, Never> {
  Ok(std::env::var_os("PATH"))
}

pub fn path() -> Result<Option<String>, Never> {
  let Ok(configured) = path_setting();
  let text = configured.as_deref().and_then(std::ffi::OsStr::to_str);
  Ok(text.filter(|value| !value.is_empty()).map(str::to_owned))
}

pub fn path_entries() -> Result<Option<Vec<std::path::PathBuf>>, Never> {
  let Ok(configured) = path_setting();
  Ok(configured.map(|path| std::env::split_paths(&path).collect()))
}

#[cfg(all(test, unix))]
mod tests {
  use super::*;

  type Failure = Box<dyn std::error::Error>;

  #[test]
  fn what_a_program_printed_is_handed_back_and_one_that_cannot_run_says_so() -> Result<(), Failure>
  {
    let said = capture_output(&["sh", "-c", "printf said"])?;

    assert_eq!(said, "said");
    assert!(matches!(
      capture_output::<&str>(&[]),
      Err(Unprinted::Absent)
    ));
    assert!(matches!(
      capture_output(&["definitely-not-a-program-anybody-installed"]),
      Err(Unprinted::Unran(..))
    ));

    Ok(())
  }

  #[test]
  fn output_that_is_not_utf8_is_reported_without_replacing_its_bytes() {
    let Ok(arguments) = Program::Sh.arguments(&["-c", "printf '\\377'"]);
    let printed = capture_output(&arguments);
    let bytes = match &printed {
      Err(Unprinted::NotText(_, fault)) => Some(fault.as_bytes()),
      Ok(_) | Err(Unprinted::Absent | Unprinted::Unran(..)) => None,
    };

    assert_eq!(bytes, Some([0xff].as_slice()), "{printed:?}");
  }

  #[test]
  fn a_program_on_the_path_is_installed() {
    assert_eq!(installed("sh"), Ok(Installed::Yes));
    assert_eq!(
      installed("definitely-not-a-program-anybody-installed"),
      Ok(Installed::No)
    );
    assert_eq!(installed(""), Ok(Installed::No));
  }

  #[test]
  fn a_program_named_by_where_it_is_is_looked_for_there() {
    assert_eq!(installed("/bin/sh"), Ok(Installed::Yes));
    assert_eq!(installed("/definitely/not/here"), Ok(Installed::No));
  }

  #[test]
  fn a_folder_on_the_path_is_not_a_program() {
    assert_eq!(installed("/"), Ok(Installed::No));
    assert_eq!(installed("/usr/bin/"), Ok(Installed::No));
  }

  #[test]
  fn a_file_nobody_can_run_is_not_a_program() -> Result<(), Failure> {
    let folder = console_core_temporary_directories::fresh("installed")?;
    let at = folder.join("unrunnable");

    console_core_atomic_writes::whole(&at, b"")?;

    let spelled = at
      .to_str()
      .ok_or(std::io::Error::from(std::io::ErrorKind::InvalidData))?;

    assert_eq!(installed(spelled), Ok(Installed::No));
    assert_eq!(
      provide(&at).map_err(|fault| fault.kind()),
      Err(std::io::ErrorKind::PermissionDenied)
    );

    std::fs::remove_dir_all(&folder)?;

    Ok(())
  }

  #[test]
  fn providing_a_symlink_keeps_its_name_and_checks_platform_access() -> Result<(), Failure> {
    let folder = console_core_temporary_directories::fresh("program-symlink")?;
    let link = folder.join("shell");
    std::os::unix::fs::symlink("/bin/sh", &link)?;

    let provided = provide(&link)?;
    assert_eq!(provided, link);
    let denied = provide_with(&link, |at| {
      assert_eq!(at, link);
      Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
    });
    assert_eq!(
      denied.map_err(|fault| fault.kind()),
      Err(std::io::ErrorKind::PermissionDenied)
    );

    std::fs::remove_dir_all(&folder)?;

    Ok(())
  }

  #[test]
  fn no_two_programs_answer_to_the_same_name() {
    let mut names: Vec<&str> = EVERY
      .iter()
      .map(|program| {
        let Ok(name) = program.name();
        name
      })
      .collect();
    names.sort_unstable();
    let mut once = names.clone();
    once.dedup();

    assert_eq!(once, names, "two variants share a name");
  }

  #[test]
  fn registered_programs_use_the_directory_for_bare_names_and_keep_absolute_paths() {
    let directory = Path::new("/registered-programs");

    for program in EVERY {
      let Ok(name) = program.name();
      let path = Path::new(name);
      let Ok(command) = program.command_from(directory);
      let placed = Path::new(command.get_program());

      match path.is_absolute().then_some(()) {
        Some(()) => assert_eq!(placed, path),
        None => {
          assert_eq!(path.file_name(), Some(std::ffi::OsStr::new(name)));
          assert_eq!(placed.parent(), Some(directory));
          assert_eq!(placed.file_name(), path.file_name());
        }
      }
    }
  }

  #[test]
  fn the_argv_starts_with_the_program() {
    let Ok(arguments) = Program::Hyprctl.arguments(&["monitors", "-j"]);

    assert_eq!(arguments, ["hyprctl", "monitors", "-j"]);
  }

  #[test]
  fn owned_words_are_carried_the_same_way() {
    let told = vec!["trash".to_string(), "--".to_string()];

    let Ok(words) = Program::Gio.words(told);

    assert_eq!(words, ["gio", "trash", "--"]);
  }
}

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
//! below is crossed against `[packages]` by a test, and a program that comes
//! with base Arch or only with a machine that develops this says which it is
//! rather than being absent from the list for a reason no one wrote down.
//!
//! What the enum cannot reach is a program named inside another program's
//! arguments: `sh -c`, and the word after `stdbuf`. Those are written with
//! [`Program::name`], which is why it is public -- the program is still a
//! variant, it is simply spelled into an argument rather than run directly.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use console_core_never::Never;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    Package(&'static str),
    Arch,
    Developing,
}

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
    Awww, "awww", Origin::Package("awww");
    Bluetoothctl, "bluetoothctl", Origin::Package("bluez-utils");
    Busctl, "busctl", Origin::Arch;
    Cal, "cal", Origin::Arch;
    Cargo, "cargo", Origin::Package("rust");
    Cat, "cat", Origin::Arch;
    Cmake, "cmake", Origin::Package("cmake");
    Cp, "cp", Origin::Arch;
    Curl, "curl", Origin::Package("curl");
    Date, "date", Origin::Arch;
    DbusDaemon, "dbus-daemon", Origin::Arch;
    Df, "df", Origin::Arch;
    Du, "du", Origin::Arch;
    Echo, "echo", Origin::Arch;
    Env, "env", Origin::Arch;
    Ffmpeg, "ffmpeg", Origin::Package("ffmpeg");
    Ffprobe, "ffprobe", Origin::Package("ffmpeg");
    Gdbus, "gdbus", Origin::Package("glib2");
    Gio, "gio", Origin::Package("glib2");
    Git, "git", Origin::Package("git");
    Grim, "grim", Origin::Package("grim");
    Hostname, "hostname", Origin::Developing;
    Hostnamectl, "hostnamectl", Origin::Arch;
    Hyprctl, "hyprctl", Origin::Package("hyprland");
    Hyprland, "Hyprland", Origin::Developing;
    Id, "id", Origin::Arch;
    Iw, "iw", Origin::Package("iw");
    Just, "just", Origin::Developing;
    Locale, "locale", Origin::Arch;
    LocaleGen, "locale-gen", Origin::Arch;
    Localectl, "localectl", Origin::Arch;
    Logger, "logger", Origin::Arch;
    Ls, "ls", Origin::Arch;
    Mkdir, "mkdir", Origin::Arch;
    Modprobe, "modprobe", Origin::Arch;
    Mv, "mv", Origin::Arch;
    Nmcli, "nmcli", Origin::Package("networkmanager");
    NotifySend, "notify-send", Origin::Package("libnotify");
    Pacman, "pacman", Origin::Arch;
    Pactl, "pactl", Origin::Package("libpulse");
    Pdfinfo, "pdfinfo", Origin::Package("poppler");
    Pdftoppm, "pdftoppm", Origin::Package("poppler");
    Pkill, "pkill", Origin::Arch;
    Powerprofilesctl, "powerprofilesctl", Origin::Package("power-profiles-daemon");
    Ps, "ps", Origin::Arch;
    PwCat, "pw-cat", Origin::Package("pipewire-audio");
    PwRecord, "pw-record", Origin::Package("pipewire-audio");
    Rm, "rm", Origin::Arch;
    RsvgConvert, "rsvg-convert", Origin::Package("librsvg");
    Runuser, "runuser", Origin::Arch;
    Scp, "scp", Origin::Developing;
    SevenZip, "7z", Origin::Package("7zip");
    Sh, "sh", Origin::Arch;
    Ss, "ss", Origin::Arch;
    Snapper, "snapper", Origin::Package("snapper");
    Ssh, "ssh", Origin::Developing;
    Stdbuf, "stdbuf", Origin::Arch;
    Su, "su", Origin::Arch;
    Sudo, "sudo", Origin::Package("sudo");
    Systemctl, "systemctl", Origin::Arch;
    SystemdInhibit, "systemd-inhibit", Origin::Arch;
    SystemdRun, "systemd-run", Origin::Arch;
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
    pub fn command(self) -> Result<Command, Never> {
        let Ok(name) = self.name();

        Ok(Command::new(name))
    }

    pub fn arguments(self, rest: &[&str]) -> Result<Vec<String>, Never> {
        let Ok(name) = self.name();

        Ok(std::iter::once(name.to_string())
            .chain(rest.iter().map(|word| (*word).to_string()))
            .collect())
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
}

impl std::fmt::Display for Unprinted {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unprinted::Absent => write!(to, "there was no program to run"),
            Unprinted::Unran(program, fault) => write!(to, "{program} would not run: {fault}"),
        }
    }
}

impl std::error::Error for Unprinted {}

pub fn printed<Word: AsRef<str>>(arguments: &[Word]) -> Result<String, Unprinted> {
    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Err(Unprinted::Absent),
    };

    let program = program.as_ref();

    match Command::new(program).args(rest.iter().map(AsRef::as_ref)).output() {
        Ok(done) => Ok(String::from_utf8_lossy(&done.stdout).to_string()),
        Err(fault) => Err(Unprinted::Unran(program.to_string(), fault)),
    }
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "PATH is where the machine looks for a program, and a program this desktop did not write is what this crate is a list of. Four crates walked it to ask whether a program was installed and each said something different when it was unset, which is why `installed` is here beside it"
    )
)]
pub fn path() -> Result<Option<String>, Never> {
    Ok(match std::env::var("PATH") {
        Ok(said) => match said.is_empty() {
            true => None,
            false => Some(said),
        },
        Err(_unset) => None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installed {
    Yes,
    No,
}

pub fn installed(program: &str) -> Result<Installed, Never> {
    let Ok(path) = path();

    match (program.contains('/'), path) {
        (true, _) => runnable(Path::new(program)),
        (false, None) => Ok(Installed::No),
        (false, Some(path)) => {
            let found = path.split(':').filter(|at| !at.is_empty()).any(|at| {
                let Ok(runnable) = runnable(&Path::new(at).join(program));

                runnable == Installed::Yes
            });

            Ok(match found {
                true => Installed::Yes,
                false => Installed::No,
            })
        }
    }
}

const RUNNABLE: u32 = 0o111;

fn runnable(at: &Path) -> Result<Installed, Never> {
    let about = match std::fs::metadata(at) {
        Ok(about) => about,
        Err(_nothing_there) => return Ok(Installed::No),
    };

    Ok(match (about.is_file(), about.permissions().mode() & RUNNABLE) {
        (false, _) => Installed::No,
        (true, 0) => Installed::No,
        (true, _someone_can_run_it) => Installed::Yes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(said: &[&str]) -> Vec<String> {
        said.iter().map(|word| (*word).to_string()).collect()
    }

    #[test]
    fn what_a_program_printed_is_handed_back_and_one_that_cannot_run_says_so() {
        assert_eq!(printed(&words(&["sh", "-c", "printf said"])).expect("it ran"), "said");
        assert!(matches!(printed::<&str>(&[]), Err(Unprinted::Absent)));
        assert!(matches!(printed(&words(&["definitely-not-a-program-anybody-installed"])), Err(Unprinted::Unran(..))));
    }

    #[test]
    fn a_program_on_the_path_is_installed() {
        assert_eq!(installed("sh"), Ok(Installed::Yes));
        assert_eq!(installed("definitely-not-a-program-anybody-installed"), Ok(Installed::No));
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
    fn a_file_nobody_can_run_is_not_a_program() {
        let folder = console_core_temporary_directories::fresh("installed").expect("a folder of this test's own");
        let at = folder.join("unrunnable");
        std::fs::write(&at, "").expect("a file to look at");

        assert_eq!(installed(&at.to_string_lossy()), Ok(Installed::No));

        std::fs::remove_dir_all(&folder).expect("the folder taken away");
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

//! Every program this desktop runs and did not write.
//!
//! There was a table of these in a test, with a scan of the source under it:
//! it looked for a string literal at the front of an argv and asked the
//! machine whether a program by that name was installed. That is a net, and a
//! net is what gets built when the thing itself cannot be enumerated. This is
//! the thing itself. One variant per program, so the list is exhaustive
//! because the compiler says so, and a program nobody thought about is a
//! variant that does not exist rather than a word that slipped past a scan.
//!
//! [`Origin`] is the other half, and is what makes the list worth holding. A
//! program has to be on the machine before it will run, `desktop.conf` is what
//! puts it there, and for a year three of these were on the device only
//! because something else had dragged them in. So every [`Origin::Package`]
//! below is crossed against `[packages]` by a test, and a program that comes
//! with base Arch or only with a machine that develops this says which it is
//! rather than being absent from the list for a reason nobody wrote down.
//!
//! What the enum cannot reach is a program named inside another program's
//! arguments: `sh -c`, and the word after `stdbuf`. Those are written with
//! [`Program::name`], which is why it is public -- the program is still a
//! variant, it is simply spelled into an argument rather than run directly.

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
    Bash, "bash", Origin::Arch;
    Bluetoothctl, "bluetoothctl", Origin::Package("bluez-utils");
    Busctl, "busctl", Origin::Arch;
    Cargo, "cargo", Origin::Package("rust");
    Cat, "cat", Origin::Arch;
    Cmake, "cmake", Origin::Package("cmake");
    Cp, "cp", Origin::Arch;
    Curl, "curl", Origin::Package("curl");
    Date, "date", Origin::Arch;
    Df, "df", Origin::Arch;
    Du, "du", Origin::Arch;
    Echo, "echo", Origin::Arch;
    Env, "env", Origin::Arch;
    Ffmpeg, "ffmpeg", Origin::Package("ffmpeg");
    Ffprobe, "ffprobe", Origin::Package("ffmpeg");
    Gio, "gio", Origin::Package("glib2");
    Git, "git", Origin::Package("git");
    Grim, "grim", Origin::Package("grim");
    Hostname, "hostname", Origin::Developing;
    Hostnamectl, "hostnamectl", Origin::Arch;
    Hyprctl, "hyprctl", Origin::Package("hyprland");
    Hyprland, "Hyprland", Origin::Developing;
    Id, "id", Origin::Arch;
    Just, "just", Origin::Developing;
    Locale, "locale", Origin::Arch;
    LocaleGen, "locale-gen", Origin::Arch;
    Localectl, "localectl", Origin::Arch;
    Logger, "logger", Origin::Arch;
    Ls, "ls", Origin::Arch;
    Makoctl, "makoctl", Origin::Package("mako");
    Mkdir, "mkdir", Origin::Arch;
    Modprobe, "modprobe", Origin::Arch;
    Mv, "mv", Origin::Arch;
    Nmcli, "nmcli", Origin::Package("networkmanager");
    NotifySend, "notify-send", Origin::Package("libnotify");
    Pacman, "pacman", Origin::Arch;
    Pactl, "pactl", Origin::Package("libpulse");
    Pkill, "pkill", Origin::Arch;
    Powerprofilesctl, "powerprofilesctl", Origin::Package("power-profiles-daemon");
    PwCat, "pw-cat", Origin::Package("pipewire-audio");
    PwRecord, "pw-record", Origin::Package("pipewire-audio");
    Rm, "rm", Origin::Arch;
    Runuser, "runuser", Origin::Arch;
    Scp, "scp", Origin::Developing;
    SevenZip, "7z", Origin::Package("7zip");
    Sh, "sh", Origin::Arch;
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

    pub fn argv(self, rest: &[&str]) -> Result<Vec<String>, Never> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let held = names.len();
        names.dedup();

        assert_eq!(names.len(), held, "two variants share a name");
    }

    #[test]
    fn the_argv_starts_with_the_program() {
        let Ok(argv) = Program::Hyprctl.argv(&["monitors", "-j"]);

        assert_eq!(argv, ["hyprctl", "monitors", "-j"]);
    }

    #[test]
    fn owned_words_are_carried_the_same_way() {
        let told = vec!["trash".to_string(), "--".to_string()];

        let Ok(words) = Program::Gio.words(told);

        assert_eq!(words, ["gio", "trash", "--"]);
    }
}

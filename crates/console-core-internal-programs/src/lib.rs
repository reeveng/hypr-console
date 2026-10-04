//! Every program this desktop runs and did write.
//!
//! `console-core-external-programs` is the other half of this and holds the
//! older argument: a program has to be on the machine before it will run, so
//! the list is the thing itself rather than a scan, and an `Origin` says what
//! puts each one there. Our own programs are on the machine for exactly the
//! same kind of reason -- `[build]` in `desktop.conf` names them, the engine
//! compiles them, and a name that is not in that list is a program the device
//! has never heard of. A string at a call site is that claim made where nothing
//! can check it, which is what EXPLICIT036 is for; here the claim is one
//! variant, and `console-manifest-engine/tests/the_programs.rs` crosses every
//! variant with `[build]`.
//!
//! [`InternalProgram::at`] is the second half and is the reason this is not
//! only a list of words. `/usr/local/bin/console-say` is the installed one, and
//! the installed one is not always the one that should answer: a nested
//! desktop runs what was staged for it, and the checks run what was compiled a
//! moment ago. So a program of ours is looked for beside the one that is
//! running, then one directory up -- a test is built into `deps/`, a step below
//! the programs it drives -- and the bare name, which is `PATH`, which is the
//! installed one, is what answers when there is nothing in either. Three crates
//! had written that walk out separately before this crate existed, and the
//! stages had a fourth that knew about `deps/` when this did not, so a check
//! asking for one of these ran the installed copy. [`beside_this_program`] is
//! the walk for a program that is not on this list, which is the checks' own
//! tools.
//!
//! [`InternalProgram::path`] is where the engine installs a program, for the
//! callers that must not be answered by `PATH` -- a key the compositor binds, a
//! step a session runs before its environment is whole. [`EXECUTABLE_DIRECTORY`] is
//! that one directory, and the engine installs into it rather than spelling it
//! again.
//!
//! [`CONFIRM_DOES`] and [`CONFIRM_UNASKED`] are here for the same reason the
//! names are, and so is every other word of a program's own vocabulary that a
//! crate which cannot depend on the program starts it with. `console-confirm`
//! is started by `console-deploy` from a laptop,
//! inside someone else's session, over ssh -- so the two ends of that call are
//! in crates that cannot depend on each other, one of them being a GTK panel
//! and the other a tool that must not pull a toolkit in. The name of the
//! variable was spelled in both for as long as it took to notice, which is the
//! drift this crate exists to stop.
//!
//! `CONFIRM_UNASKED` is the half of that call that was missing. A card exits
//! zero for yes and one for no, and it was exiting one when it could not read
//! how it had been called -- so a program that never reached a person read, to
//! the caller, exactly like a person who said no. They are different answers
//! and the second is not the card's to give.
//!
//! [`LAUNCHER_KEEP`] and [`OVERVIEW_SHOW`] are the same shape for the
//! controller, which starts the menu and the overview on a press and must not
//! pull either one's drawing in to learn how. Each is a declared flag rather
//! than a word, so the program reads it off the same constant the controller
//! starts it with, and its usage draws from it too. [`WALLPAPER_TAKE`] and
//! [`WALLPAPER_DROPPED`] are the same again for the files and the settings,
//! which hand `wallpaper-render` a picture and must not pull its rendering in.

use console_core_arguments::{Flag, Takes};
use console_core_never::Never;
use std::path::{Path, PathBuf};
use std::process::Command;

macro_rules! executable_directory {
    () => {
        "/usr/local/bin"
    };
}

macro_rules! ours {
    ($($variant:ident, $name:literal;)*) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum InternalProgram {
            $($variant,)*
        }

        pub const EVERY: &[InternalProgram] = &[$(InternalProgram::$variant,)*];

        impl InternalProgram {
            pub const fn name(self) -> Result<&'static str, Never> {
                Ok(match self {
                    $(InternalProgram::$variant => $name,)*
                })
            }

            pub const fn path(self) -> Result<&'static str, Never> {
                Ok(match self {
                    $(InternalProgram::$variant => concat!(executable_directory!(), "/", $name),)*
                })
            }
        }
    };
}

ours! {
    Asking, "console-asking";
    Books, "books";
    BooksCatalog, "books-catalog";
    Brightness, "console-brightness";
    Browser, "console-browser";
    Calculator, "calculator";
    CalendarPanel, "calendar-panel";
    Confirm, "console-confirm";
    ConsoleSay, "console-say";
    Dictate, "console-dictate";
    Downloads, "downloads";
    DownloadsFind, "downloads-find";
    DownloadsGet, "downloads-get";
    Files, "files";
    ForecastPanel, "forecast-panel";
    KeyboardShow, "keyboard-show";
    KeyboardToggle, "keyboard-toggle";
    Launcher, "launcher";
    LoginGreeter, "login-greeter";
    LoginWindow, "login-window";
    MappingPanel, "mapping-panel";
    Music, "music";
    MusicIndex, "music-index";
    MusicOnward, "music-onward";
    MusicPanel, "music-panel";
    NightShift, "console-warm";
    Notes, "notes";
    NotificationsPanel, "notifications-panel";
    Overview, "console-overview";
    PanelPictures, "panel-pictures";
    PutAway, "console-put-away";
    Scale, "console-scale";
    Screenshot, "console-screenshot";
    SessionDesktop, "session-desktop";
    SessionGame, "session-game";
    SettingsLoginPattern, "settings-login-pattern";
    SettingsPanel, "settings-panel";
    Viewer, "viewer";
    Volume, "console-volume";
}

pub const APPS: [InternalProgram; 5] = [
    InternalProgram::Books,
    InternalProgram::Downloads,
    InternalProgram::Files,
    InternalProgram::Music,
    InternalProgram::Viewer,
];

pub const EXECUTABLE_DIRECTORY: &str = executable_directory!();

pub const CONFIRM_DOES: &str = "CONSOLE_CONFIRM_DOES";

pub const CONFIRM_UNASKED: u8 = 96;

pub const LAUNCHER_KEEP: Flag = Flag {
    spelling: "--keep",
    takes: Takes::None,
    about: "stay open when asked for again, rather than putting the menu away",
};

pub const OVERVIEW_SHOW: Flag = Flag {
    spelling: "--show",
    takes: Takes::None,
    about: "open the overview that is already running, rather than becoming it",
};

pub const WALLPAPER_TAKE: Flag = Flag {
    spelling: "--take",
    takes: Takes::None,
    about: "render the pictures at PATH, whatever and wherever they are",
};

pub const WALLPAPER_DROPPED: Flag = Flag {
    spelling: "--dropped",
    takes: Takes::None,
    about: "render what is in Pictures/Wallpapers",
};

impl InternalProgram {
    pub fn at(self) -> Result<PathBuf, Never> {
        let Ok(named) = self.name();

        beside_this_program(named)
    }

    pub fn command(self) -> Result<Command, Never> {
        let Ok(at) = self.at();

        Ok(Command::new(at))
    }
}

pub fn beside_this_program(named: &str) -> Result<PathBuf, Never> {
    let running = match std::env::current_exe() {
        Ok(running) => running,
        Err(_this_program_cannot_say_where_it_is) => return Ok(PathBuf::from(named)),
    };

    let beside = running.parent().map(Path::to_path_buf);
    let above_that = running.parent().and_then(Path::parent).map(Path::to_path_buf);

    let built = [beside, above_that]
        .into_iter()
        .flatten()
        .map(|at| at.join(named))
        .find(|at| at.is_file());

    Ok(match built {
        Some(built) => built,
        None => PathBuf::from(named),
    })
}

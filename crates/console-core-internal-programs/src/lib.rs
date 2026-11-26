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
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the directory literal is used on its own and in concat! when forming compile-time executable paths"
  )
)]
macro_rules! executable_directory {
  () => {
    "/usr/local/bin"
  };
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one entry supplies the program variant, its name, and its installed path so callers and the installer share the same register"
  )
)]
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
    Bluetooth, "console-bluetooth";
    Bar, "console-bar";
    Battery, "console-battery";
    Books, "console-application-books";
    BooksCatalog, "books-catalog";
    Brightness, "console-brightness";
    Browser, "console-browser";
    BrowserWindow, "console-application-browser";
    Calculator, "console-application-calculator";
    CalendarPanel, "calendar-panel";
    Camera, "console-application-camera";
    Clipboard, "console-application-clipboard";
    Compositor, "console-compositor";
    Confirm, "console-confirm";
    Console, "console";
    ControlCenter, "console-control-center";
    ConsoleSay, "console-say";
    Dictate, "console-dictate";
    Downloads, "console-application-downloads";
    DownloadsFind, "downloads-find";
    DownloadsFormat, "downloads-format";
    DownloadsGet, "downloads-get";
    Events, "console-events";
    Files, "console-application-files";
    FilesThumbnails, "files-thumbnails";
    FilesUnzip, "files-unzip";
    ForecastPanel, "forecast-panel";
    Habits, "console-application-habits";
    Hotkey, "console-hotkey";
    KeyboardShow, "keyboard-show";
    KeyboardToggle, "keyboard-toggle";
    LanguageSwitch, "language-switch";
    Launcher, "launcher";
    LoginGreeter, "login-greeter";
    LoginWindow, "login-window";
    MappingPanel, "mapping-panel";
    Music, "console-application-music";
    MusicIndex, "music-index";
    MusicOnward, "music-onward";
    MusicPanel, "music-panel";
    MusicPlayer, "music-player";
    NightShift, "console-warm";
    Notes, "console-application-notes";
    NotificationsPanel, "notifications-panel";
    Notify, "console-notify";
    Open, "console-open";
    Overview, "console-overview";
    PanelPictures, "panel-pictures";
    Panels, "console-panels";
    Point, "console-point";
    PutAway, "console-put-away";
    Scale, "console-scale";
    Screenshot, "console-screenshot";
    SessionDesktop, "session-desktop";
    SessionGame, "session-game";
    Resume, "console-resume";
    SettingsLoginPattern, "settings-login-pattern";
    SettingsPanel, "settings-panel";
    SoundEffect, "console-sound-effect";
    Tap, "console-tap";
    TaskManager, "console-application-task-manager";
    Update, "console-update";
    UpdatePattern, "console-update-pattern";
    Viewer, "console-application-media-viewer";
    Volume, "console-volume";
    Paper, "console-paper";
    Wallpaper, "console-wallpaper";
    WallpaperRender, "wallpaper-render";
}

pub const APPS: [InternalProgram; 12] = [
  InternalProgram::Books,
  InternalProgram::BrowserWindow,
  InternalProgram::Calculator,
  InternalProgram::Camera,
  InternalProgram::Clipboard,
  InternalProgram::Downloads,
  InternalProgram::Files,
  InternalProgram::Habits,
  InternalProgram::Music,
  InternalProgram::Notes,
  InternalProgram::TaskManager,
  InternalProgram::Viewer,
];

pub const EXECUTABLE_DIRECTORY: &str = executable_directory!();

pub const CONFIRM_DOES: &str = "CONSOLE_CONFIRM_DOES";

pub const CONFIRM_UNASKED: u8 = 96;

pub const CLIPBOARD_COPY_TEXT: Flag = Flag {
  spelling: "--copy-text",
  takes: Takes::None,
  about: "copy UTF-8 text from standard input to the clipboard",
};

pub const CLIPBOARD_READ_TEXT: Flag = Flag {
  spelling: "--read-text",
  takes: Takes::None,
  about: "print the active plain-text clipboard selection",
};

pub const KEYBOARD_AUTOMATIC_SHOW: Flag = Flag {
  spelling: "--automatic",
  takes: Takes::None,
  about: "raise the keyboard only when automatic opening is enabled",
};

pub const CLIPBOARD_COPY_PNG: Flag = Flag {
  spelling: "--copy-png",
  takes: Takes::None,
  about: "copy a PNG from standard input to the clipboard",
};

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

pub const SELECT_SCREENSHOT_AREA: Flag = Flag {
  spelling: "--screenshot-area",
  takes: Takes::None,
  about: "open the screenshot area selector in Control Center",
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
  let named_path = Path::new(named);
  let sibling = match Sibling::of(named_path) {
    Ok(Some(sibling)) => sibling,
    Ok(None) => return Ok(named_path.to_path_buf()),
  };
  let running = match std::env::current_exe() {
    Ok(running) => running,
    Err(_this_program_cannot_say_where_it_is) => return Ok(PathBuf::from(named)),
  };

  let beside = running.parent();
  let above_that = beside.and_then(Path::parent);

  let built = [beside, above_that]
    .into_iter()
    .flatten()
    .map(|at| at.join(sibling))
    .find(|at| match std::fs::metadata(at) {
      Ok(entry) => entry.is_file(),
      Err(_candidate_metadata_unavailable) => false,
    });

  Ok(match built {
    Some(built_2) => built_2,
    None => PathBuf::from(named),
  })
}

#[derive(Clone, Copy)]
struct Sibling<'a>(&'a Path);

impl<'a> Sibling<'a> {
  fn of(path: &'a Path) -> Result<Option<Self>, Never> {
    let stays_beside = path.file_name().is_some()
      && path
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));

    Ok(stays_beside.then_some(Self(path)))
  }
}

impl AsRef<Path> for Sibling<'_> {
  fn as_ref(&self) -> &Path {
    self.0
  }
}

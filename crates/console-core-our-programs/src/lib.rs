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
//! [`Ours::at`] is the second half and is the reason this is not only a list of
//! words. `/usr/local/bin/console-say` is the installed one, and the installed
//! one is not always the one that should answer: a nested desktop runs what was
//! staged for it, and the checks run what was compiled a moment ago. So a
//! program of ours is looked for beside the one that is running, and the bare
//! name -- which is `PATH`, which is the installed one -- is what answers when
//! there is nothing beside it. Three crates had written that walk out
//! separately before this crate existed.
//!
//! [`CONFIRM_DOES`] and [`CONFIRM_UNASKED`] are here for the same reason the
//! names are, and are the only words of a program's own vocabulary this list
//! carries. `console-confirm` is started by `console-deploy` from a laptop,
//! inside somebody else's session, over ssh -- so the two ends of that call are
//! in crates that cannot depend on each other, one of them being a GTK panel
//! and the other a tool that must not pull a toolkit in. The name of the
//! variable was spelled in both for as long as it took to notice, which is the
//! drift this crate exists to stop.
//!
//! `CONFIRM_UNASKED` is the half of that call that was missing. A card exits
//! nought for yes and one for no, and it was exiting one when it could not read
//! how it had been called -- so a program that never reached a person read, to
//! the caller, exactly like a person who said no. They are different answers
//! and the second is not the card's to give.

use console_core_never::Never;
use std::path::PathBuf;
use std::process::Command;

macro_rules! ours {
    ($($variant:ident, $name:literal;)*) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum Ours {
            $($variant,)*
        }

        pub const EVERY: &[Ours] = &[$(Ours::$variant,)*];

        impl Ours {
            pub const fn name(self) -> Result<&'static str, Never> {
                Ok(match self {
                    $(Ours::$variant => $name,)*
                })
            }
        }
    };
}

ours! {
    Confirm, "console-confirm";
    ConsoleSay, "console-say";
    Dictate, "console-dictate";
    KeyboardToggle, "keyboard-toggle";
    Launcher, "launcher";
    MusicPanel, "music-panel";
    NotificationsPanel, "notifications-panel";
    PanelPictures, "panel-pictures";
    SettingsPanel, "settings-panel";
}

pub const CONFIRM_DOES: &str = "CONSOLE_CONFIRM_DOES";

pub const CONFIRM_UNASKED: u8 = 96;

impl Ours {
    pub fn at(self) -> Result<PathBuf, Never> {
        let Ok(named) = self.name();

        let running = match std::env::current_exe() {
            Ok(running) => running,
            Err(_this_program_cannot_say_where_it_is) => return Ok(PathBuf::from(named)),
        };

        let beside = match running.parent() {
            Some(beside) => beside.join(named),
            None => return Ok(PathBuf::from(named)),
        };

        Ok(match beside.exists() {
            true => beside,
            false => PathBuf::from(named),
        })
    }

    pub fn command(self) -> Result<Command, Never> {
        let Ok(at) = self.at();

        Ok(Command::new(at))
    }
}

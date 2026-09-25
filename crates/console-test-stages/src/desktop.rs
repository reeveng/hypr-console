//! The device's own desktop, nested on this machine, and looked at.
//!
//! What this can answer that nothing else can is what color the screen is. A
//! service being active proves nothing about whether it is doing its job: the
//! wallpaper on the device did not paint for days because hyprpaper read a
//! config format it no longer understood, painted nothing, and reported
//! success. Nothing was in a failed state. The screen was the wrong color.
//!
//! It presses with the pointer. A finger needs /dev/uinput, and uinput is this
//! machine's -- a touchscreen made here puts its finger on the laptop, not
//! inside the picture, because the nested compositor is a client of this one
//! and hears only what this one forwards. The pointer has a way in that a
//! finger does not: `console-point` asks the nested compositor for a pointer of
//! its own, over the socket, and that pointer is inside the picture. So
//! everything the pointer does -- hovering, clicking, the wheel -- can be
//! pressed here, and a finger is still the device's tier.
//!
//! What it can answer besides the color is which windows the nested compositor
//! had at the moment the picture was taken. A screen says a window is somewhere;
//! whether a session was put back is a question about which windows exist, and
//! the device's stage has been able to ask that since it was written. This one
//! could only look. So the same run writes what `hyprctl clients` said beside
//! the picture and [`Desktop::windows`] reads it back -- one session answering
//! both questions, rather than two sessions disagreeing about what was up.
//!
//! Which is not a small thing to have gained. Before it, the only pressing
//! anywhere was on someone's actual handheld: a check for what the pointer
//! does could be written only for a machine that has to be plugged in, awake
//! and reachable, so mostly it was not written at all.
//!
//! The same run writes what `hyprctl monitors` said, for the same reason, and
//! [`Desktop::color`] divides by that rather than by the scale the device's
//! own config declares. This screen is the device's mode at whatever scale
//! leaves room on the machine running it, so the two numbers part company the
//! moment the window is cut down -- and a place worked out from the wrong one
//! is a row somewhere else on the screen, which reads exactly like a surface
//! that does not paint.
//!
//! [`Desktop::filling`] is the one thing here that puts something into the
//! session rather than taking something out of it. The strip under the bar
//! reads how far along a long thing is out of a file under `/run`, which is a
//! path the staging cannot rewrite and a directory no one may make on a laptop,
//! so the file is written here and the session is told where it is. It goes
//! where the picture goes and leaves with it.
//!
//! [`Desktop::timings`] is what the session wrote down about its own waits.
//! Every program in it writes to `waited.jsonl` under the staged home, which
//! goes when the session does, so [`Desktop::keeping_timings`] is a last press
//! that copies the store out beside the picture before it is taken.
//!
//! [`Desktop::notifying`] is the same idea and a firmer version of it. A
//! notification daemon is a program that takes a name on the session bus, and
//! the session bus a check inherits is the one this laptop's own desktop is
//! using: a daemon started in the nested session would take
//! `org.freedesktop.Notifications` away from whatever is holding it out here,
//! and the run would end with the machine it ran on unable to say anything. So
//! the nested session is given a bus of its own -- a `dbus-daemon` on a socket
//! beside the picture, dying with the check that started it -- and a notifications
//! file of its own beside it. A check may take away what it made and may not
//! take away what it found, and a bus name is the largest thing on this machine
//! that could be taken.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use console_compositor::{Window, Workspace};
use console_core_external_programs::Program;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted};
use console_notifications::updating::{self, Progress};
use console_notifications::serving;
use console_program_lifetime::{BoundToParent, alongside};
use console_waiting::{Schedule, Ready, Outcome, until};

use crate::Error;
use crate::picture::{Picture, where_};

const NESTING: &str = "console-desktop";


pub const PATIENCE: u64 = 180;

const DRAWN: f64 = 6.0;

const HAND: f64 = 0.6;

const BETWEEN: f64 = 0.5;

const AFTER: f64 = 1.5;

fn pressing_hand() -> Result<(), Error> {
    let Ok(nesting) = nesting_program();
    let beside = nesting.parent().map(|at| at.join("console-point"));

    match beside {
        Some(at) => match at.is_file() {
            true => Ok(()),
            false => Err(Error::NoPointer(at)),
        },
        None => Err(Error::PointerNotBeside),
    }
}

fn nesting_program() -> Result<PathBuf, Never> {
    let beside = match std::env::current_exe() {
        Ok(at) => at.parent().map(|at| at.join("console-desktop")),
        Err(fault) => {
            eprintln!("console-test-stages: where this program is: {fault}");

            None
        }
    };

    Ok(match beside.filter(|at| at.exists()) {
        Some(beside) => beside,
        None => PathBuf::from(NESTING),
    })
}

const SEEN: &str = "clients.json";

const TIMINGS: &str = "waited.jsonl";

const SCREEN: &str = "monitors.json";

const FILLING: &str = "updating";

const BUS: &str = "bus";

const SESSION: &str = "session.conf";

const ADDRESS: &str = "DBUS_SESSION_BUS_ADDRESS";

const LISTENING: Duration = Duration::from_secs(10);

const CONFIGURATION: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>@address@</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;

pub struct Desktop {
    open_these: Vec<String>,
    press_these: Vec<String>,
    patience: f64,
    not_before: Option<PathBuf>,
    filling: Option<Progress>,
    talking: Option<BoundToParent>,
    here: PathBuf,
    taken: Option<Picture>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installed {
    Yes,
    No,
}

impl Default for Desktop {
    fn default() -> Self {
        let Ok(desktop) = Desktop::new();

        desktop
    }
}

impl Desktop {
    pub fn new() -> Result<Self, Never> {
        let here = std::env::temp_dir().join(format!("console-desktop-{}", std::process::id()));

        Ok(Desktop {
            open_these: Vec::new(),
            press_these: Vec::new(),
            patience: 0.0,
            not_before: None,
            filling: None,
            talking: None,
            here,
            taken: None,
        })
    }

    pub fn fresh(&mut self) -> Result<(), Never> {
        self.open_these.clear();
        self.press_these.clear();
        self.patience = 0.0;
        self.not_before = None;
        self.filling = None;
        self.talking = None;
        self.taken = None;

        Ok(())
    }

    pub fn notifications(&self) -> Result<PathBuf, Never> {
        Ok(self.here.join(serving::KEPT))
    }

    pub fn notifying(&mut self) -> Result<(), Error> {
        match self.taken.is_some() {
            true => return Err(Error::AlreadyTaken("start the bus")),
            false => {},
        }

        match self.talking.is_some() {
            true => return Ok(()),
            false => {},
        }

        std::fs::create_dir_all(&self.here).map_err(Error::Machine)?;

        let socket = self.here.join(BUS);
        let _ = std::fs::remove_file(&socket);
        let address = format!("unix:path={}", socket.display());
        let at = self.here.join(SESSION);

        console_core_atomic_writes::whole(&at, CONFIGURATION.replace("@address@", &address).as_bytes())
            .map_err(Error::Unwritten)?;

        let Ok(mut asking) = Program::DbusDaemon.command();

        asking.arg(format!("--config-file={}", at.display())).arg("--nofork");
        asking.stdout(Stdio::null()).stderr(Stdio::null());

        let talking = alongside(&mut asking).map_err(Error::Machine)?;
        let Ok(patience) = Schedule::of(LISTENING);
        let Ok(listening) = until(patience, || {
            Ok(match socket.exists() {
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        });

        match listening {
            Outcome::Happened => {
                self.talking = Some(talking);

                Ok(())
            }
            Outcome::RanOut => Err(Error::NoBus(socket)),
        }
    }

    pub fn filling(&mut self, far: &Progress) -> Result<(), Error> {
        match self.taken.is_some() {
            true => Err(Error::AlreadyTaken("fill the strip")),
            false => {
                self.filling = Some(far.clone());

                Ok(())
            },
        }
    }

    pub fn not_before(&mut self, at: &Path) -> Result<(), Error> {
        match self.taken.is_some() {
            true => Err(Error::AlreadyTaken("say this")),
            false => {
                self.not_before = Some(at.to_path_buf());

                Ok(())
            },
        }
    }

    pub fn open(&mut self, command: &str) -> Result<(), Error> {
        match self.taken.is_some() {
            true => return Err(Error::AlreadyTaken("open")),
            false => {},
        }

        self.open_these.push(command.to_string());
        Ok(())
    }

    pub fn inside(&mut self, command: &str) -> Result<(), Error> {
        self.press(command.to_string())
    }

    pub fn waiting_inside(&mut self, command: &str, patience: f64) -> Result<(), Error> {
        self.press(command.to_string())?;
        self.patience += patience;

        Ok(())
    }

    pub fn keeping_timings(&mut self) -> Result<(), Error> {
        let kept = self.here.join(TIMINGS);

        self.press(format!("cp \"$XDG_STATE_HOME/console/waited.jsonl\" {}", kept.display()))
    }

    pub fn timings(&mut self) -> Result<Vec<console_response_times::line::Entry>, Error> {
        self.picture()?;

        let at = self.here.join(TIMINGS);
        let said = std::fs::read_to_string(&at).map_err(|fault| Error::Read(at.clone(), fault))?;

        Ok(said
            .lines()
            .filter_map(|line| match console_response_times::line::read(line) {
                Ok(entry) => entry,
                Err(never) => match never {},
            })
            .collect())
    }

    pub fn point(&mut self, at: (u32, u32)) -> Result<(), Error> {
        self.pointing(None, at, "")
    }

    pub fn click(&mut self, at: (u32, u32)) -> Result<(), Error> {
        self.pointing(None, at, " --click")
    }

    pub fn scroll(&mut self, at: (u32, u32), notches: i32) -> Result<(), Error> {
        self.pointing(None, at, &format!(" --scroll {notches}"))
    }

    pub fn point_in(&mut self, namespace: &str, at: (u32, u32)) -> Result<(), Error> {
        self.pointing(Some(namespace), at, "")
    }

    pub fn click_in(&mut self, namespace: &str, at: (u32, u32)) -> Result<(), Error> {
        self.pointing(Some(namespace), at, " --click")
    }

    pub fn drag_in(&mut self, namespace: &str, from: (u32, u32), through: &[(u32, u32)]) -> Result<(), Error> {
        let places: Vec<String> = through.iter().map(|(across, down)| format!("{across} {down}")).collect();

        self.pointing(Some(namespace), from, &format!(" --drag {}", places.join(" ")))
    }

    pub fn scroll_in(
        &mut self,
        namespace: &str,
        at: (u32, u32),
        notches: i32,
    ) -> Result<(), Error> {
        self.pointing(Some(namespace), at, &format!(" --scroll {notches}"))
    }

    fn pointing(
        &mut self,
        inside: Option<&str>,
        at: (u32, u32),
        doing: &str,
    ) -> Result<(), Error> {
        let within = match inside {
            Some(namespace) => format!("--in {namespace} "),
            None => String::new(),
        };

        self.press(format!("console-point {within}{} {}{doing}", at.0, at.1))
    }

    fn press(&mut self, command: String) -> Result<(), Error> {
        match self.taken {
            Some(_) => Err(Error::AlreadyTaken("press")),
            None => {
                self.press_these.push(command);

                Ok(())
            },
        }
    }

    fn pressing(&self) -> Result<Option<(String, f64)>, Never> {
        let (first, rest) = match self.press_these.split_first() {
            Some((first, rest)) => (first, rest),
            None => return Ok(None),
        };

        let mut script = format!("sleep {DRAWN}; {first}");

        for command in rest {
            script.push_str(&format!("; sleep {BETWEEN}; {command}"));
        }

        let Ok(pressed) = fitted::<_, u64>(self.press_these.len());
        let Ok(many) = pressed.float();

        let waited = DRAWN + many * (HAND + BETWEEN) + self.patience + AFTER;

        Ok(Some((script, waited)))
    }

    fn picture(&mut self) -> Result<&Picture, Error> {
        match self.taken.is_none() {
            true => {
                std::fs::create_dir_all(&self.here).map_err(Error::Machine)?;
                let _ = std::fs::remove_file(self.here.join(SEEN));
                let _ = std::fs::remove_file(self.here.join(SCREEN));
                let shot = self.here.join("screen.png");
                let Ok(program) = nesting_program();
                let mut nesting = Command::new(program);
                nesting.arg("shot").arg(&shot);
                nesting.arg("--clients").arg(self.here.join(SEEN));
                nesting.arg("--monitors").arg(self.here.join(SCREEN));

                match &self.filling {
                    Some(far) => {
                        let at = self.here.join(FILLING);
                        let Ok(written) = updating::written(far);

                        console_core_atomic_writes::whole(&at, written.as_bytes())
                            .map_err(Error::Unwritten)?;
                        nesting.env(updating::WHERE, &at);
                    }
                    None => {},
                }

                match &self.talking {
                    Some(_talking) => {
                        nesting.env(ADDRESS, format!("unix:path={}", self.here.join(BUS).display()));
                        nesting.env(serving::WHERE, self.here.join(serving::KEPT));
                    }
                    None => {},
                }

                match &self.not_before {
                    Some(at) => {
                        nesting.arg("--until").arg(at);
                    }
                    None => {},
                }

                for command in &self.open_these {
                    nesting.args(["--open", command]);
                }

                let Ok(pressing) = self.pressing();

                match pressing {
                    Some((script, waited)) => {
                        pressing_hand()?;

                        nesting.args(["--open", &script]);
                        nesting.args(["--settle", &format!("{waited:.1}")]);
                    }
                    None => {},
                }

                let said = nesting.output().map_err(Error::Machine)?;

                match shot.exists() {
                    true => {},
                    false => {
                        let why = String::from_utf8_lossy(&said.stderr);
                        let last = match why.trim().lines().next_back() {
                            Some(last) => last.to_string(),
                            None => String::new(),
                        };
                        return Err(Error::TookNoPicture(last));
                    }
                }

                let picture = Picture::read(&shot)?;

                self.taken = Some(picture);
            }
            false => {},
        }

        self.taken.as_ref().ok_or(Error::NestedPictureGone)
    }

    pub fn installed(&self, program: &str) -> Result<Installed, Never> {
        let Ok(quoted) = crate::device::quoted(program);
        let Ok(mut asking) = Program::Sh.command();

        let found = asking
            .args(["-c", &format!("command -v {quoted}")])
            .output()
            .is_ok_and(|done| done.status.success());

        Ok(match found {
            true => Installed::Yes,
            false => Installed::No,
        })
    }

    pub fn color(&mut self, at: Point<f64>) -> Result<String, Error> {
        let logical = self.logical()?;
        let picture = self.picture()?;

        where_(picture, at, logical)
    }

    pub fn logical(&mut self) -> Result<Size<u32>, Error> {
        self.picture()?;

        let at = self.here.join(SCREEN);
        let said = std::fs::read_to_string(&at)
            .map_err(|fault| Error::NoScreenSaid(at.clone(), fault))?;
        let read = console_compositor::read_value(&said)?;
        let Ok(monitors) = console_compositor::monitors(&read);

        let first = monitors.first().ok_or_else(|| Error::NoScreenAtAll(at.clone()))?;
        let Ok(logical) = first.logical();

        logical.ok_or_else(|| Error::NoSize(at.clone(), first.named.clone()))
    }

    pub fn front(&mut self) -> Result<Workspace, Error> {
        self.picture()?;

        let at = self.here.join(SCREEN);
        let said = std::fs::read_to_string(&at)
            .map_err(|fault| Error::NoScreenSaid(at.clone(), fault))?;
        let read = console_compositor::read_value(&said)?;
        let first = read
            .as_array()
            .and_then(|monitors| monitors.first())
            .ok_or_else(|| Error::NoScreenAtAll(at.clone()))?;
        let Ok(front) = console_compositor::front_of(first);

        front.ok_or_else(|| Error::NoScreenAtAll(at.clone()))
    }

    pub fn patch(&mut self, at: Point<f64>) -> Result<String, Error> {
        let picture = self.picture()?;
        let Ok(average) = picture.average(at, crate::picture::PATCH);

        Ok(average)
    }

    pub fn background(&mut self) -> Result<String, Error> {
        let picture = self.picture()?;
        let Ok(most_common) = picture.most_common();

        Ok(most_common)
    }

    pub fn windows(&mut self) -> Result<Vec<Window>, Error> {
        self.picture()?;

        let at = self.here.join(SEEN);

        let said = std::fs::read_to_string(&at)
            .map_err(|fault| Error::NoWindowsSaid(at.clone(), fault))?;

        let clients = console_compositor::read_value(&said)?;
        let Ok(open) = console_compositor::windows_open(&clients);

        Ok(open)
    }

    pub fn close(&mut self) -> Result<(), Never> {
        let _ = std::fs::remove_dir_all(&self.here);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new() -> Desktop {
        let Ok(desktop) = Desktop::new();

        desktop
    }

    #[test]
    fn nothing_can_be_opened_once_the_picture_has_been_taken() {
        let mut desktop = new();
        assert!(desktop.open("alacritty").is_ok());
        desktop.taken = None;
        assert!(desktop.open("mapping-panel").is_ok());
        assert_eq!(desktop.open_these.len(), 2);
    }

    #[test]
    fn a_fresh_desktop_has_nothing_open_and_nothing_looked_at() {
        let mut desktop = new();
        desktop.open("alacritty").expect("something to open");
        desktop.point((10, 20)).expect("somewhere to point");
        desktop.fresh();
        assert!(desktop.open_these.is_empty());
        assert!(desktop.press_these.is_empty());
    }

    #[test]
    fn nothing_pressed_is_a_session_that_is_not_kept_open_for_it() {
        let Ok(pressing) = new().pressing();

        assert_eq!(pressing, None);
    }

    #[test]
    fn the_presses_are_one_script_in_the_order_they_were_asked_for() {
        let mut desktop = new();
        desktop.point((10, 20)).expect("somewhere to point");
        desktop.click((30, 40)).expect("somewhere to click");
        desktop.scroll((30, 40), -2).expect("somewhere to scroll");

        let Ok(pressing) = desktop.pressing();
        let (script, waited) = pressing.expect("a script");

        assert_eq!(
            script,
            format!(
                "sleep {DRAWN}; console-point 10 20; sleep {BETWEEN}; console-point 30 40 --click; \
                 sleep {BETWEEN}; console-point 30 40 --scroll -2"
            )
        );
        assert!(waited > DRAWN + AFTER, "the session goes before the last press: {waited}");
    }
}

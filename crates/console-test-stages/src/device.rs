//! The machine itself, pressed through InputPlumber and looked at over ssh.
//!
//! Nothing here makes an input device. InputPlumber is asked to emit the event
//! it would have read from the hardware, through the profile that is loaded,
//! which is its own supported way of doing this and is what a chord on the
//! device already uses. So there is no second pad for the daemons to find and
//! nothing to clean up if a check stops halfway.
//!
//! Every press is a chord, and that is not a shorthand for a press. InputPlumber
//! has two ways to be asked for an event and only one of them works: `SendEvent`
//! reaches a `blocking_send` on a tokio worker and panics the daemon's own task
//! rather than emitting anything, on every version this desktop has run, so a
//! button cannot be held down and a trigger cannot be pulled. `SendButtonChord`
//! presses and lets go by itself and is sound. So a walk is taps rather than a
//! hold, a key is a chord of `Keyboard:` capabilities, and the two that need a
//! button held say they cannot rather than sending something the daemon drops on
//! the floor. todos.md is the rest, and this comes back the day it is fixed.
//!
//! A chord also lets go in its own time, and the call comes back before it does.
//! Asking for one puts the button down, answers, and lets go eighty milliseconds
//! later; a second chord inside that window is not a second press, because the
//! axis is already where it would put it and there is nothing for the kernel to
//! report. Eighteen of them in a shell loop take sixty-six milliseconds and reach
//! the pad as one press and one release, which is what `290` was walking on: a
//! walk of eighteen squares that moved one, or four on a slower loop, with no
//! error anywhere to say which. So a walk waits `LET_GO` between one chord and
//! the next. The number is the daemon's rather than this file's, and reading it
//! again means asking for a chord and watching the pad's own node for what came
//! out of it.
//!
//! What a press then does to the machine is another matter, and it is somebody
//! else's machine. Two things here are the bookkeeping that lets a run give it
//! back the way it found it: a window opened through `open` is remembered, so
//! `close_window` can end the process the run started rather than something
//! the person was using, and a level is read as a `Level` rather than as a
//! number, so a reading nobody got is never mistaken for a screen at nought.


use console_core_external_programs::Program;
use console_core_number_conversion::{fitted, toward_zero_u32, whole_u32};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use console_core_never::Never;
use console_input_gamepad::profile::{Kind, Profile};
use console_input_gamepad::router::every_profile;
use console_input_gamepad::vocabulary;

use crate::checking::{Done, cannot, failed};
use crate::picture::{Picture, where_};

pub fn host() -> Result<String, String> {
    match std::env::var("CONSOLE_HOST") {
        Ok(said) if !said.trim().is_empty() => Ok(said),
        Ok(_) | Err(_) => Err("CONSOLE_HOST is not set, so there is no device to talk to. \
                  Set it to the device, as in CONSOLE_HOST=root@handheld."
            .to_string()),
    }
}

const MARK: &str = "@user@";

const PIECES: [&str; 14] = [
    "console-input-controller",
    "console-input-keyboard",
    "console-bar",
    "console-notify",
    "console-panels",
    "console-polkit",
    "console-session",
    "console-paper",
    "console-sky",
    "console-events",
    "console-home",
    "console-idle",
    "console-warm",
    "syncthing",
];

const BY_A_SWITCH: &str = "console-warm";

const SENDEVENT: &str = "InputPlumber's SendEvent panics on its own runtime rather than \
     emitting anything, so nothing here can hold a button down or pull a trigger; \
     press instead, and see todos.md";

const BUS: (&str, &str, &str) = (
    "org.shadowblip.InputPlumber",
    "/org/shadowblip/InputPlumber/CompositeDevice0",
    "org.shadowblip.Input.CompositeDevice",
);

fn session_env(whom: &str) -> Result<String, Never> {
    Ok(format!(
        "export XDG_RUNTIME_DIR=/run/user/$(id -u {whom}); \
         export HYPRLAND_INSTANCE_SIGNATURE=$(ls -1t \"$XDG_RUNTIME_DIR/hypr\" 2>/dev/null | head -1); \
         export WAYLAND_DISPLAY=$(ls -1t \"$XDG_RUNTIME_DIR\" 2>/dev/null \
         | grep -E '^wayland-[0-9]+$' | head -1); \
         [ -n \"$HYPRLAND_INSTANCE_SIGNATURE\" ] || \
         {{ echo 'nothing on the device is in a Hyprland session' >&2; exit 1; }}"
    ))
}

pub const LET_GO: f64 = 0.12;

pub const SETTLED: f64 = 0.6;

pub const PATIENCE: f64 = 4.0;

pub const OPENING: f64 = 12.0;

pub const LEAVING: f64 = 12.0;

pub const FURNITURE: [&str; console_input_controller::mode::FURNITURE.len() + 1] = {
    let mut every = [""; console_input_controller::mode::FURNITURE.len() + 1];
    let mut at = 0;

    while at < console_input_controller::mode::FURNITURE.len() {
        every[at] = console_input_controller::mode::FURNITURE[at];
        at += 1;
    }

    every[at] = "hyprpaper";
    every
};

fn spoken_as(kind: Kind, name: &str) -> Result<Option<String>, Never> {
    Ok(match kind {
        Kind::Key => Some(format!("Keyboard:{name}")),
        Kind::MouseButton => Some(format!("Mouse:Button:{name}")),
        Kind::GamepadButton => Some(format!("Gamepad:Button:{name}")),
        Kind::MouseMotion | Kind::GamepadAxis | Kind::GamepadTrigger => None,
    })
}

pub struct Device {
    pub host: String,
    whom: Option<String>,
    pub dry: bool,
    pub done: Vec<String>,
    profiles: BTreeMap<String, Profile>,
    taken: Option<Picture>,
    kept: Option<PathBuf>,
    opened: Vec<String>,
    pushed: Pushed,
    screen: Option<console_screen::Screen>,
    watching: Option<std::sync::Arc<std::sync::Mutex<crate::watching::Watching>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pushed {
    Something,
    Nothing,
}

fn named<'a>(table: &'a [(&'a str, &'a str)], spoken: &str) -> Result<Option<&'a str>, Never> {
    Ok(table.iter().find(|(said, _)| *said == spoken).map(|(_, name)| *name))
}

fn said(table: &[(&str, &str)]) -> Result<String, Never> {
    let every: Vec<&str> = table.iter().map(|(spoken, _)| *spoken).collect();

    Ok(format!("try one of {}", every.join(", ")))
}

fn menus_up(seen: &mut Device) -> Result<Seen, Never> {
    let Ok(menus) = seen.menus();

    Ok(match menus.is_empty() {
        true => Seen::NotYet,
        false => Seen::Yes,
    })
}

pub use console_waiting::{Seen, Waited};

pub const A_MOMENT: f64 = 2.0;

pub const A_PICTURE: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dry {
    Pretend,
    Really,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    At(i64),
    Unsaid,
}

impl Level {
    pub fn told(self) -> Result<Option<i64>, Never> {
        Ok(match self {
            Level::At(level) => Some(level),
            Level::Unsaid => None,
        })
    }
}

fn brightness_in(said: &str) -> Result<Level, Never> {
    let line = match said.lines().next() {
        Some(line) => line,
        None => return Ok(Level::Unsaid),
    };

    Ok(match line.trim().parse() {
        Ok(brightness) => Level::At(brightness),
        Err(_unreadable) => Level::Unsaid,
    })
}

fn volume_in(said: &str) -> Result<Level, Never> {
    let word = said.lines().next().and_then(|line| line.split_whitespace().nth(4));

    let word = match word {
        Some(word) => word,
        None => return Ok(Level::Unsaid),
    };

    Ok(match word.trim_end_matches('%').parse() {
        Ok(volume) => Level::At(volume),
        Err(_unreadable) => Level::Unsaid,
    })
}

impl Device {
    pub fn new(host: &str, dry: Dry) -> Result<Self, String> {
        let Ok(root) = crate::root();
        let profiles = every_profile(&root)?;

        Ok(Device {
            host: host.to_string(),
            dry: dry == Dry::Pretend,
            done: Vec::new(),
            profiles,
            taken: None,
            kept: None,
            opened: Vec::new(),
            whom: None,
            pushed: Pushed::Nothing,
            screen: None,
            watching: None,
        })
    }

    pub fn whoever(&mut self) -> Result<String, Never> {
        match &self.whom {
            Some(known) => return Ok(known.clone()),
            None => {},
        }

        let said = match std::env::var("CONSOLE_USER") {
            Ok(said) if !said.trim().is_empty() => said.trim().to_string(),
            Ok(_) | Err(_) => {
                let Ok(said) = self.ssh(
                    "set -- $(ls -1 /home 2>/dev/null); \
                     if [ $# -eq 1 ]; then echo \"$1\"; else id -nu 1000; fi",
                );

                said
            }
        };

        let said = match said.trim().is_empty() {
            true => MARK.to_string(),
            false => said.trim().to_string(),
        };

        self.whom = Some(said.clone());

        Ok(said)
    }

    pub fn home(&mut self) -> Result<String, Never> {
        let Ok(whom) = self.whoever();

        Ok(format!("/home/{whom}"))
    }

    pub fn watching(
        &mut self,
        watching: std::sync::Arc<std::sync::Mutex<crate::watching::Watching>>,
    ) -> Result<(), Never> {
        self.watching = Some(watching);

        Ok(())
    }

    fn along(&mut self) -> Result<String, Never> {
        let watching = match self.watching.as_ref() {
            Some(watching) => watching,
            None => return Ok(String::new()),
        };

        let Ok(mut held) = crate::watching::held(watching);

        held.tick()
    }

    pub fn ssh(&mut self, command: &str) -> Result<String, Never> {
        self.done.push(command.to_string());

        match self.dry {
            true => return Ok(String::new()),
            false => {},
        }

        let Ok(along) = self.along();
        let asked = format!("{command}{along}");
        let Ok(mut asking) = Program::Ssh.command();

        let done = asking
            .args(["-o", "BatchMode=yes", &self.host, &asked])
            .output();

        Ok(match done {
            Ok(done) => String::from_utf8_lossy(&done.stdout).trim().to_string(),
            Err(fault) => {
                eprintln!("console-test-stages: ssh {}: {fault}", self.host);

                String::new()
            }
        })
    }

    pub fn in_session(&mut self, command: &str) -> Result<String, Never> {
        let Ok(whom) = self.whoever();
        let Ok(session) = session_env(&whom);
        let asked = format!("{session} && {command}");

        self.user(&asked)
    }

    pub fn user(&mut self, command: &str) -> Result<String, Never> {
        let Ok(whom) = self.whoever();
        let Ok(quoted) = quoted(command);
        let asked = format!("machinectl shell --uid={whom} .host /bin/sh -c {quoted}");

        self.ssh(&asked)
    }

    pub fn hypr(&mut self, command: &str) -> Result<String, Never> {
        self.in_session(&format!("hyprctl {command}"))
    }

    fn loaded(&mut self) -> Result<Option<Profile>, Never> {
        let Ok(profile) = self.profile();
        let loaded = profile.to_lowercase();

        Ok(self
            .profiles
            .values()
            .find(|known| known.name.to_lowercase() == loaded)
            .cloned())
    }

    fn capability(&mut self, button: &str) -> Result<Option<String>, Never> {
        let Ok(loaded) = self.loaded();

        capability_under(loaded.as_ref(), button)
    }

    fn chorded(&mut self, capabilities: &[String]) -> Result<String, Never> {
        self.taken = None;

        let Ok(chord) = chord(capabilities);
        let Ok(asked) = calling(&chord);

        self.ssh(&asked)
    }

    pub fn press(&mut self, button: &str) -> Result<(), Never> {
        let Ok(found) = self.capability(button);

        let capability = match found {
            Some(capability) => capability,
            None => return Ok(()),
        };

        let Ok(_) = self.chorded(&[capability]);

        Ok(())
    }

    pub fn presses(&mut self, button: &str, times: usize) -> Result<(), Never> {
        self.taken = None;

        let Ok(found) = self.capability(button);

        let capability = match found {
            Some(capability) => capability,
            None => return Ok(()),
        };

        let Ok(chord) = chord(&[capability]);
        let Ok(asked) = calling(&chord);
        let Ok(_) = self.ssh(&format!(
            "for _ in $(seq {times}); do {asked}; sleep {LET_GO}; done"
        ));

        Ok(())
    }

    pub fn hold(&mut self, _button: &str) -> Done {
        cannot(SENDEVENT)
    }

    pub fn release(&mut self, _button: Option<&str>) -> Done {
        cannot(SENDEVENT)
    }

    pub fn trigger(&mut self, which: &str, amount: f64) -> Done {
        let Ok(found) = named(&vocabulary::TRIGGERS, which);

        let named = match found {
            Some(named) => named,
            None => {
                let Ok(said) = said(&vocabulary::TRIGGERS);

                return failed(format!("there is no trigger called {which:?}; {said}"));
            }
        };

        match (0.0..=1.0).contains(&amount) {
            true => {},
            false => return failed(format!("a trigger is pulled between 0 and 1, not {amount}")),
        }

        let Ok(()) = self.pushed(amount);

        self.axis(&format!("Gamepad:Trigger:{named}"), &format!("d {amount:.3}"))
    }

    pub fn stick(&mut self, which: &str, across: f64, down: f64) -> Done {
        let Ok(found) = named(&vocabulary::AXES, which);

        let named = match found {
            Some(named) => named,
            None => {
                let Ok(said) = said(&vocabulary::AXES);

                return failed(format!("there is no stick called {which:?}; {said}"));
            }
        };

        for amount in [across, down] {
            match (-1.0..=1.0).contains(&amount) {
                true => {},
                false => return failed(format!("a stick is pushed between -1 and 1, not {amount}")),
            }
        }

        let Ok(()) = self.pushed(across.abs().max(down.abs()));

        self.axis(&format!("Gamepad:Axis:{named}"), &format!("\"(dd)\" {across:.3} {down:.3}"))
    }

    pub fn let_go(&mut self) -> Done {
        self.pushed = Pushed::Nothing;

        for (spoken, _) in vocabulary::AXES {
            self.stick(spoken, 0.0, 0.0)?;
        }

        for (spoken, _) in vocabulary::TRIGGERS {
            self.trigger(spoken, 0.0)?;
        }

        Ok(())
    }

    fn pushed(&mut self, amount: f64) -> Result<(), Never> {
        self.taken = None;

        match amount == 0.0 {
            true => {},
            false => self.pushed = Pushed::Something,
        }

        Ok(())
    }

    fn axis(&mut self, capability: &str, value: &str) -> Done {
        let Ok(quoted) = quoted(capability);
        let asked = format!(
            "busctl --system call {} {} {} SendEvent sv {quoted} {value} 2>&1",
            BUS.0, BUS.1, BUS.2,
        );
        let Ok(said) = self.ssh(&asked);

        match said.contains("Call failed") {
            true => failed(format!("{capability} was not sent: {}", said.trim())),
            false => Ok(()),
        }
    }

    pub fn tap(&mut self, _across: i32, _down: i32) -> Done {
        cannot(
            "InputPlumber will not parse its own Touchpad: capabilities back, \
             so the trackpad cannot be sent; point or touch instead",
        )
    }

    pub fn touch(&mut self, at: (u32, u32)) -> Done {
        self.taken = None;

        let Ok(said) = self.ssh(&format!("console-tap {} {} 2>&1", at.0, at.1));

        match said.contains("console-tap:") {
            true => cannot(&format!("nothing could be pressed at {at:?}: {}", said.trim())),
            false => Ok(()),
        }
    }

    pub fn point(&mut self, at: (u32, u32)) -> Done {
        self.pointing(None, at, String::new())
    }

    pub fn click(&mut self, at: (u32, u32)) -> Done {
        self.pointing(None, at, " --click".to_string())
    }

    pub fn scroll(&mut self, at: (u32, u32), notches: i32) -> Done {
        self.pointing(None, at, format!(" --scroll {notches}"))
    }

    pub fn point_in(&mut self, namespace: &str, at: (u32, u32)) -> Done {
        self.pointing(Some(namespace), at, String::new())
    }

    pub fn click_in(&mut self, namespace: &str, at: (u32, u32)) -> Done {
        self.pointing(Some(namespace), at, " --click".to_string())
    }

    pub fn scroll_in(&mut self, namespace: &str, at: (u32, u32), notches: i32) -> Done {
        self.pointing(Some(namespace), at, format!(" --scroll {notches}"))
    }

    fn pointing(&mut self, inside: Option<&str>, at: (u32, u32), doing: String) -> Done {
        self.taken = None;

        let within = match inside {
            Some(namespace) => format!("--in {namespace} "),
            None => String::new(),
        };

        let Ok(said) =
            self.in_session(&format!("console-point {within}{} {}{doing} 2>&1", at.0, at.1));

        match (said.contains("console-point:"), said.contains("not found")) {
            (_, true) => cannot("this device has no console-point; deploy before pointing at it"),
            (true, false) => {
                failed(format!("the pointer could not be put at {at:?}: {}", said.trim()))
            },
            (false, false) => Ok(()),
        }
    }

    pub fn home_awake(&mut self) -> Result<Seen, Never> {
        let Ok(said) =
            self.user("test -e \"$XDG_RUNTIME_DIR/console/home-awake\" && echo awake");

        Ok(match said.contains("awake") {
            true => Seen::Yes,
            false => Seen::NotYet,
        })
    }

    pub fn home_carrying(&mut self) -> Result<Seen, Never> {
        let Ok(said) =
            self.user("test -e \"$XDG_RUNTIME_DIR/console/home-carrying\" && echo carrying");

        Ok(match said.contains("carrying") {
            true => Seen::Yes,
            false => Seen::NotYet,
        })
    }

    pub fn layer(&mut self, namespace: &str) -> Result<Option<(u32, u32, u32, u32)>, Never> {
        let Ok(said) = self.hypr("layers -j");
        let Ok(read) = read(&said);

        let found = match read {
            Some(found) => found,
            None => return Ok(None),
        };

        let Ok(surfaces) = console_compositor::surfaces(&found);

        for layer in surfaces {
            let Ok(named) = console_compositor::namespace(layer);

            match named == Some(namespace) {
                true => {},
                false => continue,
            }

            let Ok(said) = console_compositor::corner(layer);

            let corner = match said {
                Some(corner) => corner,
                None => return Ok(None),
            };

            let Ok(x) = fitted::<i64, u32>(corner.across);
            let Ok(y) = fitted::<i64, u32>(corner.down);
            let Ok(wide) = fitted::<i64, u32>(corner.wide);
            let Ok(tall) = fitted::<i64, u32>(corner.tall);

            return Ok(Some((x, y, wide, tall)));
        }

        Ok(None)
    }

    pub fn load_profile(&mut self, name: &str) -> Result<(), Never> {
        let Ok(quoted) = quoted(name);
        let Ok(_) = self.user(&format!("controller-profile {quoted}"));

        Ok(())
    }

    pub fn exec_cmd(&mut self, command: &str) -> Result<String, Never> {
        let Ok(quoted) = quoted(&format!("hl.dsp.exec_cmd(\"{command}\")"));

        self.hypr(&format!("dispatch {quoted}"))
    }

    pub fn addresses(&mut self) -> Result<Vec<String>, Never> {
        let Ok(clients) = self.clients();

        Ok(clients
            .iter()
            .filter_map(|client| {
                let Ok(address) = address(client);

                address
            })
            .collect())
    }

    pub fn opened(&mut self) -> Result<Vec<String>, Never> {
        let Ok(open) = self.addresses();

        Ok(self.opened.iter().filter(|which| open.contains(which)).cloned().collect())
    }

    pub fn window_gone(&mut self, which: &str, seconds: f64) -> Result<Waited, Never> {
        let going = which.to_string();

        self.until::<Never>(
            |seen| {
                let Ok(open) = seen.addresses();

                Ok(match open.contains(&going) {
                    true => Seen::NotYet,
                    false => Seen::Yes,
                })
            },
            seconds,
        )
    }

    fn pid_at(&mut self, which: &str) -> Result<Option<i64>, Never> {
        let Ok(clients) = self.clients();

        Ok(clients
            .iter()
            .find(|client| {
                let Ok(found) = address(client);

                found.as_deref() == Some(which)
            })
            .and_then(|client| client.get("pid"))
            .and_then(serde_json::Value::as_i64))
    }

    pub fn close_window(&mut self, which: &str) -> Result<Waited, Never> {
        let closing = which.to_string();

        self.opened.retain(|address| *address != closing);

        match self.dry {
            true => return Ok(Waited::Happened),
            false => {},
        }

        let Ok(pid) = self.pid_at(&closing);

        let pid = match pid {
            Some(pid) => pid,
            None => return Ok(Waited::Happened),
        };

        let Ok(_) = self.user(&format!("kill {pid}"));

        self.window_gone(&closing, LEAVING)
    }

    pub fn go_to(&mut self, workspace: &str) -> Result<Waited, Never> {
        let wanted = workspace.to_string();
        let Ok(quoted) = quoted(&format!("hl.dsp.focus({{workspace = \"{wanted}\"}})"));
        let Ok(_) = self.hypr(&format!("dispatch {quoted}"));

        self.until::<Never>(
            |seen| {
                let Ok(now) = seen.workspace();

                Ok(match now == wanted {
                    true => Seen::Yes,
                    false => Seen::NotYet,
                })
            },
            A_MOMENT,
        )
    }

    pub fn open(&mut self, command: &str, seconds: f64) -> Result<Waited, Never> {
        let Ok(which) = self.opening(command, seconds);

        Ok(match which {
            Some(_) => Waited::Happened,
            None => Waited::RanOut,
        })
    }

    pub fn opening(&mut self, command: &str, seconds: f64) -> Result<Option<String>, Never> {
        match self.dry {
            true => {
                let Ok(_) = self.exec_cmd(command);

                return Ok(Some(String::new()));
            }
            false => {},
        }

        let Ok(was) = self.addresses();
        let Ok(_) = self.exec_cmd(command);
        let mut where_ = String::new();
        let mut which = String::new();
        let Ok(came) = self.until::<Never>(
            |seen| {
                seen.taken = None;

                let Ok(now) = seen.clients();
                let new = now.iter().find(|client| {
                    let Ok(address) = address(client);

                    address.is_some_and(|found| !was.contains(&found))
                });

                let new = match new {
                    Some(new) => new,
                    None => return Ok(Seen::NotYet),
                };

                let Ok(found) = address(new);

                which = found.unwrap_or_default();
                where_ = new
                    .get("workspace")
                    .and_then(|workspace| workspace.get("name"))
                    .and_then(|name| name.as_str())
                    .unwrap_or_default()
                    .to_string();

                Ok(Seen::Yes)
            },
            seconds,
        );

        match came {
            Waited::Happened => {},
            Waited::RanOut => return Ok(None),
        }

        self.opened.push(which.clone());

        let Ok(there) = self.go_to(&where_);

        Ok(match there {
            Waited::Happened => Some(which),
            Waited::RanOut => None,
        })
    }

    pub fn settle(&mut self, seconds: f64) -> Result<(), Never> {
        match self.dry {
            true => {},
            false => {
                #[cfg_attr(
                    dylint_lib = "explicit021_no_sleeping",
                    allow(
                        explicit021_no_sleeping,
                        reason = "this is the gap `until` asks between two questions to the handheld, and a check that calls it directly is waiting on a number rather than on the thing -- which is what EXPLICIT022 denies, at the call, where the decision is"
                    )
                )]
                std::thread::sleep(Duration::from_secs_f64(seconds));
            }
        }

        Ok(())
    }

    pub fn workspace(&mut self) -> Result<String, Never> {
        let Ok(said) = self.hypr("activeworkspace -j");
        let Ok(found) = read(&said);

        let found = match found {
            Some(found) => found,
            None => return Ok(String::new()),
        };

        let Ok(named) = console_compositor::workspace(&found);

        Ok(named.unwrap_or_default().to_string())
    }

    fn clients(&mut self) -> Result<Vec<serde_json::Value>, Never> {
        let Ok(said) = self.hypr("clients -j");
        let Ok(found) = read(&said);

        let found = match found {
            Some(found) => found,
            None => return Ok(Vec::new()),
        };

        let Ok(clients) = console_compositor::clients(&found);

        Ok(clients.cloned().collect())
    }

    pub fn windows(&mut self) -> Result<Vec<String>, Never> {
        let Ok(clients) = self.clients();
        let mut named: Vec<String> = clients
            .iter()
            .map(|client| {
                client
                    .get("class")
                    .and_then(|class| class.as_str())
                    .unwrap_or_default()
                    .to_string()
            })
            .collect();

        named.sort();

        Ok(named)
    }

    pub fn titles(&mut self) -> Result<Vec<String>, Never> {
        let Ok(clients) = self.clients();

        Ok(clients
            .iter()
            .map(|client| {
                client
                    .get("title")
                    .and_then(|title| title.as_str())
                    .unwrap_or_default()
                    .to_string()
            })
            .collect())
    }

    pub fn types(&mut self, words: &str) -> Result<String, Never> {
        let Ok(whom) = self.whoever();
        let Ok(session) = session_env(&whom);
        let Ok(quoted) = quoted(words);

        self.user(&format!("{session} && wtype {quoted}"))
    }

    pub fn keyed(&mut self, held: &[&str], key: &str) -> Done {
        let mut chord = Vec::new();

        for one in held.iter().chain(std::iter::once(&key)) {
            let Ok(found) = vocabulary::key_capability(one);

            match found {
                Some(capability) => chord.push(capability),
                None => return failed(format!("there is no key called {one:?} to press")),
            }
        }

        let Ok(said) = self.chorded(&chord);

        match said.contains("Call failed") {
            true => failed(format!("{chord:?} was not sent: {}", said.trim())),
            false => Ok(()),
        }
    }

    pub fn windows_here(&mut self) -> Result<i64, Never> {
        let Ok(said) = self.hypr("activeworkspace -j");
        let Ok(found) = read(&said);

        let found = match found {
            Some(found) => found,
            None => return Ok(0),
        };

        let Ok(held) = console_compositor::windows(&found);

        Ok(held.unwrap_or(0))
    }

    pub fn keyboard(&mut self) -> Result<Seen, Never> {
        let Ok(said) = self.hypr("layers -j");

        Ok(match said.contains("virtual-keyboard") {
            true => Seen::Yes,
            false => Seen::NotYet,
        })
    }

    pub fn profile(&mut self) -> Result<String, Never> {
        match self.dry {
            true => return Ok(String::new()),
            false => {},
        }

        let asked = format!(
            "busctl --system get-property {} {} {} ProfileName",
            BUS.0, BUS.1, BUS.2
        );
        let Ok(said) = self.ssh(&asked);

        Ok(said.split('"').rev().nth(1).unwrap_or_default().to_string())
    }

    pub fn brightness(&mut self) -> Result<Level, Never> {
        let Ok(said) = self.ssh("cat /sys/class/backlight/*/brightness");

        brightness_in(&said)
    }

    pub fn brightness_to(&mut self, level: i64) -> Result<(), Never> {
        let Ok(_) = self.ssh(&format!(
            "printf '%s\\n' {level} | tee /sys/class/backlight/*/brightness >/dev/null"
        ));

        Ok(())
    }

    pub fn volume(&mut self) -> Result<Level, Never> {
        let Ok(said) = self.user("pactl get-sink-volume @DEFAULT_SINK@");

        volume_in(&said)
    }

    pub fn volume_to(&mut self, level: i64) -> Result<(), Never> {
        let Ok(_) = self.user(&format!("pactl set-sink-volume @DEFAULT_SINK@ {level}%"));

        Ok(())
    }

    pub fn services(&mut self) -> Result<Vec<String>, Never> {
        let asking: Vec<&str> =
            PIECES.into_iter().filter(|piece| *piece != BY_A_SWITCH).collect();
        let Ok(said) = self.user(&format!("systemctl --user is-active {}", asking.join(" ")));

        Ok(said.split_whitespace().map(str::to_string).collect())
    }

    pub fn restarts(&mut self) -> Result<Vec<String>, Never> {
        let Ok(said) = self.user(&format!(
            "systemctl --user show --value -p NRestarts {}",
            PIECES.join(" ")
        ));

        Ok(said.split_whitespace().map(str::to_string).collect())
    }

    pub fn icon_theme(&mut self) -> Result<String, Never> {
        self.user("sed -n 's/^gtk-icon-theme-name=//p' ~/.config/gtk-4.0/settings.ini | head -1")
    }

    pub fn icons_missing(&mut self, theme: &str, names: &[&str]) -> Result<Vec<String>, Never> {
        let asked = format!(
            "seen=''; todo={theme}; \
             while [ -n \"$todo\" ]; do \
               next=''; \
               for one in $todo; do \
                 case \" $seen \" in *\" $one \"*) continue ;; esac; \
                 seen=\"$seen $one\"; \
                 next=\"$next $(sed -n 's/^Inherits=//p' \
                   /usr/share/icons/$one/index.theme 2>/dev/null | head -1 | tr ',' ' ')\"; \
               done; \
               todo=$next; \
             done; \
             where=''; \
             for one in $seen hicolor; do where=\"$where /usr/share/icons/$one\"; done; \
             have=$(find $where \\( -name '*.svg' -o -name '*.png' \\) \
               -printf '%f\\n' 2>/dev/null | sed 's/\\.[^.]*$//' | sort -u); \
             for name in {names}; do \
               printf '%s\\n' \"$have\" | grep -qxF \"$name\" || echo \"$name\"; \
             done",
            theme = theme,
            names = names.join(" ")
        );

        let Ok(said) = self.user(&asked);

        Ok(said
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect())
    }

    pub fn files(&mut self, where_: &str) -> Result<Vec<String>, Never> {
        let Ok(quoted) = quoted(where_);
        let Ok(said) = self.user(&format!("ls -1 {quoted} 2>/dev/null"));
        let mut found: Vec<String> = said
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();

        found.sort();

        Ok(found)
    }

    pub fn journal(&mut self, unit: &str, lines: u32) -> Result<String, Never> {
        self.user(&format!("journalctl --user -u {unit} -n {lines} --no-pager"))
    }

    pub fn menus(&mut self) -> Result<Vec<String>, Never> {
        let Ok(said) = self.hypr("layers -j");
        let Ok(read) = read(&said);

        let found = match read {
            Some(found) => found,
            None => return Ok(Vec::new()),
        };

        let mut named = Vec::new();
        let Ok(surfaces) = console_compositor::surfaces(&found);

        for layer in surfaces {
            let Ok(said) = console_compositor::namespace(layer);
            let namespace = said.unwrap_or_default();

            match FURNITURE.contains(&namespace) {
                true => {},
                false => named.push(namespace.to_string()),
            }
        }

        named.sort();

        Ok(named)
    }

    pub fn until<Why>(
        &mut self,
        mut what: impl FnMut(&mut Self) -> Result<Seen, Why>,
        seconds: f64,
    ) -> Result<Waited, Why> {
        let Ok(rounds) = toward_zero_u32(seconds / 0.5);

        for _ in 0..rounds {
            let Ok(stop) = crate::stopping::asked();

            match stop {
                crate::stopping::Stop::Asked => return Ok(Waited::RanOut),
                crate::stopping::Stop::No => {},
            }

            #[cfg_attr(
                dylint_lib = "explicit022_no_settling",
                allow(
                    explicit022_no_settling,
                    reason = "this is the gap between two questions to the handheld, which is the one caller the sleep below it is for; the calls that still name a number say at their own sites why the elapsing was what was asked for"
                )
            )]
            let Ok(()) = self.settle(0.5);

            let seen = what(self)?;

            match seen {
                Seen::Yes => return Ok(Waited::Happened),
                Seen::NotYet => {},
            }
        }

        Ok(Waited::RanOut)
    }

    pub fn changed<T: PartialEq, Why>(
        &mut self,
        mut reading: impl FnMut(&mut Self) -> Result<T, Why>,
        from: &T,
        seconds: f64,
    ) -> Result<Waited, Why> {
        self.until(
            |seen| {
                let now = reading(seen)?;

                Ok(match now == *from {
                    true => Seen::NotYet,
                    false => Seen::Yes,
                })
            },
            seconds,
        )
    }

    pub fn drawn(&mut self, seconds: f64) -> Result<Waited, Never> {
        self.until(menus_up, seconds)
    }

    pub fn gone(&mut self, seconds: f64) -> Result<Waited, Never> {
        self.until(
            |seen| {
                let Ok(up) = menus_up(seen);

                up.flipped()
            },
            seconds,
        )
    }

    pub fn frame_cache(&mut self, picture: &str) -> Result<(Option<i64>, Option<i64>), Never> {
        let Ok(quoted) = quoted(picture);
        let Ok(said) = self.user(&format!(
            "find ~/.cache/awww -type f -exec stat -c %Y {{}} + 2>/dev/null \
             | sort -n | tail -1; echo --; stat -c %Y {quoted} 2>/dev/null"
        ));
        let mut halves = said.split("--");
        let last = |half: Option<&str>| {
            let mut found = None;

            for word in half.unwrap_or_default().split_whitespace() {
                match word.parse::<i64>() {
                    Ok(number) => found = Some(number),
                    Err(_not_a_number) => {},
                }
            }

            found
        };

        Ok((last(halves.next()), last(halves.next())))
    }

    pub fn wallpaper(&mut self) -> Result<String, Never> {
        let Ok(whom) = self.whoever();
        let Ok(session) = session_env(&whom);

        self.user(&format!("{session} && awww query"))
    }

    fn picture(&mut self) -> Result<&Picture, String> {
        match self.taken.is_none() {
            true => {
                let Ok(_) = self.exec_cmd("grim /tmp/console-check.png");
                let Ok(written) = self.until::<Never>(
                    |seen| {
                        let Ok(said) = seen.ssh(
                            "test -s /tmp/console-check.png && echo written || echo not-yet",
                        );

                        Ok(match said.trim() == "written" {
                            true => Seen::Yes,
                            false => Seen::NotYet,
                        })
                    },
                    A_PICTURE,
                );

                match written {
                    Waited::Happened => {},
                    Waited::RanOut => {
                        return Err("the device never wrote a picture to /tmp".to_string());
                    }
                }

                let here =
                    std::env::temp_dir().join(format!("console-shot-{}", std::process::id()));
                std::fs::create_dir_all(&here).map_err(|fault| fault.to_string())?;
                let shot = here.join("screen.png");
                let Ok(mut fetching) = Program::Scp.command();

                let _ = fetching
                    .args(["-q", &format!("{}:/tmp/console-check.png", self.host)])
                    .arg(&shot)
                    .status();
                let Ok(_) = self.ssh("rm -f /tmp/console-check.png");
                let picture = Picture::read(&shot)?;

                self.taken = Some(picture);
                self.kept = Some(here);
            }
            false => {},
        }

        self.taken.as_ref().ok_or_else(|| "the device took a picture and then had none".to_string())
    }

    pub fn background(&mut self) -> Result<String, String> {
        let picture = self.picture()?;
        let Ok(commonest) = picture.commonest();

        Ok(commonest)
    }

    pub fn colour(&mut self, across: f64, down: f64) -> Result<String, String> {
        let screen = self.showing()?;
        let picture = self.picture()?;

        where_(picture, across, down, &screen)
    }

    fn showing(&mut self) -> Result<console_screen::Screen, String> {
        match self.screen {
            Some(known) => Ok(known),
            None => {
                let Ok(shown) = self.shown();

                let screen = match shown {
                    Some(screen) => screen,
                    None => crate::screen()?,
                };

                self.screen = Some(screen);

                Ok(screen)
            },
        }
    }

    fn shown(&mut self) -> Result<Option<console_screen::Screen>, Never> {
        let Ok(said) = self.hypr("monitors -j");
        let Ok(read) = read(&said);

        let found = match read {
            Some(found) => found,
            None => return Ok(None),
        };

        let Ok(monitors) = console_compositor::monitors(&found);

        let first = match monitors.first() {
            Some(first) => first,
            None => return Ok(None),
        };

        let (wide, tall) = match first.size {
            Some(size) => size,
            None => return Ok(None),
        };

        let refresh = match first.refresh {
            Some(refresh) => refresh,
            None => return Ok(None),
        };

        let scale = match first.scale {
            Some(scale) => scale,
            None => return Ok(None),
        };

        let turn = match first.transform {
            Some(turn) => turn,
            None => return Ok(None),
        };

        let Ok(across) = fitted::<i64, u32>(wide);
        let Ok(down) = fitted::<i64, u32>(tall);
        let Ok(refresh) = whole_u32(refresh);
        let Ok(transform) = fitted::<i64, u32>(turn);

        Ok(Some(console_screen::Screen {
            mode: (across, down),
            refresh,
            scale,
            transform,
        }))
    }

    pub fn patch(&mut self, across: f64, down: f64) -> Result<String, String> {
        let picture = self.picture()?;
        let Ok(average) = picture.average(across, down, crate::picture::PATCH);

        Ok(average)
    }

    pub fn again(&mut self) -> Result<(), Never> {
        self.taken = None;

        Ok(())
    }

    pub fn fresh(&mut self) -> Result<(), Never> {
        self.taken = None;
        self.screen = None;

        match self.dry {
            true => return Ok(()),
            false => {},
        }

        match self.pushed {
            Pushed::Nothing => {},
            Pushed::Something => match self.let_go() {
                Ok(()) => {},
                Err(why) => {
                    eprintln!("console-test-stages: the pad would not let go: {why:?}");
                }
            },
        }

        for _ in 0..3 {
            let Ok(menus) = self.menus();

            match menus.is_empty() {
                true => break,
                false => {},
            }

            let Ok(()) = self.press("b");
            let Ok(_closed) = self.gone(A_MOMENT);
        }

        let Ok(profile) = self.profile();

        match profile == "Router" {
            true => {},
            false => {
                let Ok(()) = self.load_profile(console_input_gamepad::router::NAME);
                let Ok(loaded) = self.until::<Never>(
                    |seen| {
                        let Ok(now) = seen.profile();

                        Ok(match now == "Router" {
                            true => Seen::Yes,
                            false => Seen::NotYet,
                        })
                    },
                    A_MOMENT,
                );

                match loaded {
                    Waited::Happened => {},
                    Waited::RanOut => {
                        eprintln!("console-test-stages: the router profile would not load");
                    }
                }
            }
        }

        Ok(())
    }

    pub fn close(&mut self) -> Result<(), Never> {
        match self.kept.take() {
            Some(here) => {
                let _ = std::fs::remove_dir_all(here);
            }
            None => {},
        }

        Ok(())
    }
}

pub fn capability_under(here: Option<&Profile>, button: &str) -> Result<Option<String>, Never> {
    let itself = match vocabulary::button_name(button) {
        Ok(name) => Some(format!("Gamepad:Button:{name}")),
        Err(_unnamed) => None,
    };

    let here = match here {
        Some(here) => here,
        None => return Ok(itself),
    };

    let named = match here.for_button(button) {
        Ok(named) => named,
        Err(_fault) => return Ok(itself),
    };

    let sent = named.iter().flat_map(|mapping| &mapping.targets).find_map(|target| {
        let Ok(spoken) = spoken_as(target.kind, &target.name);

        spoken
    });

    Ok(match (sent, named.is_empty()) {
        (Some(sent), _) => Some(sent),
        (None, false) => None,
        (None, true) => itself,
    })
}

fn address(client: &serde_json::Value) -> Result<Option<String>, Never> {
    Ok(client.get("address").and_then(|address| address.as_str()).map(str::to_string))
}

fn read(said: &str) -> Result<Option<serde_json::Value>, Never> {
    Ok(match console_compositor::read(said) {
        Ok(parsed) => Some(parsed),
        Err(why) => {
            eprintln!("console-test-stages: the device answered with something the compositor did not: {why}");

            None
        }
    })
}

pub fn quoted(said: &str) -> Result<String, Never> {
    Ok(format!("'{}'", said.replace('\'', r"'\''")))
}

fn chord(capabilities: &[String]) -> Result<String, Never> {
    let mut words = vec![format!("as {}", capabilities.len())];

    for one in capabilities {
        let Ok(one) = quoted(one);

        words.push(one);
    }

    Ok(words.join(" "))
}

fn calling(chord: &str) -> Result<String, Never> {
    Ok(format!("busctl --system call {} {} {} SendButtonChord {chord} 2>&1", BUS.0, BUS.1, BUS.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dry() -> Device {
        Device::new("root@handheld", Dry::Pretend).expect("a stage")
    }

    fn root() -> std::path::PathBuf {
        let Ok(root) = crate::root();

        root
    }

    fn quoted(word: &str) -> String {
        let Ok(quoted) = super::quoted(word);

        quoted
    }

    fn capability_under(profile: Option<&Profile>, button: &str) -> Option<String> {
        let Ok(capability) = super::capability_under(profile, button);

        capability
    }

    #[test]
    fn the_pieces_asked_about_are_the_ones_the_manifest_enables() {
        let held = std::fs::read_to_string(root().join("desktop.conf"))
            .expect("the manifest");

        let Ok(services) = console_core_ini_files::lines(&held, "services");

        let mut named: Vec<String> = services
            .into_iter()
            .filter(|line| line.ends_with(".service"))
            .map(|line| line.trim_end_matches(".service").to_string())
            .collect();

        let mut wanted: Vec<String> = PIECES.iter().map(|piece| piece.to_string()).collect();
        named.sort();
        wanted.sort();

        assert_eq!(
            named, wanted,
            "the desktop the manifest enables is not the desktop the checks ask about"
        );
    }

    #[test]
    fn a_word_with_a_quote_in_it_is_still_one_word() {
        assert_eq!(quoted("plain"), "'plain'");
        assert_eq!(quoted("it's"), r"'it'\''s'");
    }

    #[test]
    fn a_chord_says_how_many_it_is_before_it_says_what_they_are() {
        let Ok(said) = super::chord(&["Gamepad:Button:South".to_string()]);
        assert_eq!(said, "as 1 'Gamepad:Button:South'");
    }

    #[test]
    fn a_key_under_a_modifier_is_the_modifier_and_then_the_key() {
        let Ok(one) = vocabulary::key_capability("super");
        let Ok(other) = vocabulary::key_capability("i");
        let Ok(said) = super::chord(&[one.expect("a modifier"), other.expect("a letter")]);

        assert_eq!(said, "as 2 'Keyboard:KeyLeftMeta' 'Keyboard:KeyI'");
    }

    #[test]
    fn a_key_nothing_names_is_answered_with_nothing_rather_than_a_guess() {
        assert_eq!(vocabulary::key_capability("logo"), Ok(None));
        assert_eq!(vocabulary::key_capability("slash"), Ok(None));
    }

    #[test]
    fn a_button_is_sent_as_what_the_loaded_profile_makes_of_it() {
        let profiles = every_profile(&root()).expect("the profiles");
        assert_eq!(
            capability_under(profiles.get("router"), "right-paddle-top"),
            Some("Keyboard:KeyF15".to_string())
        );
    }

    #[test]
    fn a_button_that_means_nothing_in_a_chooser_is_still_sent() {
        let profiles = every_profile(&root()).expect("the profiles");
        assert_eq!(
            capability_under(profiles.get("router"), "view"),
            Some("Gamepad:Button:Select".to_string())
        );
    }

    #[test]
    fn a_button_a_profile_says_nothing_about_is_sent_as_itself() {
        let profiles = every_profile(&root()).expect("the profiles");
        assert_eq!(
            capability_under(profiles.get("game"), "a"),
            Some("Gamepad:Button:South".to_string())
        );
    }

    #[test]
    fn a_level_the_machine_would_not_say_is_not_a_level_of_nought() {
        assert_eq!(brightness_in("24000\n"), Ok(Level::At(24000)));
        assert_eq!(brightness_in(""), Ok(Level::Unsaid));
        assert_eq!(brightness_in("no such file"), Ok(Level::Unsaid));

        let said = "Volume: front-left: 26214 /  40% / -23.86 dB,   front-right: 26214 /  40%";

        assert_eq!(volume_in(said), Ok(Level::At(40)));
        assert_eq!(volume_in(""), Ok(Level::Unsaid));
        assert_eq!(volume_in("Volume: unknown"), Ok(Level::Unsaid));
    }

    #[test]
    fn nothing_is_sent_on_a_dry_run() {
        let mut device = dry();
        device.press("a");
        assert!(!device.done.is_empty(), "the command is still read");
    }
}

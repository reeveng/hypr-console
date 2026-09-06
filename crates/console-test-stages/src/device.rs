//! The machine itself, pressed through InputPlumber and looked at over ssh.
//!
//! Nothing here makes an input device. InputPlumber is asked to emit the event
//! it would have read from the hardware, through the profile that is loaded,
//! which is its own supported way of doing this and is what a chord on the
//! device already uses. So there is no second pad for the daemons to find and
//! nothing to clean up if a check stops halfway.


use console_external_programs::Program;
use console_number_conversion::{fitted, toward_zero_u32, whole_u32};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use console_controller::means::Press;
use console_never::Never;
use console_gamepad::profile::{Kind, Profile};
use console_gamepad::router::every_profile;
use console_gamepad::vocabulary;

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
    "console-controller",
    "console-keyboard",
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

pub const SETTLED: f64 = 0.6;

pub const PATIENCE: f64 = 4.0;

pub const OPENING: f64 = 12.0;

pub const FURNITURE: [&str; console_controller::mode::FURNITURE.len() + 1] = {
    let mut every = [""; console_controller::mode::FURNITURE.len() + 1];
    let mut at = 0;

    while at < console_controller::mode::FURNITURE.len() {
        every[at] = console_controller::mode::FURNITURE[at];
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
    pushed: Pushed,
    screen: Option<console_screen::Screen>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    Yes,
    NotYet,
}

impl Seen {
    pub fn flipped(self) -> Result<Self, Never> {
        Ok(match self {
            Seen::Yes => Seen::NotYet,
            Seen::NotYet => Seen::Yes,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waited {
    Happened,
    RanOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dry {
    Pretend,
    Really,
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
            whom: None,
            pushed: Pushed::Nothing,
            screen: None,
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

    pub fn ssh(&mut self, command: &str) -> Result<String, Never> {
        self.done.push(command.to_string());

        match self.dry {
            true => return Ok(String::new()),
            false => {},
        }

        let Ok(mut asking) = Program::Ssh.command();

        let done = asking
            .args(["-o", "BatchMode=yes", &self.host, command])
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

    pub fn press(&mut self, button: &str) -> Result<(), Never> {
        self.taken = None;

        let Ok(found) = self.capability(button);

        let Some(capability) = found else {
            return Ok(());
        };

        let Ok(quoted) = quoted(&capability);
        let asked = format!(
            "busctl --system call {} {} {} SendButtonChord as 1 {quoted}",
            BUS.0, BUS.1, BUS.2,
        );
        let Ok(_) = self.ssh(&asked);

        Ok(())
    }

    fn send(&mut self, capability: &str, down: Press) -> Result<(), Never> {
        self.taken = None;

        let Ok(quoted) = quoted(capability);
        let asked = format!(
            "busctl --system call {} {} {} SendEvent sv {quoted} b {}",
            BUS.0,
            BUS.1,
            BUS.2,
            match down {
                Press::Down => "true",
                Press::Up => "false",
            }
        );
        let Ok(_) = self.ssh(&asked);

        Ok(())
    }

    pub fn hold(&mut self, button: &str) -> Result<(), Never> {
        let Ok(found) = self.capability(button);

        match found {
            Some(capability) => self.send(&capability, Press::Down),
            None => Ok(()),
        }
    }

    pub fn release(&mut self, button: Option<&str>) -> Result<(), Never> {
        let Some(button) = button else { return Ok(()) };

        let Ok(found) = self.capability(button);

        match found {
            Some(capability) => self.send(&capability, Press::Up),
            None => Ok(()),
        }
    }

    pub fn trigger(&mut self, which: &str, amount: f64) -> Done {
        let Ok(found) = named(&vocabulary::TRIGGERS, which);

        let Some(named) = found else {
            let Ok(said) = said(&vocabulary::TRIGGERS);

            return failed(format!("there is no trigger called {which:?}; {said}"));
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

        let Some(named) = found else {
            let Ok(said) = said(&vocabulary::AXES);

            return failed(format!("there is no stick called {which:?}; {said}"));
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

    pub fn layer(&mut self, namespace: &str) -> Result<Option<(u32, u32, u32, u32)>, Never> {
        let Ok(said) = self.hypr("layers -j");
        let Ok(read) = read(&said);

        let Some(found) = read else { return Ok(None) };

        for screen in found.as_object().into_iter().flat_map(|screens| screens.values()) {
            for level in screen
                .get("levels")
                .and_then(|levels| levels.as_object())
                .into_iter()
                .flat_map(|levels| levels.values())
            {
                for layer in level.as_array().into_iter().flatten() {
                    match layer.get("namespace").and_then(|said| said.as_str()) == Some(namespace)
                    {
                        true => {},
                        false => continue,
                    }

                    let at = |name: &str| {
                        let said = layer.get(name).and_then(|said| said.as_i64())?;

                        let Ok(said) = fitted::<i64, u32>(said);

                        Some(said)
                    };

                    let Some(x) = at("x") else { return Ok(None) };

                    let Some(y) = at("y") else { return Ok(None) };

                    let Some(wide) = at("w") else { return Ok(None) };

                    let Some(tall) = at("h") else { return Ok(None) };

                    return Ok(Some((x, y, wide, tall)));
                }
            }
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

    pub fn open(&mut self, command: &str, seconds: f64) -> Result<Waited, Never> {
        match self.dry {
            true => {
                let Ok(_) = self.exec_cmd(command);

                return Ok(Waited::Happened);
            }
            false => {},
        }

        let Ok(clients) = self.clients();
        let was: Vec<String> = clients
            .iter()
            .filter_map(|client| {
                let Ok(address) = address(client);

                address
            })
            .collect();
        let Ok(_) = self.exec_cmd(command);
        let until = Instant::now() + Duration::from_secs_f64(seconds);

        while Instant::now() < until {
            self.taken = None;

            let Ok(now) = self.clients();
            let new = now.iter().find(|client| {
                let Ok(address) = address(client);

                address.is_some_and(|found| !was.contains(&found))
            });

            match new {
                Some(new) => {
                    let workspace = new
                        .get("workspace")
                        .and_then(|workspace| workspace.get("name"))
                        .and_then(|name| name.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let Ok(quoted) =
                        quoted(&format!("hl.dsp.focus({{workspace = \"{workspace}\"}})"));
                    let Ok(_) = self.hypr(&format!("dispatch {quoted}"));

                    std::thread::sleep(Duration::from_secs_f64(0.6));

                    return Ok(Waited::Happened);
                }
                None => {},
            }

            std::thread::sleep(Duration::from_secs_f64(0.4));
        }

        Ok(Waited::RanOut)
    }

    pub fn settle(&mut self, seconds: f64) -> Result<(), Never> {
        match self.dry {
            true => {},
            false => std::thread::sleep(Duration::from_secs_f64(seconds)),
        }

        Ok(())
    }

    pub fn workspace(&mut self) -> Result<String, Never> {
        let Ok(said) = self.hypr("activeworkspace -j");
        let Ok(found) = read(&said);

        Ok(found
            .map(|found| {
                found
                    .get("name")
                    .and_then(|name| name.as_str())
                    .unwrap_or_default()
                    .to_string()
            })
            .unwrap_or_default())
    }

    fn clients(&mut self) -> Result<Vec<serde_json::Value>, Never> {
        let Ok(said) = self.hypr("clients -j");
        let Ok(found) = read(&said);

        Ok(found.and_then(|found| found.as_array().cloned()).unwrap_or_default())
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

    pub fn windows_here(&mut self) -> Result<i64, Never> {
        let Ok(said) = self.hypr("activeworkspace -j");
        let Ok(found) = read(&said);

        Ok(found
            .and_then(|found| found.get("windows").and_then(|windows| windows.as_i64()))
            .unwrap_or(0))
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

    pub fn brightness(&mut self) -> Result<i64, Never> {
        let Ok(said) = self.ssh("cat /sys/class/backlight/*/brightness");

        let Some(line) = said.lines().next() else { return Ok(0) };

        Ok(match line.trim().parse() {
            Ok(brightness) => brightness,
            Err(fault) => {
                eprintln!("console-test-stages: the backlight said {line:?}: {fault}");

                0
            }
        })
    }

    pub fn volume(&mut self) -> Result<i64, Never> {
        let Ok(said) = self.user("pactl get-sink-volume @DEFAULT_SINK@");
        let word = said.lines().next().and_then(|line| line.split_whitespace().nth(4));

        let Some(word) = word else { return Ok(0) };

        Ok(match word.trim_end_matches('%').parse() {
            Ok(volume) => volume,
            Err(fault) => {
                eprintln!("console-test-stages: pactl said {word:?} where the level goes: {fault}");

                0
            }
        })
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

        let Some(found) = read else {
            return Ok(Vec::new());
        };

        let mut named = Vec::new();

        for screen in found
            .as_object()
            .into_iter()
            .flat_map(|screens| screens.values())
        {
            for level in screen
                .get("levels")
                .and_then(|levels| levels.as_object())
                .into_iter()
                .flat_map(|levels| levels.values())
            {
                for layer in level.as_array().into_iter().flatten() {
                    let namespace = layer
                        .get("namespace")
                        .and_then(|said| said.as_str())
                        .unwrap_or_default();

                    match FURNITURE.contains(&namespace) {
                        true => {},
                        false => named.push(namespace.to_string()),
                    }
                }
            }
        }

        named.sort();

        Ok(named)
    }

    pub fn until(
        &mut self,
        mut what: impl FnMut(&mut Self) -> Result<Seen, Never>,
        seconds: f64,
    ) -> Result<Waited, Never> {
        let Ok(rounds) = toward_zero_u32(seconds / 0.5);

        for _ in 0..rounds {
            let Ok(()) = self.settle(0.5);
            let Ok(seen) = what(self);

            match seen {
                Seen::Yes => return Ok(Waited::Happened),
                Seen::NotYet => {},
            }
        }

        Ok(Waited::RanOut)
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

                std::thread::sleep(Duration::from_secs_f64(1.5));
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

        let Some(found) = read else { return Ok(None) };

        let Some(every) = found.as_array() else { return Ok(None) };

        let Some(first) = every.first() else { return Ok(None) };

        let number = |name: &str| first.get(name).and_then(serde_json::Value::as_f64);

        let Some(wide) = number("width") else { return Ok(None) };

        let Some(tall) = number("height") else { return Ok(None) };

        let Some(refresh) = number("refreshRate") else { return Ok(None) };

        let Some(scale) = number("scale") else { return Ok(None) };

        let Some(turn) = number("transform") else { return Ok(None) };

        let Ok(across) = whole_u32(wide);
        let Ok(down) = whole_u32(tall);
        let Ok(refresh) = whole_u32(refresh);
        let Ok(transform) = whole_u32(turn);

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

            std::thread::sleep(Duration::from_secs_f64(0.8));
        }

        let Ok(profile) = self.profile();

        match profile == "Router" {
            true => {},
            false => {
                let Ok(()) = self.load_profile(console_gamepad::router::NAME);

                std::thread::sleep(Duration::from_secs_f64(0.5));
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

    let Some(here) = here else { return Ok(itself) };

    let Ok(named) = here.for_button(button) else { return Ok(itself) };

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
    Ok(match serde_json::from_str(said) {
        Ok(parsed) => Some(parsed),
        Err(fault) => {
            eprintln!("console-test-stages: the device answered with something that is not JSON: {fault}");

            None
        }
    })
}

pub fn quoted(said: &str) -> Result<String, Never> {
    Ok(format!("'{}'", said.replace('\'', r"'\''")))
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

        let mut named: Vec<String> = Vec::new();
        let mut inside = false;

        for line in held.lines() {
            let line = line.trim();

            match line.starts_with('[') {
                true => inside = line == "[services]",
                false => {}
            }

            match inside && line.ends_with(".service") {
                true => named.push(line.trim_end_matches(".service").to_string()),
                false => {}
            }
        }

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
    fn nothing_is_sent_on_a_dry_run() {
        let mut device = dry();
        device.press("a");
        assert!(!device.done.is_empty(), "the command is still read");
    }
}

//! The machine itself, pressed through InputPlumber and looked at over ssh.
//!
//! Nothing here makes an input device. InputPlumber is asked to emit the event
//! it would have read from the hardware, through the profile that is loaded,
//! which is its own supported way of doing this and is what a chord on the
//! device already uses. So there is no second pad for the daemons to find and
//! nothing to clean up if a check stops halfway.


use console_number::{fitted, toward_zero_u32};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use console_controller::means::Press;
use console_pad::profile::{Kind, Profile};
use console_pad::router::every_profile;
use console_pad::vocabulary;

use crate::checking::{Done, cannot};
use crate::picture::{Picture, where_};

/// Where the device is, which only the person with one can say.
///
/// Read from `CONSOLE_HOST` and nowhere else. The address of somebody's machine
/// is not source: it is a name on their own network, it is different for
/// everybody who builds this, and there is nothing on the device to ask
/// because reaching the device is the thing it answers. `tools/console-deploy`
/// and `tools/console-pull` have always read this variable; this is the same
/// setting, said once.
pub fn host() -> Result<String, String> {
    match std::env::var("CONSOLE_HOST") {
        Ok(said) if !said.trim().is_empty() => Ok(said),
        _ => Err("CONSOLE_HOST is not set, so there is no device to talk to. \
                  Set it to the device, as in CONSOLE_HOST=root@handheld."
            .to_string()),
    }
}

/// The mark the manifest writes for whoever a desktop belongs to.
///
/// Printed by a dry run, where nothing has been asked of the device and so
/// nobody has been named. A command shown with this still in it is a command
/// that would have had a name in it had it been run.
const MARK: &str = "@user@";

/// The bus InputPlumber answers on.
const BUS: (&str, &str, &str) = (
    "org.shadowblip.InputPlumber",
    "/org/shadowblip/InputPlumber/CompositeDevice0",
    "org.shadowblip.Input.CompositeDevice",
);

/// The session's own environment, worked out on the device.
///
/// Not from the compositor's own environ: that is what it was handed when it
/// was executed, and it sets its signature and the socket name after that.
/// Not from whatever else the session started either, which is where this used
/// to look. A process outlives the session that started it, and the first one
/// listed is the oldest, so a machine that had been logged in twice answered
/// with the signature of the session before last and every check failed at
/// once for a desktop that was perfectly well.
///
/// The compositor writes a directory named after its signature, so the newest
/// of those is the session that is up.
fn session_env(whom: &str) -> String {
    format!(
        "export XDG_RUNTIME_DIR=/run/user/$(id -u {whom}); \
         export HYPRLAND_INSTANCE_SIGNATURE=$(ls -1t \"$XDG_RUNTIME_DIR/hypr\" 2>/dev/null | head -1); \
         export WAYLAND_DISPLAY=$(ls -1t \"$XDG_RUNTIME_DIR\" 2>/dev/null \
         | grep -E '^wayland-[0-9]+$' | head -1); \
         [ -n \"$HYPRLAND_INSTANCE_SIGNATURE\" ] || \
         {{ echo 'nothing on the device is in a Hyprland session' >&2; exit 1; }}"
    )
}

/// How long a press is given to have arrived, when nobody says.
pub const SETTLED: f64 = 0.6;

/// How long something is waited for before it is not going to happen.
pub const PATIENCE: f64 = 4.0;

/// How long a window is given to open.
pub const OPENING: f64 = 12.0;

/// The layers that are always there: the bar, the strip under it, the
/// wallpaper, the keyboard. Anything else drawn over the desktop is something
/// a person opened.
///
/// `updating` is the strip, which is a second waybar under the first and
/// arrives under its own name. Left out, every check that waits for the menus
/// to be gone waits for a strip that is never going away.
///
/// `notifications` is not furniture and is not a chooser either: a card is
/// raised by the desktop rather than opened by anybody, and it goes on its own
/// six seconds later. Counted, it made three checks fail at once on a device
/// where a service had just restarted and said so -- B had closed the menu,
/// and what was left on the screen was the desktop telling somebody about
/// something else.
/// The daemon's own list plus the wallpaper this device does not run.
///
/// Kept as the daemon's list and not as a copy of it. A second list is a list
/// that drifts, and this one had: the home screen was added to the desktop and
/// to `console_controller::mode::FURNITURE` and not to here, so every check
/// that waited for the menus to be gone was waiting for a surface that is up
/// for every minute the machine is running -- and said "something was already
/// up" about the desktop itself.
pub const FURNITURE: [&str; console_controller::mode::FURNITURE.len() + 1] = {
    let mut every = [""; console_controller::mode::FURNITURE.len() + 1];
    let mut at = 0;

    while at < console_controller::mode::FURNITURE.len() {
        every[at] = console_controller::mode::FURNITURE[at];
        at += 1;
    }

    // The wallpaper under another name. This desktop draws its own, and
    // hyprpaper is what a machine that has not been applied to yet is running.
    every[at] = "hyprpaper";
    every
};

/// What a target event is called when it is sent rather than received.
fn spoken_as(kind: Kind, name: &str) -> Option<String> {
    match kind {
        Kind::Key => Some(format!("Keyboard:{name}")),
        Kind::MouseButton => Some(format!("Mouse:Button:{name}")),
        Kind::GamepadButton => Some(format!("Gamepad:Button:{name}")),
        _ => None,
    }
}

pub struct Device {
    pub host: String,
    /// Whoever the device belongs to, once it has been asked.
    whom: Option<String>,
    /// Read the commands rather than send them.
    pub dry: bool,
    pub done: Vec<String>,
    profiles: BTreeMap<String, Profile>,
    taken: Option<Picture>,
    kept: Option<PathBuf>,
}

/// Whether any chooser is on the screen, as the wait wants it.
///
/// A free function rather than a method because both `drawn` and `gone` hand
/// it to `until` as the closure, and one of them reads it the other way round.
fn menus_up(seen: &mut Device) -> Seen {
    match seen.menus().is_empty() {
        true => Seen::NotYet,
        false => Seen::Yes,
    }
}

/// Whether the thing being waited for is true yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    /// It is there.
    Yes,
    /// Not yet, which is not the same as never.
    NotYet,
}

impl Seen {
    /// The opposite, for a wait that is for something to stop rather than to
    /// start. `gone` is `drawn` read this way round.
    pub fn flipped(self) -> Self {
        match self {
            Seen::Yes => Seen::NotYet,
            Seen::NotYet => Seen::Yes,
        }
    }
}

/// How a wait ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waited {
    /// The thing happened, and the wait stopped early because of it.
    Happened,
    /// The time ran out first, and the thing may still be coming.
    RanOut,
}

/// Whether the device is really being touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dry {
    /// Say what would be done and do none of it.
    Pretend,
    /// Do it.
    Really,
}

impl Device {
    pub fn new(host: &str, dry: Dry) -> Result<Self, String> {
        Ok(Device {
            host: host.to_string(),
            dry: dry == Dry::Pretend,
            done: Vec::new(),
            profiles: every_profile(&crate::root())?,
            taken: None,
            kept: None,
            whom: None,
        })
    }

    /// Whoever the device belongs to, asked of the device.
    ///
    /// The same question `console apply` answers on the machine itself, asked
    /// from here instead: one home in `/home` is that person, and a device with
    /// several is asked for the first account it made. Asked once and kept,
    /// because it costs a round trip and cannot change under a running check.
    ///
    /// `CONSOLE_USER` says it outright for a device this cannot work out.
    pub fn whoever(&mut self) -> String {
        if let Some(known) = &self.whom {
            return known.clone();
        }

        let said = match std::env::var("CONSOLE_USER") {
            Ok(said) if !said.trim().is_empty() => said.trim().to_string(),
            _ => self.ssh(
                "set -- $(ls -1 /home 2>/dev/null); \
                 if [ $# -eq 1 ]; then echo \"$1\"; else id -nu 1000; fi",
            ),
        };
        let said = match said.trim().is_empty() {
            true => MARK.to_string(),
            false => said.trim().to_string(),
        };
        self.whom = Some(said.clone());
        said
    }

    /// That person's home.
    pub fn home(&mut self) -> String {
        format!("/home/{}", self.whoever())
    }

    pub fn ssh(&mut self, command: &str) -> String {
        self.done.push(command.to_string());

        if self.dry {
            return String::new();
        }

        let done = Command::new("ssh")
            .args(["-o", "BatchMode=yes", &self.host, command])
            .output();

        match done {
            Ok(done) => String::from_utf8_lossy(&done.stdout).trim().to_string(),
            // Nothing, which every caller of this reads as the device having no
            // answer. An ssh that will not run at all is not the device saying
            // nothing, and the two used to leave by the same line.
            Err(fault) => {
                eprintln!("console-stage: ssh {}: {fault}", self.host);

                String::new()
            }
        }
    }

    /// As the person whose session the desktop is.
    pub fn user(&mut self, command: &str) -> String {
        let whom = self.whoever();
        let asked = format!(
            "machinectl shell --uid={whom} .host /bin/sh -c {}",
            quoted(command)
        );
        self.ssh(&asked)
    }

    /// hyprctl needs the session's own environment to find its socket.
    pub fn hypr(&mut self, command: &str) -> String {
        let whom = self.whoever();
        let asked = format!("{} && hyprctl {command}", session_env(&whom));
        self.user(&asked)
    }

    // ------------------------------------------------------------------ doing

    /// The profile the device has loaded, read as what it maps.
    fn loaded(&mut self) -> Option<Profile> {
        let loaded = self.profile().to_lowercase();
        self.profiles
            .values()
            .find(|known| known.name.to_lowercase() == loaded)
            .cloned()
    }

    /// What this button becomes under the profile the device has loaded.
    ///
    /// InputPlumber sends what it is handed. A capability given to SendEvent
    /// arrives at the targets as itself, with the loaded profile's mapping not
    /// applied on the way, which is the opposite of what a physical press does.
    /// So a paddle sent as a paddle arrives as a paddle, where a thumb on the
    /// same paddle arrives as a function key, and the daemon that acts on
    /// function keys sees nothing at all. Four checks failed on that and read as
    /// four faults on the device.
    ///
    /// A button the profile says nothing about is sent as itself, because that
    /// is what the device does with it too. A button the profile names and sends
    /// nowhere is not sent at all: under a chooser that is a deliberate silence,
    /// and sending the button itself would put it on the pad, which is the
    /// accident the naming exists to prevent.
    fn capability(&mut self, button: &str) -> Option<String> {
        capability_under(self.loaded().as_ref(), button)
    }

    pub fn press(&mut self, button: &str) {
        self.taken = None;

        // Named here, and sent nowhere, on purpose.
        let Some(capability) = self.capability(button) else {
            return;
        };

        let asked = format!(
            "busctl --system call {} {} {} SendButtonChord as 1 {}",
            BUS.0,
            BUS.1,
            BUS.2,
            quoted(&capability)
        );
        self.ssh(&asked);
    }

    fn send(&mut self, capability: &str, down: Press) {
        self.taken = None;
        let asked = format!(
            "busctl --system call {} {} {} SendEvent sv {} b {}",
            BUS.0,
            BUS.1,
            BUS.2,
            quoted(capability),
            match down {
                Press::Down => "true",
                Press::Up => "false",
            }
        );
        self.ssh(&asked);
    }

    pub fn hold(&mut self, button: &str) {
        if let Some(capability) = self.capability(button) {
            self.send(&capability, Press::Down);
        }
    }

    pub fn release(&mut self, button: Option<&str>) {
        let Some(button) = button else { return };

        if let Some(capability) = self.capability(button) {
            self.send(&capability, Press::Up);
        }
    }

    /// Not from here. InputPlumber sends events, and a held axis is state.
    ///
    /// An injected value is overwritten by the pad's own reading of a trigger
    /// nobody is pulling, a few hundred times a second, so it lasts about as
    /// long as it takes the hardware to report again. Measured on the device:
    /// the step by hand, 64000 to 58000. The trigger injected forty times around
    /// the press over one connection, 58000 to 58000. Sent instead as
    /// Gamepad:Button:LeftTrigger, which the composite device does publish,
    /// 64000 to 64000. SendEvent and SendButtonChord are the whole surface and
    /// neither of them holds anything.
    ///
    /// So the screen, console-brightness and the daemon are all in the clear, and
    /// a check whose subject is a held trigger wants a thumb, the way the
    /// touchpad does. Saying so makes those checks skip and give the reason. Not
    /// saying it made two of them pass because nothing had arrived, which is
    /// worse than either of them failing.
    pub fn trigger(&mut self, _which: &str, _amount: f64) -> Done {
        cannot("an axis cannot be held from here; L2 wants a thumb")
    }

    pub fn stick(&mut self, _which: &str, _across: f64, _down: f64) -> Done {
        cannot("a stick is two axes in one event; not yet")
    }

    pub fn tap(&mut self, _across: i32, _down: i32) -> Done {
        cannot("the touchpad is not InputPlumber's to send")
    }

    /// A finger on the glass, at a place on the picture.
    ///
    /// The one thing a check could not do until there was a program to do it
    /// with, and the reason is in `console-poke`: InputPlumber publishes the
    /// pad and nothing publishes the screen, so every check that wanted to
    /// know whether a surface answers a touch had to ask something else
    /// instead -- and everything else says yes about a surface nothing can
    /// press.
    ///
    /// Said in the size the desktop is laid out in, which is what `hyprctl
    /// layers` answers in and what anything drawing a surface thinks in.
    /// Turning that into a place on the digitizer is `console_screen`, once,
    /// for everybody.
    pub fn touch(&mut self, at: (u32, u32)) -> Done {
        // Root, because /dev/uinput is. The pad goes through InputPlumber's
        // bus and needs no such thing; a touchscreen is made rather than
        // spoken to.
        let said = self.ssh(&format!("console-poke {} {}", at.0, at.1));

        match said.contains("console-poke:") {
            true => cannot(&format!("nothing could be pressed at {at:?}: {}", said.trim())),
            false => Ok(()),
        }
    }

    /// Whether the home screen is holding a highlight.
    ///
    /// Its own answer about itself, which is the one thing about the home
    /// screen the compositor cannot be asked: the surface is drawn asleep and
    /// awake alike, and what changes is whose button A is.
    pub fn home_awake(&mut self) -> Seen {
        match self.user("test -e \"$XDG_RUNTIME_DIR/console/home-awake\" && echo awake").contains("awake") {
            true => Seen::Yes,
            false => Seen::NotYet,
        }
    }

    /// Where a layer surface is, in the size the desktop is laid out in.
    ///
    /// So that a check can press what is drawn rather than a number somebody
    /// wrote down: where the bar's icons are is the bar's business, and a
    /// check that hard-codes it is a check that fails the day the bar changes
    /// height and says the wrong thing about why.
    pub fn layer(&mut self, namespace: &str) -> Option<(u32, u32, u32, u32)> {
        let said = self.hypr("layers -j");
        let found = read(&said)?;

        for screen in found.as_object().into_iter().flat_map(|screens| screens.values()) {
            for level in screen["levels"].as_object().into_iter().flat_map(|levels| levels.values())
            {
                for layer in level.as_array().into_iter().flatten() {
                    if layer["namespace"].as_str() != Some(namespace) {
                        continue;
                    }

                    // A place on the screen, held inside what a place is: the
                    // compositor answers these as signed and a layer hung off
                    // the top or the left is a number no finger can be put on.
                    let at = |name: &str| layer[name].as_i64().map(fitted::<i64, u32>);

                    return Some((at("x")?, at("y")?, at("w")?, at("h")?));
                }
            }
        }

        None
    }

    pub fn load_profile(&mut self, name: &str) {
        let asked = format!("controller-profile {}", quoted(name));
        self.user(&asked);
    }

    /// Ask the compositor to start something.
    ///
    /// Quoted as one argument, because the shell on the far end would otherwise
    /// eat the quotes around the Lua and hand hyprctl a bare word where a string
    /// was meant. It answers ok either way.
    pub fn exec_cmd(&mut self, command: &str) -> String {
        let asked = format!(
            "dispatch {}",
            quoted(&format!("hl.dsp.exec_cmd(\"{command}\")"))
        );
        self.hypr(&asked)
    }

    /// Start something on the device, and wait until it is really there.
    ///
    /// A check that needs a window has to be able to make one. The device is
    /// usually sitting on an empty desktop, and a check that refuses unless
    /// somebody happened to leave something open is a check that never runs.
    pub fn open(&mut self, command: &str, seconds: f64) -> Waited {
        if self.dry {
            self.exec_cmd(command);
            return Waited::Happened;
        }

        let was: Vec<String> = self.clients().iter().filter_map(address).collect();
        self.exec_cmd(command);
        let until = Instant::now() + Duration::from_secs_f64(seconds);

        while Instant::now() < until {
            self.taken = None;
            let now = self.clients();
            let new = now
                .iter()
                .find(|client| address(client).is_some_and(|found| !was.contains(&found)));

            if let Some(new) = new {
                // And look at it. Every window here opens on a workspace of its
                // own, so one that has just opened is not the one being looked
                // at, and a button aimed at the active window would find none.
                let workspace = new["workspace"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let asked = format!(
                    "dispatch {}",
                    quoted(&format!("hl.dsp.focus({{workspace = \"{workspace}\"}})"))
                );
                self.hypr(&asked);
                // Drawn, not only mapped.
                std::thread::sleep(Duration::from_secs_f64(0.6));
                return Waited::Happened;
            }

            std::thread::sleep(Duration::from_secs_f64(0.4));
        }

        Waited::RanOut
    }

    pub fn settle(&mut self, seconds: f64) {
        if !self.dry {
            std::thread::sleep(Duration::from_secs_f64(seconds));
        }
    }

    // ----------------------------------------------------------------- seeing

    pub fn workspace(&mut self) -> String {
        let said = self.hypr("activeworkspace -j");
        read(&said)
            .map(|found| found["name"].as_str().unwrap_or_default().to_string())
            .unwrap_or_default()
    }

    fn clients(&mut self) -> Vec<serde_json::Value> {
        let said = self.hypr("clients -j");
        read(&said)
            .and_then(|found| found.as_array().cloned())
            .unwrap_or_default()
    }

    pub fn windows(&mut self) -> Vec<String> {
        let mut named: Vec<String> = self
            .clients()
            .iter()
            .map(|client| client["class"].as_str().unwrap_or_default().to_string())
            .collect();
        named.sort();
        named
    }

    /// What the windows call themselves.
    ///
    /// The class says which program a window belongs to and the title says what
    /// it has been asked to show, which is the one thing a check can read from
    /// the far side of a page: a page that sets `document.title` has said
    /// something out loud, and `hyprctl` repeats it.
    pub fn titles(&mut self) -> Vec<String> {
        self.clients()
            .iter()
            .map(|client| client["title"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    /// Type the way the on-screen keyboard types.
    ///
    /// The keyboard does not type into a window. It makes a virtual keyboard at the
    /// compositor and the compositor hands the keys to whatever has the focus,
    /// so nothing on the far side can tell them from the real keyboard's. wtype
    /// speaks that same protocol, `zwp_virtual_keyboard_v1`, which makes this
    /// the keyboard's own path with a program where a thumb would be.
    pub fn types(&mut self, words: &str) -> String {
        let whom = self.whoever();
        let asked = format!("{} && wtype {}", session_env(&whom), quoted(words));
        self.user(&asked)
    }

    /// How many are on the workspace being looked at, which is the only number
    /// that says whether anything is covering the wallpaper.
    pub fn windows_here(&mut self) -> i64 {
        let said = self.hypr("activeworkspace -j");
        read(&said)
            .and_then(|found| found["windows"].as_i64())
            .unwrap_or(0)
    }

    /// Whether the on-screen keyboard is on screen, not merely running.
    pub fn keyboard(&mut self) -> Seen {
        match self.hypr("layers -j").contains("virtual-keyboard") {
            true => Seen::Yes,
            false => Seen::NotYet,
        }
    }

    pub fn profile(&mut self) -> String {
        if self.dry {
            return String::new();
        }

        let asked = format!(
            "busctl --system get-property {} {} {} ProfileName",
            BUS.0, BUS.1, BUS.2
        );
        let said = self.ssh(&asked);
        said.split('"').rev().nth(1).unwrap_or_default().to_string()
    }

    pub fn brightness(&mut self) -> i64 {
        let said = self.ssh("cat /sys/class/backlight/*/brightness");

        let Some(line) = said.lines().next() else { return 0 };

        match line.trim().parse() {
            Ok(brightness) => brightness,
            // Nought, which a check reads as "the machine did not say". A
            // backlight that answered with something which is not a number is
            // a different fault from one that did not answer at all.
            Err(fault) => {
                eprintln!("console-stage: the backlight said {line:?}: {fault}");

                0
            }
        }
    }

    /// What the machine says the volume is, as a percentage.
    ///
    /// Asked of pactl rather than of `console-volume`, for the same reason
    /// `brightness` reads the backlight rather than asking
    /// `console-brightness`: a check that reads the machine through the
    /// program it is checking cannot tell a working program from a broken one
    /// that lies the same way twice.
    ///
    /// The field is the one `console_settings::rocker::level` takes, and that
    /// is where the reading is decided. It is read again here rather than
    /// imported because the stage would have to carry the whole settings panel
    /// -- and GTK behind it -- into the fast suite to borrow four words.
    pub fn volume(&mut self) -> i64 {
        let said = self.user("pactl get-sink-volume @DEFAULT_SINK@");
        let word = said.lines().next().and_then(|line| line.split_whitespace().nth(4));

        let Some(word) = word else { return 0 };

        match word.trim_end_matches('%').parse() {
            Ok(volume) => volume,
            // As above: nought is "it did not say", and pactl answering with
            // something that is not a percentage is worth telling apart from
            // pactl not answering.
            Err(fault) => {
                eprintln!("console-stage: pactl said {word:?} where the level goes: {fault}");

                0
            }
        }
    }

    pub fn services(&mut self) -> Vec<String> {
        let said = self.user(
            "systemctl --user is-active console-controller \
             console-keyboard console-bar console-session console-paper",
        );
        said.split_whitespace().map(str::to_string).collect()
    }

    /// How many times each of those has had to be started again.
    ///
    /// A service that restarts is active almost all of the time, so asking
    /// whether it is up says nothing about a service dying every few minutes.
    /// This is the number that does.
    pub fn restarts(&mut self) -> Vec<String> {
        let said = self.user(
            "systemctl --user show --value -p NRestarts console-controller \
             console-keyboard console-bar console-session console-paper",
        );
        said.split_whitespace().map(str::to_string).collect()
    }

    /// What is in a directory, for the things that leave one behind.
    pub fn files(&mut self, where_: &str) -> Vec<String> {
        let said = self.user(&format!("ls -1 {} 2>/dev/null", quoted(where_)));
        let mut found: Vec<String> = said
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        found.sort();
        found
    }

    pub fn journal(&mut self, unit: &str, lines: u32) -> String {
        self.user(&format!(
            "journalctl --user -u {unit} -n {lines} --no-pager"
        ))
    }

    /// The choosers on screen, by name.
    ///
    /// A chooser is a layer and not a window, so nothing that counts windows can
    /// see one. Asking the profile instead is not the same question: a chooser
    /// hands the desktop's buttons back as it closes, so with two of them open
    /// the pad comes back while one is still drawn, and a check that asks only
    /// about the profile passes with a menu on the screen.
    pub fn menus(&mut self) -> Vec<String> {
        let said = self.hypr("layers -j");

        let Some(found) = read(&said) else {
            return Vec::new();
        };

        let mut named = Vec::new();

        for screen in found
            .as_object()
            .into_iter()
            .flat_map(|screens| screens.values())
        {
            for level in screen["levels"]
                .as_object()
                .into_iter()
                .flat_map(|levels| levels.values())
            {
                for layer in level.as_array().into_iter().flatten() {
                    let namespace = layer["namespace"].as_str().unwrap_or_default();

                    if !FURNITURE.contains(&namespace) {
                        named.push(namespace.to_string());
                    }
                }
            }
        }

        named.sort();
        named
    }

    /// Wait for something to become true, rather than for a number of seconds.
    ///
    /// How long a chooser takes to draw or to go is how busy the machine is, and
    /// a check that sleeps for a fixed guess passes on a quiet device and fails
    /// on the same device behind a screenshot somebody else is taking.
    ///
    /// Answers whether it happened rather than raising, so a check says what it
    /// was waiting for in its own words.
    pub fn until(&mut self, mut what: impl FnMut(&mut Self) -> Seen, seconds: f64) -> Waited {
        for _ in 0..toward_zero_u32(seconds / 0.5) {
            self.settle(0.5);

            if what(self) == Seen::Yes {
                return Waited::Happened;
            }
        }

        Waited::RanOut
    }

    /// Wait for a chooser to be on screen.
    pub fn drawn(&mut self, seconds: f64) -> Waited {
        self.until(menus_up, seconds)
    }

    /// Wait for every chooser to have left the screen.
    pub fn gone(&mut self, seconds: f64) -> Waited {
        self.until(|seen| menus_up(seen).flipped(), seconds)
    }

    /// When the decoded frames were written, and when the picture was.
    ///
    /// awww names a cache file after the picture's path, its size and how it was
    /// fitted to the screen. Nothing in that name comes from what is inside the
    /// file, so a redrawn garden installed at the same path is played as the old
    /// picture's frames over the new picture's still.
    pub fn frame_cache(&mut self, picture: &str) -> (Option<i64>, Option<i64>) {
        let said = self.user(&format!(
            "find ~/.cache/awww -type f -exec stat -c %Y {{}} + 2>/dev/null \
             | sort -n | tail -1; echo --; stat -c %Y {} 2>/dev/null",
            quoted(picture)
        ));
        let mut halves = said.split("--");
        let last = |half: Option<&str>| {
            let mut found = None;

            // Most of the words here are not numbers. That is what is being
            // looked past rather than a failure to report, and the last one
            // that is a number is the answer.
            for word in half.unwrap_or_default().split_whitespace() {
                if let Ok(number) = word.parse::<i64>() {
                    found = Some(number);
                }
            }

            found
        };
        (last(halves.next()), last(halves.next()))
    }

    /// What the wallpaper daemon says it is showing, per screen.
    ///
    /// Colour alone cannot answer this here. The bare background is the
    /// palette's darkest colour on purpose, so that a wallpaper arriving after
    /// the compositor does not announce itself, and the resting garden is that
    /// same colour; a screen nothing painted and a screen the garden painted
    /// read alike. The daemon knows which it is, so it is asked.
    pub fn wallpaper(&mut self) -> String {
        let whom = self.whoever();
        let asked = format!("{} && awww query", session_env(&whom));
        self.user(&asked)
    }

    // --------------------------------------------------------------- colour

    /// The device's screen, taken there and fetched, kept until it moves.
    ///
    /// Every question about colour is asked of one picture, because each one
    /// costs a screenshot, a copy over the network and a second and a half of
    /// waiting. Anything pressed throws it away again.
    fn picture(&mut self) -> Result<&Picture, String> {
        if self.taken.is_none() {
            self.exec_cmd("grim /tmp/console-check.png");
            std::thread::sleep(Duration::from_secs_f64(1.5));
            let here = std::env::temp_dir().join(format!("console-shot-{}", std::process::id()));
            std::fs::create_dir_all(&here).map_err(|fault| fault.to_string())?;
            let shot = here.join("screen.png");
            let _ = Command::new("scp")
                .args(["-q", &format!("{}:/tmp/console-check.png", self.host)])
                .arg(&shot)
                .status();
            self.ssh("rm -f /tmp/console-check.png");
            self.taken = Some(Picture::read(&shot)?);
            self.kept = Some(here);
        }

        // Set just above, on the one road that reaches here without one. Said
        // rather than unwrapped, because "it was set a moment ago" is a claim
        // about code that goes on being true until somebody edits the road.
        self.taken.as_ref().ok_or_else(|| "the device took a picture and then had none".to_string())
    }

    /// What colour most of the device's screen is.
    pub fn background(&mut self) -> Result<String, String> {
        Ok(self.picture()?.commonest())
    }

    /// The colour of one place in the desktop's layout, on the device.
    pub fn colour(&mut self, across: f64, down: f64) -> Result<String, String> {
        let screen = console_screen::Screen::read(
            &std::fs::read_to_string(crate::root().join(console_screen::CONFIG))
                .map_err(|fault| fault.to_string())?,
        )?;
        where_(self.picture()?, across, down, &screen)
    }

    /// The average colour of a small patch, placed by fraction.
    pub fn patch(&mut self, across: f64, down: f64) -> Result<String, String> {
        Ok(self.picture()?.average(across, down, crate::picture::PATCH))
    }

    /// Forget the picture, and put the desk back where a check expects it.
    ///
    /// A chooser some earlier check left drawn is not scenery. A button is
    /// resolved against the profile that is loaded, and a button a chooser's
    /// profile names and sends nowhere is not sent at all, so the next check's
    /// presses quietly come to mean something else. 060 read as "R1 did not
    /// move" because 050 had failed with the guide still up and the pad in Menu,
    /// which maps no right bumper; 080 passed on a chooser it had never opened.
    /// Neither was a fact about the machine.
    ///
    /// Best effort, and it never fails. It is called outside whatever turns a
    /// check's own trouble into a result, so anything raised here would end the
    /// tier rather than fail one check.
    pub fn fresh(&mut self) {
        self.taken = None;

        if self.dry {
            return;
        }

        for _ in 0..3 {
            if self.menus().is_empty() {
                break;
            }

            self.press("b");
            std::thread::sleep(Duration::from_secs_f64(0.8));
        }

        if self.profile() != "Router" {
            self.load_profile(console_pad::router::NAME);
            std::thread::sleep(Duration::from_secs_f64(0.5));
        }
    }

    pub fn close(&mut self) {
        if let Some(here) = self.kept.take() {
            let _ = std::fs::remove_dir_all(here);
        }
    }
}

/// The same, as a question about one profile rather than about the machine.
pub fn capability_under(here: Option<&Profile>, button: &str) -> Option<String> {
    let itself = match vocabulary::button_name(button) {
        Ok(name) => Some(format!("Gamepad:Button:{name}")),
        // A button this vocabulary has no name for, which is the question being
        // asked rather than a failure to answer it.
        Err(_unnamed) => None,
    };

    let Some(here) = here else { return itself };

    // A button the vocabulary does not know cannot be mapped by a profile
    // either, so the answer is the button's own name -- which is where the
    // empty case at the bottom of this function arrives anyway.
    let Ok(named) = here.for_button(button) else { return itself };

    let sent = named
        .iter()
        .flat_map(|mapping| &mapping.targets)
        .find_map(|target| spoken_as(target.kind, &target.name));

    match (sent, named.is_empty()) {
        (Some(sent), _) => Some(sent),
        (None, false) => None,
        (None, true) => itself,
    }
}

fn address(client: &serde_json::Value) -> Option<String> {
    client["address"].as_str().map(str::to_string)
}

fn read(said: &str) -> Option<serde_json::Value> {
    match serde_json::from_str(said) {
        Ok(parsed) => Some(parsed),
        // For a check, an answer that is not JSON is the device not having
        // answered. Said out loud, because no answer and a malformed one are
        // two different faults on the machine and only one of them is quiet.
        Err(fault) => {
            eprintln!("console-stage: the device answered with something that is not JSON: {fault}");

            None
        }
    }
}

/// One word for a shell, whatever is in it.
pub fn quoted(said: &str) -> String {
    format!("'{}'", said.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A device nothing is sent to, so its address is any address.
    fn dry() -> Device {
        Device::new("root@handheld", Dry::Pretend).expect("a stage")
    }

    #[test]
    fn a_word_with_a_quote_in_it_is_still_one_word() {
        assert_eq!(quoted("plain"), "'plain'");
        assert_eq!(quoted("it's"), r"'it'\''s'");
    }

    /// A paddle sent as a paddle arrives as a paddle, where a thumb on the same
    /// paddle arrives as a function key, and the daemon that acts on function
    /// keys sees nothing at all.
    #[test]
    fn a_button_is_sent_as_what_the_loaded_profile_makes_of_it() {
        let profiles = every_profile(&crate::root()).expect("the profiles");
        assert_eq!(
            capability_under(profiles.get("router"), "right-paddle-top"),
            Some("Keyboard:KeyF15".to_string())
        );
    }

    /// A button that means nothing with a chooser up is still sent. There was
    /// a profile once that named such buttons and sent them nowhere, because
    /// what a button meant was the profile's to say and the only way to say
    /// "not here" was silence. What a press comes to is the daemon's now, so
    /// the profile sends the button and the daemon is what has nothing to do
    /// with it.
    #[test]
    fn a_button_that_means_nothing_in_a_chooser_is_still_sent() {
        let profiles = every_profile(&crate::root()).expect("the profiles");
        assert_eq!(
            capability_under(profiles.get("router"), "view"),
            Some("Gamepad:Button:Select".to_string())
        );
    }

    /// Which is what the device does with it too.
    #[test]
    fn a_button_a_profile_says_nothing_about_is_sent_as_itself() {
        let profiles = every_profile(&crate::root()).expect("the profiles");
        assert_eq!(
            capability_under(profiles.get("keyboard"), "a"),
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

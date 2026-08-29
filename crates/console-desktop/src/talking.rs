//! Speaking to the nested compositor, and looking at what it has.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use console_screen::Screen;

use crate::nested;

/// How long the screen is given to arrive.
pub const A_SCREEN: Duration = Duration::from_secs(12);

/// How long whatever was opened is given to draw something.
pub const SOMETHING: Duration = Duration::from_secs(20);

/// How long the wallpaper daemon is given to go once it has been asked.
pub const A_GOODBYE: Duration = Duration::from_secs(3);

/// Whether a process is still about.
fn still_there(pid: i32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// The environment a command inside the nested session is given.
pub struct Inside {
    pub environment: Vec<(String, String)>,
}

impl Inside {
    pub fn new(environment: Vec<(String, String)>, socket: &str, signature: &str) -> Self {
        let mut environment = environment;
        environment.push(("WAYLAND_DISPLAY".to_string(), socket.to_string()));
        environment.push((
            "HYPRLAND_INSTANCE_SIGNATURE".to_string(),
            signature.to_string(),
        ));
        Inside { environment }
    }

    pub fn command(&self, program: &str) -> Command {
        let mut asking = Command::new(program);
        asking.env_remove("HYPRLAND_INSTANCE_SIGNATURE");
        for (name, value) in &self.environment {
            asking.env(name, value);
        }
        asking
    }

    pub fn hyprctl(&self, arguments: &[&str]) -> String {
        self.command("hyprctl")
            .args(arguments)
            .output()
            .map(|done| String::from_utf8_lossy(&done.stdout).trim().to_string())
            .unwrap_or_default()
    }

    /// Wait until there is something to take a picture of.
    ///
    /// The screen is made a moment after the compositor starts, not with it, so
    /// everything that follows has to wait for it rather than assume it.
    pub fn wait_for_screen(&self, name: &str) -> bool {
        let by = Instant::now() + A_SCREEN;
        while Instant::now() < by {
            std::thread::sleep(Duration::from_millis(250));
            let taken = self.command("grim").args(["-o", name, "-"]).output();
            if taken.is_ok_and(|done| done.status.success()) {
                return true;
            }
        }
        false
    }

    /// The device's screen, and only that one: its mode, turn and density.
    ///
    /// Not a screen the size the desktop is laid out in. Everything drawn here
    /// is drawn at two and a half times that, the way it is on the device,
    /// because a stand-in at a density nobody uses agrees with you about how
    /// text sits and where a rounded edge falls.
    ///
    /// Done from out here rather than from the config, because a config can only
    /// have one handler for the compositor starting: registering a second one
    /// replaces the first, and the first is what starts the desktop.
    ///
    /// The window has to go last. Turning it off while it is the only screen
    /// leaves nothing to draw on, and nothing to photograph either.
    pub fn make_the_screen(&self, screen: &Screen) {
        self.hyprctl(&["output", "create", "headless"]);
        std::thread::sleep(Duration::from_millis(800));
        self.hyprctl(&["eval", &nested::made_headless(screen)]);
        std::thread::sleep(Duration::from_millis(500));
        self.hyprctl(&[
            "eval",
            r#"hl.monitor({ output = "WAYLAND-1", disabled = true })"#,
        ]);
        std::thread::sleep(Duration::from_millis(500));
    }

    /// Everything on the screen: the windows, and the layers over and under.
    ///
    /// A menu here is not a window. The panel, the guide and the keyboard are
    /// all layer surfaces, which is most of what somebody using this device
    /// touches, so anything that waits for a window to appear waits for ever.
    pub fn surfaces(&self) -> BTreeSet<String> {
        let mut on = BTreeSet::new();
        let address = |what: &serde_json::Value| {
            what.get("address")
                .and_then(|at| at.as_str())
                .map(str::to_string)
        };
        if let Ok(clients) =
            serde_json::from_str::<serde_json::Value>(&self.hyprctl(&["clients", "-j"]))
        {
            on.extend(clients.as_array().into_iter().flatten().filter_map(address));
        }
        if let Ok(layers) =
            serde_json::from_str::<serde_json::Value>(&self.hyprctl(&["layers", "-j"]))
        {
            for screen in layers
                .as_object()
                .into_iter()
                .flatten()
                .map(|(_, screen)| screen)
            {
                let levels = screen.get("levels").and_then(|at| at.as_object());
                for level in levels.into_iter().flatten().map(|(_, level)| level) {
                    on.extend(level.as_array().into_iter().flatten().filter_map(address));
                }
            }
        }
        on
    }

    /// Wait until what was asked for has put something on the screen.
    ///
    /// This used to be a sleep long enough for a window, which is a guess in
    /// both directions: too long for a terminal, and too short for a panel that
    /// reads the controller before it draws anything. What it is waiting for is
    /// a fact it can ask about.
    pub fn wait_for_something(&self, was: &BTreeSet<String>) -> bool {
        let by = Instant::now() + SOMETHING;
        while Instant::now() < by {
            if self.surfaces().difference(was).next().is_some() {
                // Drawn, not only mapped.
                std::thread::sleep(Duration::from_millis(600));
                return true;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        false
    }

    /// Look at whatever was opened.
    ///
    /// Every window on this desktop opens on an empty workspace of its own,
    /// which is the whole point of it: there is never a second window to share a
    /// screen with. It also means a window that has just opened is not
    /// necessarily the one being looked at, and a picture of the workspace it is
    /// not on is a picture of nothing.
    pub fn show_a_window(&self) {
        let Ok(clients) =
            serde_json::from_str::<serde_json::Value>(&self.hyprctl(&["clients", "-j"]))
        else {
            return;
        };
        let where_ = clients
            .as_array()
            .and_then(|every| every.first())
            .and_then(|client| client.get("workspace"))
            .and_then(|workspace| workspace.get("name"))
            .and_then(|name| name.as_str());
        let Some(where_) = where_ else { return };
        self.hyprctl(&[
            "dispatch",
            &format!(r#"hl.dsp.focus({{workspace = "{where_}"}})"#),
        ]);
        std::thread::sleep(Duration::from_millis(600));
    }

    /// Hand the wallpaper to a screen that did not exist when it was asked.
    ///
    /// awww gives the picture to the outputs it can see at the moment it is
    /// told, and here the screen is made after the session has started, so the
    /// daemon comes up with nothing to paint on and stays that way. On the
    /// device the screen is there before anything runs and this never arises.
    pub fn paint_the_background(&self, picture: &std::path::Path) {
        if !picture.exists() {
            return;
        }
        // The daemon is started by the session and takes a moment to listen.
        // Said once and quietly, this failed for a week without anybody
        // noticing, because a screen with no wallpaper on it is the palette's
        // own darkest colour and looks like a screen with one.
        let by = Instant::now() + Duration::from_secs(8);
        let mut last = String::new();
        while Instant::now() < by {
            let told = self
                .command("awww")
                .args(["img"])
                .arg(picture)
                .args(["--resize", "crop", "--transition-type", "none"])
                .output();
            match told {
                Ok(done) if done.status.success() => {
                    std::thread::sleep(Duration::from_millis(500));
                    return;
                }
                Ok(done) => last = String::from_utf8_lossy(&done.stderr).trim().to_string(),
                // Nothing to paint with, which is not a fault to wait out.
                Err(_) => return,
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        eprintln!("the wallpaper was never painted: {last}");
    }

    /// Send the bar away before the compositor goes, for the same reason.
    ///
    /// waybar's Hyprland modules keep a thread listening on the compositor's
    /// socket, owned by a static that outlives `main`. When the compositor
    /// leaves, that thread calls `exit` from inside itself; the static's
    /// destructor then runs and destroys the very thread that is running it,
    /// and destroying a joinable `std::thread` is defined to call
    /// `std::terminate`. So the bar aborts and leaves a core behind, on about
    /// three staged sessions in five, for a bar that had nothing left to do.
    /// Read out of one: `~IPC` at `backend.cpp:70`, under `exit(0)`, on a
    /// thread rather than on main.
    ///
    /// A signal is the whole answer. waybar installs no handler for TERM, so it
    /// stops where it stands without running an exit handler, and there is
    /// nothing left to abort about.
    ///
    /// Only this session's bar. Another staged desktop may be up in the next
    /// terminal and its bar is not this one's to take away, so they are told
    /// apart by the compositor each is talking to, which is the one thing a
    /// Wayland client cannot be running without.
    ///
    /// Only the bar itself. A copy of it, forked for one of the little programs
    /// it reads a module from, dies the same way and leaves the same core, but
    /// it does so within half a second of being forked and has been a zombie
    /// for the whole session by the time this is asked to stop anything. What
    /// that one wanted was for the bar to be started later, which is in the
    /// staged session-start.
    pub fn stop_the_bar(&self) {
        let bars = self.talking_to("waybar");
        for bar in &bars {
            // SAFETY: a signal to a process of this session's own, by its pid.
            unsafe { libc::kill(*bar, libc::SIGTERM) };
        }
        // Gone before the compositor is told to leave, or the wait was the
        // whole point and it was spent for nothing.
        let by = Instant::now() + A_GOODBYE;
        while Instant::now() < by && bars.iter().any(|bar| still_there(*bar)) {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Every process of a given name that is a client of this session.
    ///
    /// Found by the socket in its environment rather than by name alone,
    /// because the name alone is every such program on the machine: this
    /// desktop's own bar, and the bar of whatever else is staged beside it.
    fn talking_to(&self, program: &str) -> Vec<i32> {
        let Some(socket) = self
            .environment
            .iter()
            .find(|(name, _)| name == "WAYLAND_DISPLAY")
        else {
            return Vec::new();
        };
        let wanted = format!("WAYLAND_DISPLAY={}", socket.1);
        let Ok(all) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        all.flatten()
            .filter_map(|found| {
                let at = found.path();
                let pid: i32 = at.file_name()?.to_str()?.parse().ok()?;
                let named = std::fs::read_to_string(at.join("comm")).ok()?;
                if named.trim() != program {
                    return None;
                }
                let held = std::fs::read(at.join("environ")).ok()?;
                held.split(|byte| *byte == 0)
                    .any(|said| said == wanted.as_bytes())
                    .then_some(pid)
            })
            .collect()
    }

    /// Ask the wallpaper daemon to leave, before the compositor does.
    ///
    /// awww's main loop flushes its wayland connection and unwraps what comes
    /// back, so a daemon still holding one when the compositor goes is a daemon
    /// that aborts: the socket closes, the flush is a broken pipe, and
    /// daemon/src/main.rs:733 makes a panic of it. Every staged session left a
    /// core behind that way, ninety of them before anyone read one. Asked while
    /// there is still a compositor to answer, it leaves its own loop and exits,
    /// and there is nothing left to abort about.
    ///
    /// Best effort at both ends, because this is tidiness and nothing waits on
    /// it: a session that started no daemon has nothing to tell, and one that
    /// will not go is left to the leaving that follows.
    pub fn stop_the_wallpaper(&self) {
        let told = self.command("awww").arg("kill").output();
        if !told.is_ok_and(|done| done.status.success()) {
            return;
        }
        // Answered is not gone. The daemon says Ok from inside the loop it is
        // about to fall out of, and the compositor is told to leave next.
        let by = Instant::now() + A_GOODBYE;
        while Instant::now() < by {
            let still = self.command("awww").arg("query").output();
            if !still.is_ok_and(|done| done.status.success()) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

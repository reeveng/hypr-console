//! Speaking to the nested compositor, and looking at what it has.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use console_core_never::Never;
use console_screen::Screen;
use console_waiting::{Patience, Seen, until};

use crate::nested;

pub const A_SCREEN: Duration = Duration::from_secs(12);

pub const SOMETHING: Duration = Duration::from_secs(20);

pub const A_GOODBYE: Duration = Duration::from_secs(3);

pub const A_MONITOR: Duration = Duration::from_secs(5);

pub const A_STILL_SCREEN: Duration = Duration::from_secs(3);

pub const A_PAINT: Duration = Duration::from_secs(8);

const HEADLESS: &str = "HEADLESS-1";

const WINDOWED: &str = "WAYLAND-1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum There {
    Yes,
    No,
}

pub use console_waiting::Waited;

fn still_there(pid: i32) -> Result<There, Never> {
    Ok(match Path::new(&format!("/proc/{pid}")).exists() {
        true => There::Yes,
        false => There::No,
    })
}

pub struct Inside {
    pub environment: Vec<(String, String)>,
}

impl Inside {
    pub fn new(
        environment: Vec<(String, String)>,
        socket: &str,
        signature: &str,
    ) -> Result<Self, Never> {
        let mut environment = environment;
        environment.push(("WAYLAND_DISPLAY".to_string(), socket.to_string()));
        environment.push((
            "HYPRLAND_INSTANCE_SIGNATURE".to_string(),
            signature.to_string(),
        ));
        Ok(Inside { environment })
    }

    pub fn command(&self, program: &str) -> Result<Command, Never> {
        let mut asking = Command::new(program);
        asking.env_remove("HYPRLAND_INSTANCE_SIGNATURE");

        for (name, value) in &self.environment {
            asking.env(name, value);
        }

        Ok(asking)
    }

    pub fn hyprctl(&self, arguments: &[&str]) -> Result<String, Never> {
        let Ok(mut asking) = self.command("hyprctl");

        Ok(match asking.args(arguments).output() {
            Ok(done) => String::from_utf8_lossy(&done.stdout).trim().to_string(),
            Err(fault) => {
                eprintln!("console-desktop: hyprctl {}: {fault}", arguments.join(" "));

                String::new()
            }
        })
    }

    pub fn wait_for_screen(&self, name: &str) -> Result<Waited, Never> {
        let Ok(patience) = Patience::asking_every(A_SCREEN, Duration::from_millis(250));

        until(patience, || {
            let Ok(mut asking) = self.command("grim");
            let taken = asking.args(["-o", name, "-"]).output();

            Ok(match taken.is_ok_and(|done| done.status.success()) {
                true => Seen::Yes,
                false => Seen::NotYet,
            })
        })
    }

    fn monitors(&self) -> Result<BTreeSet<String>, Never> {
        let Ok(said) = self.hyprctl(&["monitors", "all", "-j"]);

        let Ok(monitors) = serde_json::from_str::<serde_json::Value>(&said) else {
            return Ok(BTreeSet::new());
        };

        Ok(monitors
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|monitor| {
                monitor.get("name").and_then(|name| name.as_str()).map(str::to_string)
            })
            .collect())
    }

    fn wait_for_monitors(&self, named: &str, want: Seen) -> Result<Waited, Never> {
        let Ok(patience) = Patience::asking_every(A_MONITOR, Duration::from_millis(50));

        until(patience, || {
            let Ok(monitors) = self.monitors();

            Ok(match monitors.contains(named) == (want == Seen::Yes) {
                true => Seen::Yes,
                false => Seen::NotYet,
            })
        })
    }

    pub fn make_the_screen(&self, screen: &Screen) -> Result<(), Never> {
        let Ok(_created) = self.hyprctl(&["output", "create", "headless"]);
        let Ok(arrived) = self.wait_for_monitors(HEADLESS, Seen::Yes);

        match arrived {
            Waited::Happened => {},
            Waited::RanOut => eprintln!("console-desktop: {HEADLESS} never appeared"),
        }

        let Ok(made) = nested::made_headless(screen);
        let Ok(_said) = self.hyprctl(&["eval", &made]);
        let Ok(sized) = self.wait_for_mode(screen);

        match sized {
            Waited::Happened => {},
            Waited::RanOut => {
                eprintln!("console-desktop: {HEADLESS} never took the mode it was given");
            }
        }

        let Ok(_disabled) = self.hyprctl(&[
            "eval",
            r#"hl.monitor({ output = "WAYLAND-1", disabled = true })"#,
        ]);
        let Ok(alone) = self.wait_for_monitors(WINDOWED, Seen::NotYet);

        match alone {
            Waited::Happened => {},
            Waited::RanOut => eprintln!("console-desktop: {WINDOWED} would not go away"),
        }

        Ok(())
    }

    fn wait_for_mode(&self, screen: &Screen) -> Result<Waited, Never> {
        let (wide, tall) = screen.mode;
        let Ok(patience) = Patience::asking_every(A_MONITOR, Duration::from_millis(50));

        until(patience, || {
            let Ok(said) = self.hyprctl(&["monitors", "all", "-j"]);

            let Ok(monitors) = serde_json::from_str::<serde_json::Value>(&said) else {
                return Ok(Seen::NotYet);
            };

            let took = monitors.as_array().into_iter().flatten().any(|monitor| {
                let named = monitor.get("name").and_then(|name| name.as_str());
                let across = monitor.get("width").and_then(|width| width.as_i64());
                let down = monitor.get("height").and_then(|height| height.as_i64());

                named == Some(HEADLESS)
                    && across == Some(i64::from(wide))
                    && down == Some(i64::from(tall))
            });

            Ok(match took {
                true => Seen::Yes,
                false => Seen::NotYet,
            })
        })
    }

    fn shot(&self) -> Result<Option<Vec<u8>>, Never> {
        let Ok(mut asking) = self.command("grim");
        let taken = asking.args(["-t", "ppm", "-"]).output();

        Ok(match taken {
            Ok(done) if done.status.success() => Some(done.stdout),
            Ok(_it_would_not_take_a_picture) => None,
            Err(_grim_is_not_here) => None,
        })
    }

    pub fn wait_for_a_still_screen(&self) -> Result<Waited, Never> {
        let Ok(patience) = Patience::asking_every(A_STILL_SCREEN, Duration::from_millis(100));
        let mut before: Option<Vec<u8>> = None;

        until(patience, || {
            let Ok(now) = self.shot();

            let Some(now) = now else { return Ok(Seen::NotYet) };

            let same = before.as_ref() == Some(&now);

            before = Some(now);

            Ok(match same {
                true => Seen::Yes,
                false => Seen::NotYet,
            })
        })
    }

    pub fn surfaces(&self) -> Result<BTreeSet<String>, Never> {
        let mut on = BTreeSet::new();
        let address = |what: &serde_json::Value| {
            what.get("address")
                .and_then(|at| at.as_str())
                .map(str::to_string)
        };

        let Ok(said_clients) = self.hyprctl(&["clients", "-j"]);

        match serde_json::from_str::<serde_json::Value>(&said_clients) {
            Ok(clients) => {
                on.extend(clients.as_array().into_iter().flatten().filter_map(address));
            }
            Err(_the_compositor_said_nothing) => {},
        }

        let Ok(said_layers) = self.hyprctl(&["layers", "-j"]);

        match serde_json::from_str::<serde_json::Value>(&said_layers) {
            Ok(layers) => {
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
            Err(_the_compositor_said_nothing) => {},
        }

        Ok(on)
    }

    pub fn wait_for_something(&self, was: &BTreeSet<String>) -> Result<Waited, Never> {
        let Ok(patience) = Patience::asking_every(SOMETHING, Duration::from_millis(100));
        let Ok(came) = until(patience, || {
            let Ok(surfaces) = self.surfaces();

            Ok(match surfaces.difference(was).next().is_some() {
                true => Seen::Yes,
                false => Seen::NotYet,
            })
        });

        match came {
            Waited::Happened => self.wait_for_a_still_screen(),
            Waited::RanOut => Ok(Waited::RanOut),
        }
    }

    pub fn show_a_window(&self) -> Result<(), Never> {
        let Ok(said) = self.hyprctl(&["clients", "-j"]);

        let Ok(clients) = serde_json::from_str::<serde_json::Value>(&said) else {
            return Ok(());
        };

        let where_ = clients
            .as_array()
            .and_then(|every| every.first())
            .and_then(|client| client.get("workspace"))
            .and_then(|workspace| workspace.get("name"))
            .and_then(|name| name.as_str());

        let Some(where_) = where_ else { return Ok(()) };

        let Ok(_focused) = self.hyprctl(&[
            "dispatch",
            &format!(r#"hl.dsp.focus({{workspace = "{where_}"}})"#),
        ]);
        let Ok(patience) = Patience::asking_every(A_GOODBYE, Duration::from_millis(50));
        let Ok(there) = until(patience, || {
            let Ok(said) = self.hyprctl(&["activeworkspace", "-j"]);

            let Ok(now) = serde_json::from_str::<serde_json::Value>(&said) else {
                return Ok(Seen::NotYet);
            };

            let named = now.get("name").and_then(|name| name.as_str());

            Ok(match named == Some(where_) {
                true => Seen::Yes,
                false => Seen::NotYet,
            })
        });

        match there {
            Waited::Happened => {},
            Waited::RanOut => eprintln!("console-desktop: {where_} never came to the front"),
        }

        let Ok(_still) = self.wait_for_a_still_screen();

        Ok(())
    }

    pub fn paint_the_background(&self, picture: &std::path::Path) -> Result<(), Never> {
        match picture.exists() {
            true => {},
            false => return Ok(()),
        }

        let Ok(patience) = Patience::asking_every(A_PAINT, Duration::from_millis(250));
        let mut last = String::new();
        let Ok(painted) = until(patience, || {
            let Ok(mut asking) = self.command("awww");
            let told = asking
                .args(["img"])
                .arg(picture)
                .args(["--resize", "crop", "--transition-type", "none"])
                .output();

            Ok(match told {
                Ok(done) if done.status.success() => Seen::Yes,
                Ok(done) => {
                    last = String::from_utf8_lossy(&done.stderr).trim().to_string();

                    Seen::NotYet
                }
                Err(_there_is_no_awww_here) => {
                    last = "awww is not on this machine".to_string();

                    Seen::Yes
                }
            })
        });

        match painted {
            Waited::Happened => {
                let Ok(_still) = self.wait_for_a_still_screen();
            }
            Waited::RanOut => eprintln!("the wallpaper was never painted: {last}"),
        }

        Ok(())
    }

    pub fn stop_the_bar(&self) -> Result<(), Never> {
        let Ok(bars) = self.talking_to("waybar");

        for bar in &bars {
            // SAFETY: a signal to a process of this session's own, by its pid.
            unsafe { libc::kill(*bar, libc::SIGTERM) };
        }

        let Ok(patience) = Patience::asking_every(A_GOODBYE, Duration::from_millis(50));
        let Ok(_went) = until(patience, || {
            let left = bars.iter().any(|bar| {
                let Ok(there) = still_there(*bar);

                there == There::Yes
            });

            Ok(match left {
                true => Seen::NotYet,
                false => Seen::Yes,
            })
        });

        Ok(())
    }

    fn talking_to(&self, program: &str) -> Result<Vec<i32>, Never> {
        let Some(socket) = self
            .environment
            .iter()
            .find(|(name, _)| name == "WAYLAND_DISPLAY")
        else {
            return Ok(Vec::new());
        };

        let wanted = format!("WAYLAND_DISPLAY={}", socket.1);

        let Ok(all) = std::fs::read_dir("/proc") else {
            return Ok(Vec::new());
        };

        Ok(all
            .flatten()
            .filter_map(|found| {
                let at = found.path();

                let called = at.file_name()?;
                let said = called.to_str()?;

                let Ok(pid) = said.parse::<i32>() else { return None };

                let Ok(named) = std::fs::read_to_string(at.join("comm")) else { return None };


                match named.trim() == program {
                    true => {},
                    false => return None,
                }

                let Ok(held) = std::fs::read(at.join("environ")) else { return None };

                held.split(|byte| *byte == 0)
                    .any(|said| said == wanted.as_bytes())
                    .then_some(pid)
            })
            .collect())
    }

    pub fn stop_the_wallpaper(&self) -> Result<(), Never> {
        let Ok(mut asking) = self.command("awww");
        let told = asking.arg("kill").output();

        match told.is_ok_and(|done| done.status.success()) {
            true => {},
            false => return Ok(()),
        }

        let Ok(patience) = Patience::asking_every(A_GOODBYE, Duration::from_millis(100));
        let Ok(_went) = until(patience, || {
            let Ok(mut asking) = self.command("awww");
            let still = asking.arg("query").output();

            Ok(match still.is_ok_and(|done| done.status.success()) {
                true => Seen::NotYet,
                false => Seen::Yes,
            })
        });

        Ok(())
    }
}

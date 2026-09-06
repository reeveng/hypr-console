//! Speaking to the nested compositor, and looking at what it has.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use console_never::Never;
use console_screen::Screen;

use crate::nested;

pub const A_SCREEN: Duration = Duration::from_secs(12);

pub const SOMETHING: Duration = Duration::from_secs(20);

pub const A_GOODBYE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum There {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waited {
    Happened,
    RanOut,
}

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
        let by = Instant::now() + A_SCREEN;

        while Instant::now() < by {
            std::thread::sleep(Duration::from_millis(250));
            let Ok(mut asking) = self.command("grim");
            let taken = asking.args(["-o", name, "-"]).output();

            match taken.is_ok_and(|done| done.status.success()) {
                true => return Ok(Waited::Happened),
                false => {},
            }
        }

        Ok(Waited::RanOut)
    }

    pub fn make_the_screen(&self, screen: &Screen) -> Result<(), Never> {
        let Ok(_created) = self.hyprctl(&["output", "create", "headless"]);

        std::thread::sleep(Duration::from_millis(800));
        let Ok(made) = nested::made_headless(screen);
        let Ok(_said) = self.hyprctl(&["eval", &made]);

        std::thread::sleep(Duration::from_millis(500));
        let Ok(_disabled) = self.hyprctl(&[
            "eval",
            r#"hl.monitor({ output = "WAYLAND-1", disabled = true })"#,
        ]);

        std::thread::sleep(Duration::from_millis(500));

        Ok(())
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
        let by = Instant::now() + SOMETHING;

        while Instant::now() < by {
            let Ok(surfaces) = self.surfaces();

            match surfaces.difference(was).next().is_some() {
                true => {
                    std::thread::sleep(Duration::from_millis(600));

                    return Ok(Waited::Happened);
                }
                false => {},
            }

            std::thread::sleep(Duration::from_millis(200));
        }

        Ok(Waited::RanOut)
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

        std::thread::sleep(Duration::from_millis(600));

        Ok(())
    }

    pub fn paint_the_background(&self, picture: &std::path::Path) -> Result<(), Never> {
        match picture.exists() {
            true => {},
            false => return Ok(()),
        }

        let by = Instant::now() + Duration::from_secs(8);
        let mut last = String::new();

        while Instant::now() < by {
            let Ok(mut asking) = self.command("awww");
            let told = asking
                .args(["img"])
                .arg(picture)
                .args(["--resize", "crop", "--transition-type", "none"])
                .output();

            match told {
                Ok(done) if done.status.success() => {
                    std::thread::sleep(Duration::from_millis(500));

                    return Ok(());
                }
                Ok(done) => last = String::from_utf8_lossy(&done.stderr).trim().to_string(),
                Err(_) => return Ok(()),
            }

            std::thread::sleep(Duration::from_millis(250));
        }

        eprintln!("the wallpaper was never painted: {last}");

        Ok(())
    }

    pub fn stop_the_bar(&self) -> Result<(), Never> {
        let Ok(bars) = self.talking_to("waybar");

        for bar in &bars {
            // SAFETY: a signal to a process of this session's own, by its pid.
            unsafe { libc::kill(*bar, libc::SIGTERM) };
        }

        let by = Instant::now() + A_GOODBYE;
        let any_left = |bars: &[i32]| {
            bars.iter().any(|bar| {
                let Ok(there) = still_there(*bar);

                there == There::Yes
            })
        };

        while Instant::now() < by && any_left(&bars) {
            std::thread::sleep(Duration::from_millis(50));
        }

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

        let by = Instant::now() + A_GOODBYE;

        while Instant::now() < by {
            let Ok(mut asking) = self.command("awww");
            let still = asking.arg("query").output();

            match still.is_ok_and(|done| done.status.success()) {
                true => {},
                false => return Ok(()),
            }

            std::thread::sleep(Duration::from_millis(100));
        }

        Ok(())
    }
}

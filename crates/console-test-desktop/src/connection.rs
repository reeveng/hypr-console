//! Connecting to the nested compositor, and looking at what it has.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use console_compositor::{Query, Request};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_screen::Screen;
use console_waiting::{Schedule, Ready, until, until_handed};

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

pub use console_waiting::Outcome;

fn still_there(pid: i32) -> Result<There, Never> {
    Ok(match Path::new(&format!("/proc/{pid}")).exists() {
        true => There::Yes,
        false => There::No,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instance<'a> {
    pub socket: &'a str,
    pub signature: &'a str,
}

pub struct Inside {
    pub environment: Vec<(String, String)>,
}

impl Inside {
    pub fn new(environment: Vec<(String, String)>, at: Instance<'_>) -> Result<Self, Never> {
        let mut environment = environment;
        environment.push(("WAYLAND_DISPLAY".to_string(), at.socket.to_string()));
        environment.push((
            "HYPRLAND_INSTANCE_SIGNATURE".to_string(),
            at.signature.to_string(),
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
        let Ok(name) = Program::Hyprctl.name();
        let Ok(mut asking) = self.command(name);

        Ok(match asking.args(arguments).output() {
            Ok(done) => String::from_utf8_lossy(&done.stdout).trim().to_string(),
            Err(fault) => {
                eprintln!("console-desktop: hyprctl {}: {fault}", arguments.join(" "));

                String::new()
            }
        })
    }

    fn asking(&self, question: Query) -> Result<Option<console_compositor::Answer>, Never> {
        let Ok(name) = Program::Hyprctl.name();
        let Ok(asking) = self.command(name);

        Ok(match console_compositor::query_with(asking, question) {
            Ok(said) => Some(said),
            Err(why) => {
                eprintln!("console-desktop: {why}");

                None
            }
        })
    }

    fn told(&self, what: Request, lua: &str) -> Result<console_compositor::DispatchResult, Never> {
        let Ok(name) = Program::Hyprctl.name();
        let Ok(telling) = self.command(name);

        console_compositor::request_with(telling, what, lua)
    }

    pub fn wait_for_screen(&self, name: &str) -> Result<Outcome, Never> {
        let Ok(patience) = Schedule::asking_every(A_SCREEN, Duration::from_millis(250));

        until(patience, || {
            let Ok(mut asking) = self.command("grim");
            let taken = asking.args(["-o", name, "-"]).output();

            Ok(match taken.is_ok_and(|done| done.status.success()) {
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        })
    }

    fn monitors(&self) -> Result<Vec<console_compositor::Monitor>, Never> {
        let Ok(said) = self.asking(Query::Monitors);

        Ok(match said {
            Some(console_compositor::Answer::Monitors(monitors)) => monitors,
            Some(_not_what_was_asked) => Vec::new(),
            None => Vec::new(),
        })
    }

    fn named(&self) -> Result<BTreeSet<String>, Never> {
        let Ok(monitors) = self.monitors();

        Ok(monitors.into_iter().map(|monitor| monitor.named).collect())
    }

    fn wait_for_monitors(&self, named: &str, want: Ready) -> Result<Outcome, Never> {
        let Ok(patience) = Schedule::asking_every(A_MONITOR, Duration::from_millis(50));

        until(patience, || {
            let Ok(monitors) = self.named();

            Ok(match monitors.contains(named) == (want == Ready::Yes) {
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        })
    }

    pub fn make_the_screen(&self, screen: &Screen) -> Result<Outcome, Never> {
        let Ok(_created) = self.hyprctl(&["output", "create", "headless"]);
        let Ok(arrived) = self.wait_for_monitors(HEADLESS, Ready::Yes);

        match arrived {
            Outcome::Happened => {},
            Outcome::RanOut => eprintln!("console-desktop: {HEADLESS} never appeared"),
        }

        let Ok(made) = nested::made_headless(screen);
        let Ok(_said) = self.told(Request::Eval, &made);
        let Ok(sized) = self.wait_for_mode(screen);

        match sized {
            Outcome::Happened => {},
            Outcome::RanOut => {
                eprintln!("console-desktop: {HEADLESS} never took the mode it was given");
            }
        }

        let Ok(_disabled) =
            self.told(Request::Eval, r#"hl.monitor({ output = "WAYLAND-1", disabled = true })"#);
        let Ok(alone) = self.wait_for_monitors(WINDOWED, Ready::NotYet);

        match alone {
            Outcome::Happened => {},
            Outcome::RanOut => eprintln!("console-desktop: {WINDOWED} would not go away"),
        }

        Ok(match (arrived, sized, alone) {
            (Outcome::Happened, Outcome::Happened, Outcome::Happened) => Outcome::Happened,
            (Outcome::RanOut, Outcome::Happened | Outcome::RanOut, Outcome::Happened | Outcome::RanOut)
            | (Outcome::Happened, Outcome::RanOut, Outcome::Happened | Outcome::RanOut)
            | (Outcome::Happened, Outcome::Happened, Outcome::RanOut) => Outcome::RanOut,
        })
    }

    fn wait_for_mode(&self, screen: &Screen) -> Result<Outcome, Never> {
        let (wide, tall) = (screen.mode.width, screen.mode.height);
        let Ok(patience) = Schedule::asking_every(A_MONITOR, Duration::from_millis(50));

        until(patience, || {
            let Ok(monitors) = self.monitors();

            let took = monitors.iter().any(|monitor| {
                monitor.named == HEADLESS
                    && monitor.size == Some((i64::from(wide), i64::from(tall)))
            });

            Ok(match took {
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        })
    }

    fn shot(&self) -> Result<Option<Vec<u8>>, Never> {
        let Ok(mut asking) = self.command("grim");
        let taken = asking.args(["-t", "ppm", "-"]).output();

        Ok(match taken {
            Ok(done) => match done.status.success() {
                true => Some(done.stdout),
                false => None,
            },
            Err(_grim_is_not_here) => None,
        })
    }

    pub fn wait_for_a_still_screen(&self) -> Result<Outcome, Never> {
        let Ok(patience) = Schedule::asking_every(A_STILL_SCREEN, Duration::from_millis(100));
        let mut before: Option<Vec<u8>> = None;

        until_handed(patience, &mut before, |before| {
            let Ok(now) = self.shot();

            let now = match now {
                Some(now) => now,
                None => return Ok(Ready::NotYet),
            };

            let same = before.as_ref() == Some(&now);

            *before = Some(now);

            Ok(match same {
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        })
    }

    pub fn surfaces(&self) -> Result<BTreeSet<String>, Never> {
        let mut on = BTreeSet::new();
        let Ok(windows) = self.asking(Query::Clients);

        match windows {
            Some(console_compositor::Answer::Clients(clients)) => {
                on.extend(clients.into_iter().map(|client| client.address));
            }
            Some(_not_what_was_asked) => {},
            None => {},
        }

        let Ok(screens) = self.asking(Query::Layers);

        match screens {
            Some(console_compositor::Answer::Layers(surfaces)) => {
                on.extend(surfaces.into_iter().filter_map(|surface| surface.address));
            }
            Some(_not_what_was_asked) => {},
            None => {},
        }

        Ok(on)
    }

    pub fn wait_for_something(&self, was: &BTreeSet<String>) -> Result<Outcome, Never> {
        let Ok(patience) = Schedule::asking_every(SOMETHING, Duration::from_millis(100));
        let Ok(came) = until(patience, || {
            let Ok(surfaces) = self.surfaces();

            Ok(match surfaces.difference(was).next().is_some() {
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        });

        match came {
            Outcome::Happened => self.wait_for_a_still_screen(),
            Outcome::RanOut => Ok(Outcome::RanOut),
        }
    }

    pub fn show_a_window(&self) -> Result<(), Never> {
        let clients = match self.asking(Query::Clients) {
            Ok(Some(console_compositor::Answer::Clients(clients))) => clients,
            Ok(Some(_not_what_was_asked)) => return Ok(()),
            Ok(None) => return Ok(()),
        };

        let where_ = match clients.first() {
            Some(client) => client.workspace_named.clone(),
            None => return Ok(()),
        };

        let where_ = where_.as_str();

        let Ok(lua) = console_compositor::onto(where_, console_compositor::Carrying::None);
        let Ok(_focused) = self.told(Request::Dispatch, &lua);
        let Ok(patience) = Schedule::asking_every(A_GOODBYE, Duration::from_millis(50));
        let Ok(there) = until(patience, || {
            let Ok(now) = self.asking(Query::ActiveWorkspace);

            let named = match now {
                Some(console_compositor::Answer::ActiveWorkspace(front)) => {
                    front.map(|workspace| workspace.named)
                }
                Some(_not_what_was_asked) => return Ok(Ready::NotYet),
                None => return Ok(Ready::NotYet),
            };

            Ok(match named.as_deref() == Some(where_) {
                true => Ready::Yes,
                false => Ready::NotYet,
            })
        });

        match there {
            Outcome::Happened => {},
            Outcome::RanOut => eprintln!("console-desktop: {where_} never came to the front"),
        }

        let Ok(_still) = self.wait_for_a_still_screen();

        Ok(())
    }

    pub fn paint_the_background(&self, picture: &std::path::Path) -> Result<(), Never> {
        match picture.exists() {
            true => {},
            false => return Ok(()),
        }

        let Ok(patience) = Schedule::asking_every(A_PAINT, Duration::from_millis(250));
        let mut last = String::new();
        let Ok(painted) = until_handed(patience, &mut last, |last| {
            let Ok(mut asking) = self.command("awww");
            let told = asking
                .args(["img"])
                .arg(picture)
                .args(["--resize", "crop", "--transition-type", "none"])
                .output();

            Ok(match told {
                Ok(done) => match done.status.success() {
                    true => Ready::Yes,
                    false => {
                        *last = String::from_utf8_lossy(&done.stderr).trim().to_string();

                        Ready::NotYet
                    }
                },
                Err(_there_is_no_awww_here) => {
                    *last = "awww is not on this machine".to_string();

                    Ready::Yes
                }
            })
        });

        match painted {
            Outcome::Happened => {
                let Ok(_still) = self.wait_for_a_still_screen();
            }
            Outcome::RanOut => eprintln!("the wallpaper was never painted: {last}"),
        }

        Ok(())
    }

    pub fn stop_the_bar(&self) -> Result<(), Never> {
        let Ok(bars) = self.talking_to(console_onscreen::BAR);

        for bar in &bars {
            // SAFETY: a signal to a process of this session's own, by its pid.
            unsafe { libc::kill(*bar, libc::SIGTERM) };
        }

        let Ok(patience) = Schedule::asking_every(A_GOODBYE, Duration::from_millis(50));
        let Ok(_went) = until(patience, || {
            let left = bars.iter().any(|bar| {
                let Ok(there) = still_there(*bar);

                there == There::Yes
            });

            Ok(match left {
                true => Ready::NotYet,
                false => Ready::Yes,
            })
        });

        Ok(())
    }

    fn talking_to(&self, program: &str) -> Result<Vec<i32>, Never> {
        let socket = match self
            .environment
            .iter()
            .find(|(name, _)| name == "WAYLAND_DISPLAY")
        {
            Some(socket) => socket,
            None => return Ok(Vec::new()),
        };

        let wanted = format!("WAYLAND_DISPLAY={}", socket.1);

        let all = match std::fs::read_dir("/proc") {
            Ok(all) => all,
            Err(_fault) => return Ok(Vec::new()),
        };

        Ok(all
            .flatten()
            .filter_map(|found| {
                let at = found.path();

                let called = at.file_name()?;
                let said = called.to_str()?;

                let pid = match said.parse::<i32>() {
                    Ok(pid) => pid,
                    Err(_fault) => return None,
                };

                let named = match std::fs::read_to_string(at.join("comm")) {
                    Ok(named) => named,
                    Err(_fault) => return None,
                };


                match named.trim() == program {
                    true => {},
                    false => return None,
                }

                let held = match std::fs::read(at.join("environ")) {
                    Ok(held) => held,
                    Err(_fault) => return None,
                };

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

        let Ok(patience) = Schedule::asking_every(A_GOODBYE, Duration::from_millis(100));
        let Ok(_went) = until(patience, || {
            let Ok(mut asking) = self.command("awww");
            let still = asking.arg("query").output();

            Ok(match still.is_ok_and(|done| done.status.success()) {
                true => Ready::NotYet,
                false => Ready::Yes,
            })
        });

        Ok(())
    }
}

//! Connecting to the nested compositor, and looking at what it has.
//!
//! The wallpaper belongs to the caller, not the delayed session script. A
//! short check can finish before that script reaches the daemon, leaving
//! teardown asking a socket that does not exist yet and a daemon starting
//! just as its compositor closes. Starting it here makes its lifetime a
//! handle: it is bound to this process, and stopped and reaped before the
//! compositor. A socket disappearing is not proof that its process exited.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use console_compositor::{ActiveWorkspace, Clients, Layers, Monitors, Question, Request};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_operating_system_linux::program_lifetime::{BoundToParent, alongside};
use console_program_lifetime::Still;
use console_screen::Screen;
use console_waiting::{Ready, Schedule, until, until_handed};

use crate::{Process, nested, process_presence};

pub const A_SCREEN: Duration = Duration::from_secs(12);

pub const SOMETHING: Duration = Duration::from_secs(20);

pub const A_GOODBYE: Duration = Duration::from_secs(3);

pub const A_MONITOR: Duration = Duration::from_secs(5);

pub const A_STILL_SCREEN: Duration = Duration::from_secs(3);

pub const A_PAINT: Duration = Duration::from_secs(8);

const HEADLESS: &str = "HEADLESS-1";

const WINDOWED: &str = "WAYLAND-1";

pub use console_waiting::Outcome;

fn still_there(pid: i32) -> Result<Process, Never> {
  match rustix::process::Pid::from_raw(pid) {
    Some(process) => process_presence(process),
    None => Ok(Process::Gone),
  }
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
    let mut environment_2 = environment;
    environment_2.push(("WAYLAND_DISPLAY".to_string(), at.socket.to_string()));
    environment_2.push((
      "HYPRLAND_INSTANCE_SIGNATURE".to_string(),
      at.signature.to_string(),
    ));
    Ok(Inside {
      environment: environment_2,
    })
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
      Ok(done) => match std::str::from_utf8(&done.stdout) {
        Ok(text) => text.trim().to_string(),
        Err(fault) => {
          console_core_logging::warn!("console-desktop: hyprctl output is not UTF-8: {fault}");

          String::new()
        }
      },
      Err(fault) => {
        console_core_logging::warn!("console-desktop: hyprctl {}: {fault}", arguments.join(" "));

        String::new()
      }
    })
  }

  fn query<Q: Question>(&self, question: Q) -> Result<Option<Q::Reply>, Never> {
    let Ok(name) = Program::Hyprctl.name();
    let Ok(asking) = self.command(name);

    Ok(match console_compositor::ask_with(asking, question) {
      Ok(said) => Some(said),
      Err(why) => {
        eprintln!("console-desktop: {why}");

        None
      }
    })
  }

  fn request(
    &self,
    what: Request,
    lua: &str,
  ) -> Result<console_compositor::DispatchResult, Never> {
    let Ok(name) = Program::Hyprctl.name();
    let Ok(telling) = self.command(name);

    console_compositor::request_with(telling, what, lua)
  }

  pub fn wait_for_screen(&self, name: &str) -> Result<Outcome, Never> {
    let Ok(patience) = Schedule::asking_every(A_SCREEN, Duration::from_millis(250));

    until(patience, || {
      let Ok(mut asking) = self.command("grim");
      let taken = asking.args(["-o", name, "-"]).output();

      Ok(
        match ::core::primitive::bool::then_some(
          taken.is_ok_and(|done| done.status.success()),
          (),
        ) {
          ::core::option::Option::Some(()) => Ready::Yes,
          ::core::option::Option::None => Ready::NotYet,
        },
      )
    })
  }

  fn monitors(&self) -> Result<Vec<console_compositor::Monitor>, Never> {
    let Ok(said) = self.query(Monitors);

    Ok(match said {
      Some(monitors) => monitors,
      None => Vec::new(),
    })
  }

  fn monitor_names(&self) -> Result<BTreeSet<String>, Never> {
    let Ok(monitors) = self.monitors();

    Ok(monitors.into_iter().map(|monitor| monitor.named).collect())
  }

  fn wait_for_monitors(&self, named: &str, want: Ready) -> Result<Outcome, Never> {
    let Ok(patience) = Schedule::asking_every(A_MONITOR, Duration::from_millis(50));

    until(patience, || {
      let Ok(monitors) = self.monitor_names();

      Ok(
        match ::core::primitive::bool::then_some(
          monitors.contains(named) == (want == Ready::Yes),
          (),
        ) {
          ::core::option::Option::Some(()) => Ready::Yes,
          ::core::option::Option::None => Ready::NotYet,
        },
      )
    })
  }

  pub fn make_the_screen(&self, screen: &Screen) -> Result<Outcome, Never> {
    let Ok(_created) = self.hyprctl(&["output", "create", "headless"]);
    let Ok(arrived) = self.wait_for_monitors(HEADLESS, Ready::Yes);

    match arrived {
      Outcome::Happened => {}
      Outcome::RanOut => eprintln!("console-desktop: {HEADLESS} never appeared"),
    }

    let Ok(made) = nested::made_headless(screen);
    let Ok(_said) = self.request(Request::Script, &made);
    let Ok(sized) = self.wait_for_mode(screen);

    match sized {
      Outcome::Happened => {}
      Outcome::RanOut => {
        eprintln!("console-desktop: {HEADLESS} never took the mode it was given");
      }
    }

    let Ok(_disabled) = self.request(
      Request::Script,
      r#"hl.monitor({ output = "WAYLAND-1", disabled = true })"#,
    );
    let Ok(alone) = self.wait_for_monitors(WINDOWED, Ready::NotYet);

    match alone {
      Outcome::Happened => {}
      Outcome::RanOut => eprintln!("console-desktop: {WINDOWED} would not go away"),
    }

    Ok(match (arrived, sized, alone) {
      (Outcome::Happened, Outcome::Happened, Outcome::Happened) => Outcome::Happened,
      (
        Outcome::RanOut,
        Outcome::Happened | Outcome::RanOut,
        Outcome::Happened | Outcome::RanOut,
      )
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
        monitor.named == HEADLESS && monitor.size == Some((i64::from(wide), i64::from(tall)))
      });

      Ok(match ::core::primitive::bool::then_some(took, ()) {
        ::core::option::Option::Some(()) => Ready::Yes,
        ::core::option::Option::None => Ready::NotYet,
      })
    })
  }

  fn shot(&self) -> Result<Option<Vec<u8>>, Never> {
    let Ok(mut asking) = self.command("grim");
    let taken = asking.args(["-t", "ppm", "-"]).output();

    Ok(match taken {
      Ok(done) => match ::core::primitive::bool::then_some(done.status.success(), ()) {
        ::core::option::Option::Some(()) => Some(done.stdout),
        ::core::option::Option::None => None,
      },
      Err(_grim_is_not_here) => None,
    })
  }

  pub fn wait_for_a_still_screen(&self) -> Result<Outcome, Never> {
    let Ok(patience) = Schedule::asking_every(A_STILL_SCREEN, Duration::from_millis(100));
    let mut before: Option<Vec<u8>> = None;

    until_handed(patience, &mut before, |before_2| {
      let Ok(now) = self.shot();

      let now_3 = match now {
        Some(now_2) => now_2,
        None => return Ok(Ready::NotYet),
      };

      let same = before_2.as_ref() == Some(&now_3);

      *before_2 = Some(now_3);

      Ok(match ::core::primitive::bool::then_some(same, ()) {
        ::core::option::Option::Some(()) => Ready::Yes,
        ::core::option::Option::None => Ready::NotYet,
      })
    })
  }

  pub fn surfaces(&self) -> Result<BTreeSet<String>, Never> {
    let mut on = BTreeSet::new();
    let Ok(windows) = self.query(Clients);

    match windows {
      Some(clients) => {
        on.extend(clients.into_iter().map(|client| client.address));
      }
      None => {}
    }

    let Ok(screens) = self.query(Layers);

    match screens {
      Some(surfaces) => {
        on.extend(surfaces.into_iter().filter_map(|surface| surface.address));
      }
      None => {}
    }

    Ok(on)
  }

  pub fn wait_for_something(&self, was: &BTreeSet<String>) -> Result<Outcome, Never> {
    let Ok(patience) = Schedule::asking_every(SOMETHING, Duration::from_millis(100));
    let Ok(came) = until(patience, || {
      let Ok(surfaces) = self.surfaces();

      Ok(
        match ::core::primitive::bool::then_some(surfaces.difference(was).next().is_some(), ()) {
          ::core::option::Option::Some(()) => Ready::Yes,
          ::core::option::Option::None => Ready::NotYet,
        },
      )
    });

    match came {
      Outcome::Happened => self.wait_for_a_still_screen(),
      Outcome::RanOut => Ok(Outcome::RanOut),
    }
  }

  pub fn show_a_window(&self) -> Result<(), Never> {
    let clients = match self.query(Clients) {
      Ok(Some(clients)) => clients,
      Ok(None) => return Ok(()),
    };

    let where_ = match clients.first() {
      Some(client) => client.workspace_named.clone(),
      None => return Ok(()),
    };

    let workspace = where_.as_str();

    let Ok(lua) = console_compositor::onto(workspace, console_compositor::Carrying::None);
    let Ok(_focused) = self.request(Request::Dispatch, &lua);
    let Ok(patience) = Schedule::asking_every(A_GOODBYE, Duration::from_millis(50));
    let Ok(there) = until(patience, || {
      let Ok(now) = self.query(ActiveWorkspace);

      let named = match now {
        Some(front) => front.map(|workspace| workspace.named),
        None => return Ok(Ready::NotYet),
      };

      Ok(
        match ::core::primitive::bool::then_some(named.as_deref() == Some(workspace), ()) {
          ::core::option::Option::Some(()) => Ready::Yes,
          ::core::option::Option::None => Ready::NotYet,
        },
      )
    });

    match there {
      Outcome::Happened => {}
      Outcome::RanOut => eprintln!("console-desktop: {workspace} never came to the front"),
    }

    let Ok(_still) = self.wait_for_a_still_screen();

    Ok(())
  }

  pub fn focus_a_window(&self) -> Result<(), Never> {
    let Ok(windows) = self.query(Clients);
    if let Some(window) = windows.as_ref().and_then(|windows| windows.first()) {
      let Ok(target) = console_compositor::quote(&format!("address:{}", window.address));
      let Ok(_focused) = self.request(
        Request::Dispatch,
        &format!("hl.dsp.focus({{window = {target}}})"),
      );
    }

    Ok(())
  }

  pub fn paint_the_background(&self, picture: &std::path::Path) -> Result<(), Never> {
    match crate::open_regular(picture) {
      Ok((_file, _metadata)) => {}
      Err(_unavailable) => return Ok(()),
    }

    let Ok(patience) = Schedule::asking_every(A_PAINT, Duration::from_millis(250));
    let mut last = String::new();
    let Ok(painted) = until_handed(patience, &mut last, |last_2| {
      let Ok(mut asking) = self.command("awww");
      let told = asking
        .args(["img"])
        .arg(picture)
        .args(["--resize", "crop", "--transition-type", "none"])
        .output();

      Ok(match told {
        Ok(done) => match ::core::primitive::bool::then_some(done.status.success(), ()) {
          ::core::option::Option::Some(()) => Ready::Yes,
          ::core::option::Option::None => {
            *last_2 = match std::str::from_utf8(&done.stderr) {
              Ok(text) => text.trim().to_string(),
              Err(fault) => format!("awww's error output is not UTF-8: {fault}"),
            };

            Ready::NotYet
          }
        },
        Err(_there_is_no_awww_here) => {
          *last_2 = "awww is not on this machine".to_string();

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
      let Ok(()) = console_operating_system_linux::program_lifetime::signal(
        *bar,
        rustix::process::Signal::TERM,
      );
    }

    let Ok(patience) = Schedule::asking_every(A_GOODBYE, Duration::from_millis(50));
    let Ok(_went) = until(patience, || {
      let left = bars.iter().any(|bar| {
        let Ok(there) = still_there(*bar);

        there == Process::Present
      });

      Ok(match ::core::primitive::bool::then_some(left, ()) {
        ::core::option::Option::Some(()) => Ready::NotYet,
        ::core::option::Option::None => Ready::Yes,
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
      Err(_unreadable) => return Ok(Vec::new()),
    };

    Ok(
      all
        .flatten()
        .filter_map(|found| {
          let at = found.path();

          let called = at.file_name()?;
          let said = called.to_str()?;

          let pid = match said.parse::<i32>() {
            Ok(pid) => pid,
            Err(_not_a_pid) => return None,
          };

          let named = match std::fs::read_to_string(at.join("comm")) {
            Ok(named) => named,
            Err(_unreadable) => return None,
          };

          match ::core::primitive::bool::then_some(named.trim() == program, ()) {
            ::core::option::Option::Some(()) => {}
            ::core::option::Option::None => return None,
          }

          let held = match std::fs::read(at.join("environ")) {
            Ok(held) => held,
            Err(_unreadable) => return None,
          };

          held
            .split(|byte| *byte == 0)
            .any(|said_2| said_2 == wanted.as_bytes())
            .then_some(pid)
        })
        .collect(),
    )
  }

  pub fn start_the_wallpaper(&self, palette: &Path) -> Result<Option<BoundToParent>, Never> {
    let Ok(program) = Program::AwwwDaemon.name();
    let Ok(mut asking) = self.command(program);
    asking.arg("--no-cache");

    let daemon = match alongside(&mut asking) {
      Ok(daemon) => daemon,
      Err(fault) => {
        eprintln!("console-desktop: starting the wallpaper: {fault}");

        return Ok(None);
      }
    };

    let Ok(program_2) = Program::Sh.name();
    let Ok(client) = Program::Awww.name();
    let Ok(mut clearing) = self.command(program_2);
    clearing.args([
      "-c",
      r#". "$1" && exec "$2" clear "$night""#,
      "console-desktop",
    ]);
    clearing.arg(palette).arg(client);

    let mut starting = (daemon, clearing);
    let Ok(patience) = Schedule::asking_every(A_PAINT, Duration::from_millis(100));
    let Ok(painted) = until_handed(patience, &mut starting, |(daemon_2, clearing_2)| {
      let Ok(still) = daemon_2.still();

      Ok(match still {
        Still::Ended => Ready::Yes,
        Still::Running => match ::core::primitive::bool::then_some(
          clearing_2.output().is_ok_and(|done| done.status.success()),
          (),
        ) {
          ::core::option::Option::Some(()) => Ready::Yes,
          ::core::option::Option::None => Ready::NotYet,
        },
      })
    });

    match painted {
      Outcome::Happened => {}
      Outcome::RanOut => eprintln!("console-desktop: the wallpaper never painted its ground"),
    }

    Ok(Some(starting.0))
  }

  pub fn stop_the_wallpaper(&self, wallpaper: Option<BoundToParent>) -> Result<(), Never> {
    let mut daemon = match wallpaper {
      Some(daemon) => daemon,
      None => return Ok(()),
    };
    let Ok(still) = daemon.still();

    match still {
      Still::Running => {}
      Still::Ended => return Ok(()),
    }

    let Ok(program) = Program::Awww.name();
    let Ok(mut asking) = self.command(program);
    let _told = asking.arg("kill").output();

    let Ok(patience) = Schedule::asking_every(A_GOODBYE, Duration::from_millis(50));
    let Ok(_went) = until_handed(patience, &mut daemon, |daemon_2| {
      let Ok(still_2) = daemon_2.still();

      Ok(match still_2 {
        Still::Running => Ready::NotYet,
        Still::Ended => Ready::Yes,
      })
    });

    drop(daemon);

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::os::unix::fs::PermissionsExt;

  type Failure = Box<dyn std::error::Error>;

  fn executable(at: &Path, script: &str) -> Result<(), Failure> {
    console_core_atomic_writes::whole_with_permissions(
      at,
      script.as_bytes(),
      std::fs::Permissions::from_mode(0o755),
    )?;

    Ok(())
  }

  #[test]
  fn the_wallpaper_process_exits_before_its_compositor_can_be_stopped() -> Result<(), Failure> {
    let here = console_core_temporary_directories::fresh("wallpaper-exit")?;
    executable(
      &here.join("awww-daemon"),
      r#"#!/bin/sh
[ "$1" = --no-cache ] || exit 1
printf '%s' "$WAYLAND_DISPLAY" > "$HOME/display"
while [ ! -f "$HOME/stop" ]; do sleep 0.01; done
sleep 0.2
printf done > "$HOME/exited"
"#,
    )?;
    executable(
      &here.join("awww"),
      r#"#!/bin/sh
case "$1" in
    clear) printf '%s' "$2" > "$HOME/ground" ;;
    kill) touch "$HOME/stop" ;;
    query) exit 1 ;;
    *) exit 1 ;;
esac
"#,
    )?;
    let palette = here.join("palette.sh");
    console_core_atomic_writes::whole(&palette, b"night=ground-from-test\n")?;
    let Ok(inside) = Inside::new(
      vec![
        (
          "PATH".to_string(),
          format!("{}:/usr/bin:/bin", here.display()),
        ),
        ("HOME".to_string(), here.display().to_string()),
      ],
      Instance {
        socket: "wayland-wallpaper-test",
        signature: "wallpaper-test",
      },
    );

    let wallpaper = inside.start_the_wallpaper(&palette)?;
    let daemon = wallpaper.ok_or("the wallpaper was not started")?;
    let pid = daemon.id()?;
    let Ok(raw_pid) = console_core_number_conversion::fitted::<_, i32>(pid);
    inside.stop_the_wallpaper(Some(daemon))?;
    let display = std::fs::read_to_string(here.join("display"))?;
    let ground = std::fs::read_to_string(here.join("ground"))?;
    let exited = std::fs::read_to_string(here.join("exited"))?;

    assert_eq!(display, "wayland-wallpaper-test");
    assert_eq!(ground, "ground-from-test");
    assert_eq!(exited, "done");
    assert_eq!(still_there(raw_pid), Ok(Process::Gone));

    Ok(())
  }

  #[test]
  fn a_session_without_a_wallpaper_never_sends_a_kill_to_another_daemon() -> Result<(), Failure> {
    let here = console_core_temporary_directories::fresh("wallpaper-absent")?;
    executable(
      &here.join("awww"),
      "#!/bin/sh\nprintf called > \"$HOME/called\"\n",
    )?;
    let Ok(inside) = Inside::new(
      vec![
        ("PATH".to_string(), here.display().to_string()),
        ("HOME".to_string(), here.display().to_string()),
      ],
      Instance {
        socket: "wayland-wallpaper-test",
        signature: "wallpaper-test",
      },
    );

    inside.stop_the_wallpaper(None)?;

    match std::fs::read(here.join("called")) {
      Ok(_) => return Err(Box::from("a wallpaper daemon was unexpectedly called")),
      Err(fault) => assert_eq!(fault.kind(), std::io::ErrorKind::NotFound),
    }

    Ok(())
  }
}

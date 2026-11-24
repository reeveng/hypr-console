//! Exercise the commands and the default watcher over the compositor socket.
//! The fixture finishes its last reply before its client is stopped.

use std::error::Error;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Mutex, MutexGuard, TryLockError};
use std::thread;
use std::time::Duration;

use console_compositor::socket::{self, Socket};
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::index;
use console_waiting::{Schedule, clock::LiveClock};
use serde_json::{Value, json};

const SIGNATURE: &str = "display-test";
const SAVED_POSITION: i64 = -1920;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layout {
  Extended,
  Mirrored,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Display {
  connector: &'static str,
  id: i64,
  x: i64,
  layout: Layout,
}

struct Desktop {
  external: Option<Display>,
  workspace_output: &'static str,
  applied: Vec<String>,
  client_reads: u32,
}

struct Monitor {
  display: Display,
  serial: &'static str,
  active: i64,
}

impl Monitor {
  fn json(self) -> Result<Value, Never> {
    let Display {
      connector,
      id,
      x,
      layout,
    } = self.display;
    let (mirror, workspace, name) = match layout {
      Layout::Extended => ("none", self.active, self.active.to_string()),
      Layout::Mirrored => ("0", -1, String::new()),
    };

    Ok(json!({
      "name": connector, "id": id, "make": "Fixture", "model": "Display",
      "serial": self.serial, "width": 1920, "height": 1080, "refreshRate": 60,
      "x": x, "y": 0, "scale": 1, "transform": 0, "mirrorOf": mirror,
      "activeWorkspace": { "id": workspace, "name": name },
      "focused": connector == "eDP-1"
    }))
  }
}

impl Desktop {
  fn reply(&mut self, question: &str) -> Result<String, Never> {
    match question {
      "j/monitors all" => self.monitors(),
      "j/workspaces" => Ok(
        json!([
          { "id": 2, "name": "2", "monitor": "eDP-1", "windows": 0 },
          { "id": 7, "name": "7", "monitor": self.workspace_output, "windows": 1 }
        ])
        .to_string(),
      ),
      "j/clients" => {
        self.client_reads = self.client_reads.saturating_add(1);
        Ok("[]".to_owned())
      }
      message => match message.split_once(' ') {
        Some(("/eval", script)) => {
          let Ok(()) = self.apply(script);
          self.applied.push(message.to_owned());
          Ok("ok".to_owned())
        }
        Some((_, _)) | None => Ok(format!("error: unexpected request {message}")),
      },
    }
  }

  fn monitors(&self) -> Result<String, Never> {
    let Ok(panel) = (Monitor {
      display: Display {
        connector: "eDP-1",
        id: 0,
        x: 0,
        layout: Layout::Extended,
      },
      serial: "panel",
      active: 2,
    })
    .json();
    let mut monitors = vec![panel];

    match self.external {
      Some(display) => {
        let active = match (self.workspace_output == display.connector).then_some(()) {
          Some(()) => 7,
          None => 4,
        };
        let Ok(dock) = (Monitor {
          display,
          serial: "external",
          active,
        })
        .json();
        monitors.push(dock);
      }
      None => {}
    }

    Ok(json!(monitors).to_string())
  }

  fn apply(&mut self, script: &str) -> Result<(), Never> {
    match (&mut self.external, script.contains("hl.monitor(")) {
      (Some(display), true) => match (
        script.contains(r#"mirror = "eDP-1""#),
        script.contains(r#"mirror = """#),
      ) {
        (true, true | false) => {
          display.layout = Layout::Mirrored;
          self.workspace_output = "eDP-1";
        }
        (false, true) => {
          display.layout = Layout::Extended;
          display.x = SAVED_POSITION;
        }
        (false, false) => {}
      },
      (Some(_), false) | (None, true | false) => {}
    }

    match (self.external, script.contains("hl.dsp.workspace.move(")) {
      (Some(display), true) => self.workspace_output = display.connector,
      (Some(_), false) | (None, true | false) => {}
    }

    Ok(())
  }
}

fn serve(
  listener: &UnixListener,
  desktop: &Mutex<Desktop>,
  stop: &Receiver<()>,
) -> io::Result<()> {
  let delay = Duration::from_millis(10);

  iterate((), |()| {
    Ok(match stop.recv_timeout(delay) {
      Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => Step::Halt(Ok(())),
      Err(mpsc::RecvTimeoutError::Timeout) => match listener.accept() {
        Ok((mut connection, _)) => match reply(&mut connection, desktop) {
          Ok(()) => Step::Again(()),
          Err(fault) => Step::Halt(Err(fault)),
        },
        Err(fault) => match (fault.kind() == io::ErrorKind::WouldBlock).then_some(()) {
          Some(()) => Step::Again(()),
          None => Step::Halt(Err(fault)),
        },
      },
    })
  })
  .map_err(io::Error::other)?
}

fn reply(connection: &mut UnixStream, desktop: &Mutex<Desktop>) -> io::Result<()> {
  let deadline = Some(Duration::from_secs(1));
  connection.set_read_timeout(deadline)?;
  connection.set_write_timeout(deadline)?;
  let mut bytes = [0; 4096];
  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the fixture sets a read deadline on this stream"
    )
  )]
  let received = connection
    .read(&mut bytes)
    .and_then(|count| u32::try_from(count).map_err(io::Error::other))?;
  let Ok(end) = index(received);
  let packet = bytes
    .get(..end)
    .ok_or_else(|| io::Error::other("request exceeds the fixture buffer"))?;
  let message = std::str::from_utf8(packet).map_err(io::Error::other)?;
  let response = {
    let mut locked_desktop = lock_desktop(desktop)?;
    let Ok(answer) = locked_desktop.reply(message);
    answer
  };

  connection.write_all(response.as_bytes())
}

fn resume(at: &Path) -> Result<Command, Never> {
  let mut command = Command::new(env!("CARGO_BIN_EXE_console-resume"));
  command
    .env("CONSOLE_RESUME_PATH", at.join("sessions"))
    .env("XDG_RUNTIME_DIR", at)
    .env(console_compositor::session::SIGNATURE, SIGNATURE);

  Ok(command)
}

struct Watching(Child);

impl Drop for Watching {
  fn drop(&mut self) {
    match self.0.kill() {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::warn!("stopping the display watcher: {fault}");
      }
    }

    match self.0.wait() {
      Ok(_status) => {}
      Err(fault) => {
        console_core_logging::warn!("reaping the display watcher: {fault}");
      }
    }
  }
}

fn wait_for<T>(
  missing: &'static str,
  mut probe: impl FnMut() -> io::Result<Option<T>>,
) -> io::Result<T> {
  let Ok(patience) = Schedule::asking_every(Duration::from_secs(8), Duration::from_millis(20));
  let Ok(mut clock) = LiveClock::started();
  let found =
    console_waiting::until_some_handed_on_result(&mut clock, patience, &mut probe, |ask| ask())?;

  found.ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, missing))
}

fn eventually(ready: impl Fn() -> bool) -> io::Result<()> {
  wait_for("the display setup did not settle", || {
    Ok(ready().then_some(()))
  })
}

fn lock_desktop(desktop: &Mutex<Desktop>) -> io::Result<MutexGuard<'_, Desktop>> {
  wait_for("the fixture desktop stayed locked", || {
    match desktop.try_lock() {
      Ok(locked) => Ok(Some(locked)),
      Err(TryLockError::WouldBlock) => Ok(None),
      Err(TryLockError::Poisoned(_)) => Err(io::Error::other("desktop lock poisoned")),
    }
  })
}

fn checkpoint_displays_and_windows(at: &Path, desktop: &Mutex<Desktop>) -> io::Result<()> {
  let Ok(mut simulated) = resume(at);
  let said = simulated
    .args(["checkpoint", "--simulate", "ending"])
    .output()?;
  assert!(said.status.success(), "{said:?}");
  let setups = at.join("sessions/ending/setups.json");
  assert!(matches!(fs::read(&setups), Err(fault) if fault.kind() == io::ErrorKind::NotFound));

  let Ok(mut checkpoint) = resume(at);
  let saved = checkpoint
    .args(["checkpoint", "ending"])
    .env_remove("DBUS_SESSION_BUS_ADDRESS")
    .output()?;
  assert!(saved.status.success(), "{saved:?}");
  let profiles = fs::read(setups)?;
  let currents = lock_desktop(desktop)?;
  assert!(!profiles.is_empty());
  assert_eq!(currents.client_reads, 1);

  Ok(())
}

fn recall_from_a_different_connector(at: &Path, desktop: &Mutex<Desktop>) -> io::Result<()> {
  let Ok(mut save) = resume(at);
  let saved = save.arg("save").output()?;
  assert!(saved.status.success(), "{saved:?}");

  {
    let mut current = lock_desktop(desktop)?;
    current.external = Some(Display {
      connector: "DP-3",
      id: 9,
      x: 1920,
      layout: Layout::Extended,
    });
    current.workspace_output = "eDP-1";
  }

  let Ok(mut recall) = resume(at);
  let recalled = recall.arg("recall").output()?;
  assert!(recalled.status.success(), "{recalled:?}");

  {
    let mut current = lock_desktop(desktop)?;
    assert_eq!(current.workspace_output, "DP-3");
    assert!(current.applied.iter().any(|script| {
      script.contains(r#"output = "DP-3""#) && script.contains(r#"position = "-1920x0""#)
    }));
    assert!(
      current
        .applied
        .iter()
        .any(|script| { script.contains(r#"workspace = "7", monitor = "DP-3""#) })
    );
    current.external = None;
    current.workspace_output = "eDP-1";
    current.applied.clear();
  }

  Ok(())
}

fn start_watcher(at: &Path) -> io::Result<Watching> {
  let Ok(mut resume) = resume(at);
  let child = resume
    .args(["default", "--save-interval", "1"])
    .stdout(Stdio::null())
    .spawn()?;

  Ok(Watching(child))
}

fn watch_from_an_undocked_login(setups: &Path, desktop: &Mutex<Desktop>) -> io::Result<()> {
  eventually(|| {
    fs::read(setups).is_ok_and(|bytes| {
      serde_json::from_slice::<Vec<Value>>(&bytes).is_ok_and(|profiles| profiles.len() == 2)
    })
  })?;

  {
    let mut current = lock_desktop(desktop)?;
    current.external = Some(Display {
      connector: "DP-3",
      id: 9,
      x: 1920,
      layout: Layout::Mirrored,
    });
  }

  eventually(|| {
    desktop.try_lock().is_ok_and(|current| {
      current.external
        == Some(Display {
          connector: "DP-3",
          id: 9,
          x: SAVED_POSITION,
          layout: Layout::Extended,
        })
        && current.workspace_output == "DP-3"
    })
  })
}

fn wait_for_edit_lock(at: &Path) -> io::Result<fs::File> {
  #[cfg_attr(
    dylint_lib = "explicit040_no_torn_write",
    expect(
      explicit040_no_torn_write,
      reason = "this file handle is used only to take a lock"
    )
  )]
  let editing = fs::OpenOptions::new()
    .write(true)
    .open(at.join("console-display.lock"))?;

  wait_for(
    "the fixture could not obtain the display lock",
    || match editing.try_lock() {
      Ok(()) => Ok(Some(())),
      Err(fs::TryLockError::WouldBlock) => Ok(None),
      Err(fs::TryLockError::Error(fault)) => Err(fault),
    },
  )?;

  Ok(editing)
}

fn persist_an_edit_after_unlocking(
  instance: &Path,
  setups: &Path,
  desktop: &Mutex<Desktop>,
) -> io::Result<()> {
  let editing = wait_for_edit_lock(instance)?;
  let reads = {
    let mut current = lock_desktop(desktop)?;
    let display = current
      .external
      .as_mut()
      .ok_or_else(|| io::Error::other("display is missing"))?;
    display.layout = Layout::Mirrored;
    current.workspace_output = "eDP-1";
    current.client_reads
  };

  eventually(|| {
    desktop
      .try_lock()
      .is_ok_and(|current| current.client_reads >= reads.saturating_add(2))
  })?;
  let locked_setup = fs::read_to_string(setups)?;
  assert!(!locked_setup.contains(r#""mirror":{"#));
  drop(editing);

  eventually(|| fs::read_to_string(setups).is_ok_and(|text| text.contains(r#""mirror":{"#)))?;

  {
    let mut current = lock_desktop(desktop)?;
    current.external = Some(Display {
      connector: "DP-4",
      id: 8,
      x: 1920,
      layout: Layout::Extended,
    });
    current.workspace_output = "DP-4";
  }

  eventually(|| {
    desktop.try_lock().is_ok_and(|current| {
      current
        .external
        .is_some_and(|display| display.layout == Layout::Mirrored)
        && current.workspace_output == "eDP-1"
    })
  })
}

#[test]
fn saved_displays_and_workspaces_are_recalled_manually_and_after_an_undocked_login()
-> Result<(), Box<dyn Error>> {
  let at = console_core_temporary_directories::fresh("resume-dock")?;
  let instance = socket::instance_directory(&at, SIGNATURE)?;
  fs::create_dir_all(&instance)?;
  let listener = UnixListener::bind(instance.join(Socket::Requests))?;
  listener.set_nonblocking(true)?;
  let desktop = Mutex::new(Desktop {
    external: Some(Display {
      connector: "DP-1",
      id: 1,
      x: SAVED_POSITION,
      layout: Layout::Extended,
    }),
    workspace_output: "DP-1",
    applied: Vec::new(),
    client_reads: 0,
  });

  thread::scope(|scope| -> Result<(), Box<dyn Error>> {
    let (send, stop) = mpsc::sync_channel(0);
    let socket = &listener;
    let state = &desktop;
    let server = scope.spawn(move || serve(socket, state, &stop));
    let started = checkpoint_displays_and_windows(&at, &desktop)
      .and_then(|()| recall_from_a_different_connector(&at, &desktop))
      .and_then(|()| start_watcher(&at));
    let (checked, watcher) = match started {
      Ok(watching) => {
        let setups = at.join("sessions/default/setups.json");
        let checked = watch_from_an_undocked_login(&setups, &desktop)
          .and_then(|()| persist_an_edit_after_unlocking(&instance, &setups, &desktop));
        (checked, Some(watching))
      }
      Err(fault) => (Err(fault), None),
    };

    drop(send);
    eventually(|| server.is_finished())?;
    #[cfg_attr(
      dylint_lib = "explicit057_no_unbounded_wait",
      expect(
        explicit057_no_unbounded_wait,
        reason = "the bounded wait above checks that the server has finished"
      )
    )]
    let served = server.join().map_err(|_| "compositor fixture panicked")?;
    served?;
    drop(watcher);
    checked?;
    Ok(())
  })?;
  fs::remove_dir_all(at)?;

  Ok(())
}

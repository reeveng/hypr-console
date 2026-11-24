//! What was open, written down, and put back.
//!
//! Two files per session, because they answer two different questions. `exec.conf`
//! is the list of commands that start the programs again, one line each, with the
//! rules the window wants in front of it. `clients.json` is what each window was
//! like once it was there -- which workspace, where, how big, floating or pinned
//! or filling the screen -- and it is read as windows appear, because a program
//! that has just been started has no window yet to move.
//!
//! ## A desktop with nothing on it is not a session
//!
//! Logging out closes every window a second or two before the compositor stops,
//! and a saver watching that sees an empty desktop and writes it down. What was
//! on the screen all day is then gone, and nothing looks wrong: the file is
//! there, it is valid, it says nothing was open. So a save of nothing is refused
//! and the session already on disk stands. Throwing one away is `delete`, which
//! is a thing someone asks for.
//!
//! A window closing is the same fault a minute earlier. It is written down, but
//! only after [`LOSS_SETTLES_AFTER`], because the first thing a log-out looks
//! like is one window closing.
//!
//! ## Every relaunched window has a distinct identity
//!
//! The compositor attaches a tag to windows launched by this restore. Older
//! snapshots and windows opened by a shared process fall back to their class,
//! title and workspace.
//!
//! ## Closing a window is asked of the compositor
//!
//! The fork this came from cleared the screen by sending each window's process a
//! signal. That is one program deciding to end another one on a desktop where the
//! compositor owns windows, and it takes a whole program down when one of its
//! windows was on the screen. `hl.dsp.window.close` is the same request made to
//! the thing that can refuse it, and it is what the close button asks for.
//!
//! ## A special workspace is known by its name and not by its number
//!
//! Hyprland numbers them from -99 downwards, one further down for every one
//! that exists, so the second special workspace anyone makes is -98 and a
//! program that knows only -99 writes that number down as though it were a
//! workspace someone could ask for. `workspace -98 silent` names nothing, and
//! the window comes back wherever a rule the compositor cannot read leaves it.
//! What survives is the name -- `special:sky` is what it was called and what
//! asks for it back -- so the number decides nothing here. The two places that
//! ask spell the answer differently: a rule prefix takes the name as a word and
//! a dispatcher takes it quoted, which is why they are one question and two
//! sentences.
//!
//! ## Where a window goes back is the layout's, unless it floats
//!
//! A tiled window has no place of its own. The layout puts it where the windows
//! around it leave room, and pixels asked for over that are refused or held
//! until the next window disagrees with them. What decides where a tiled window
//! lands is the layout. For a two-pane Dwindle split its axis and ratio can
//! be inferred from the bounds. A corner and a size are asked of a window that
//! floats -- and of a floating window both are asked, because a window that
//! comes back the wrong size is the same fault as one that comes back in the
//! wrong place, and the rule prefix meant to have settled it is applied as the
//! window maps rather than after the program has decided what it wanted.
//!
//! ## A maximized window comes back sharing its place
//!
//! Nothing on this desktop maximizes any more: a window alone on a place fills
//! it because the layout gives it the whole of it, and one maximized over a
//! place it shares hides whatever else is there. Sessions written down while
//! every window was maximized still say so, and putting that back left a
//! browser covering the windows carried onto its place after it. So maximized
//! is written down as it was and put back as tiled. Filling the screen is not
//! the same request -- a film asked for it -- and it still comes back.
//!
//! ## A program with no window is not in a session
//!
//! What is written down is what the compositor can be asked about, and a
//! program that sits in the tray or waits on a socket has no window to be asked
//! about. That is not a gap here: what this device runs without a window is
//! what `desktop.conf` starts under `[services]`, and it is running again
//! before anyone has logged in. A session is the windows.
//!
//! ## `Saved` spells booleans because JSON does
//!
//! Everywhere else in this desktop what a window is doing is a name --
//! [`console_compositor::Floating`] and its siblings. `Saved` is not that; it is
//! the shape of a file, and the file is JSON, whose vocabulary for these is
//! `true` and `false`. So the names are turned into the file's words on the way
//! out and read back into names on the way in, and no caller sees a bare boolean.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{create_dir_all, remove_dir_all};
use std::path::{Path, PathBuf};

use std::time::{Duration, Instant};

use console_compositor::events::CompositorEvent;
use console_compositor::{
  Coordinates, DispatchResult, Filling, Floating, FullscreenMode, Operation, Pinned, Window,
  WindowAction, WindowChange,
};
use console_core_atomic_writes::Stored;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_state_machine::Machine;
use console_events::subscription::Received;
use console_program_contract::EventGroup;
use console_response_times::{Wait, Waiting};
use console_waiting::{Outcome, Ready, Schedule};

use crate::ending::{Coordination, ShutdownStatus};
use crate::starting::what_starts_it;
use crate::{LocalName, Unresumed};

mod restoration;
mod setup;
mod tiling;

use restoration::{Effect as RestoreEffect, Observed, Restoration, Targets};

const NO_SCREEN_NAMED: i64 = 0;

const EXEC_NAME: &str = "exec.conf";
const CLIENTS_NAME: &str = "clients.json";

const TICK: Duration = Duration::from_secs(1);
const WINDOW_POLL: Duration = Duration::from_millis(250);

const LOSS_SETTLES_AFTER: Duration = Duration::from_secs(15);

const CLOSING_SETTLES: Duration = Duration::from_secs(10);

const EVENTS_PER_CHECK: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Really {
  Truly,
  Simulated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restoring {
  MovingWhatIsOpen,
  StartingItAgain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duplicates {
  OnePerProgram,
  OnePerWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restore {
  Windows,
  NothingSaved(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Worth {
  Saving,
  Waiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Differs {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placing {
  ByPixels,
  ByTheLayout,
}

pub struct Sessions {
  pub at: PathBuf,
  pub adjusting_for: Duration,
  pub really: Really,
  pub restoring: Restoring,
  pub duplicates: Duplicates,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(transparent)]
struct SavedText(String);

impl std::ops::Deref for SavedText {
  type Target = str;

  fn deref(&self) -> &str {
    &self.0
  }
}

impl<'de> serde::Deserialize<'de> for SavedText {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let text = <String as serde::Deserialize>::deserialize(deserializer)?;
    let Ok(length) = fitted::<_, u64>(text.len());

    match ::core::primitive::bool::then_some(
      length <= u64::from(console_core_atomic_writes::MAX_TEXT_BYTES),
      (),
    ) {
      ::core::option::Option::Some(()) => Ok(Self(text)),
      ::core::option::Option::None => Err(serde::de::Error::custom(
        "text exceeds the session file limit",
      )),
    }
  }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "this is what hyprctl says about one window, written to disk as it was read; `floating`, `pinned` and a monitor are three separate facts the compositor keeps separately, and a shape of our own here would be a second reading of someone else's format"
  )
)]
struct Saved {
  address: SavedText,
  title: SavedText,
  first_class: SavedText,
  first_title: SavedText,
  workspace: i64,
  workspace_named: SavedText,
  monitor: Option<i64>,
  floating: bool,
  pinned: bool,
  fullscreen: i64,
  at: (i64, i64),
  size: (i64, i64),
  pid: i64,
  #[serde(default)]
  command: Option<String>,
  #[serde(default)]
  focused: bool,
}

fn open_windows() -> Result<Vec<Window>, Unresumed> {
  let open = console_compositor::ask(console_compositor::Clients)?;

  Ok(open)
}

fn to_saved(window: &Window) -> Result<Saved, Never> {
  Ok(Saved {
    address: SavedText(window.address.clone()),
    title: SavedText(window.title.clone()),
    first_class: SavedText(window.first_class.clone()),
    first_title: SavedText(window.first_title.clone()),
    workspace: window.workspace,
    workspace_named: SavedText(window.workspace_named.clone()),
    monitor: window.monitor,
    floating: window.floating == Floating::Yes,
    pinned: window.pinned == Pinned::Yes,
    fullscreen: match window.filling {
      Filling::None => 0,
      Filling::Maximized => 1,
      Filling::Screen => 2,
    },
    at: window.at,
    size: window.size,
    pid: window.pid,
    command: None,
    focused: false,
  })
}

fn as_window(saved: &Saved) -> Result<Window, Never> {
  Ok(Window {
    address: saved.address.0.clone(),
    title: saved.title.0.clone(),
    first_class: saved.first_class.0.clone(),
    first_title: saved.first_title.0.clone(),
    workspace: saved.workspace,
    workspace_named: saved.workspace_named.0.clone(),
    monitor: saved.monitor,
    floating: match ::core::primitive::bool::then_some(saved.floating, ()) {
      ::core::option::Option::Some(()) => Floating::Yes,
      ::core::option::Option::None => Floating::No,
    },
    pinned: match ::core::primitive::bool::then_some(saved.pinned, ()) {
      ::core::option::Option::Some(()) => Pinned::Yes,
      ::core::option::Option::None => Pinned::No,
    },
    filling: match saved.fullscreen {
      1 => Filling::Maximized,
      2 => Filling::Screen,
      _filling_nothing => Filling::None,
    },
    at: saved.at,
    size: saved.size,
    pid: saved.pid,
  })
}

#[derive(Default)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "`gained` is a window that arrived and `lost` is when one went; a moment where both happened is the ordinary one, and neither answers the other"
  )
)]
struct Changes {
  gained: bool,
  lost: Option<Instant>,
}

impl Changes {
  fn gain(&mut self) -> Result<(), Never> {
    self.gained = true;

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
      explicit039_no_reading_the_clock,
      reason = "when a window went, which is what settles: `worth_saving` is handed the other instant and decides from it, and this is a mark made where the compositor said it happened"
    )
  )]
  fn lose(&mut self) -> Result<(), Never> {
    self.lost = Some(Instant::now());

    Ok(())
  }

  fn worth_saving(&self, saved: Instant, interval: Duration) -> Result<Worth, Never> {
    let settled = self
      .lost
      .is_some_and(|at| at.elapsed() >= LOSS_SETTLES_AFTER);

    Ok(
      match ::core::primitive::bool::then_some(
        self.gained || settled || saved.elapsed() >= interval,
        (),
      ) {
        ::core::option::Option::Some(()) => Worth::Saving,
        ::core::option::Option::None => Worth::Waiting,
      },
    )
  }
}

fn note(changes: &mut Changes, stirred: &CompositorEvent) -> Result<(), Never> {
  let change = match stirred {
    CompositorEvent::WindowOpened(_)
    | CompositorEvent::WindowRenamed(_)
    | CompositorEvent::WindowMoved
    | CompositorEvent::WindowFloated
    | CompositorEvent::WindowPinned
    | CompositorEvent::WindowFilled => Changes::gain,
    CompositorEvent::WindowClosed(_) => Changes::lose,
    CompositorEvent::LayerOpened
    | CompositorEvent::LayerClosed
    | CompositorEvent::WorkspaceChanged
    | CompositorEvent::ScreenFocused
    | CompositorEvent::WindowFocused
    | CompositorEvent::ConfigurationReloaded
    | CompositorEvent::Ignored => return Ok(()),
  };

  change(changes)
}

pub(super) fn under(at: &Path, name: &str) -> Result<PathBuf, Unresumed> {
  match ::core::primitive::bool::then_some(name.is_empty(), ()) {
    ::core::option::Option::Some(()) => Ok(at.to_path_buf()),
    ::core::option::Option::None => {
      let component = LocalName::read(name)?;

      Ok(at.join(component))
    }
  }
}

fn workspace_selector(saved: &Window) -> Result<String, Never> {
  console_compositor::setup::workspace_selector(saved.workspace, &saved.workspace_named)
}

fn placement(saved: &Window) -> Result<Placing, Never> {
  Ok(match (saved.floating, saved.filling) {
    (Floating::Yes, Filling::None) => Placing::ByPixels,
    (Floating::Yes, Filling::Maximized | Filling::Screen)
    | (Floating::No, Filling::None | Filling::Maximized | Filling::Screen) => {
      Placing::ByTheLayout
    }
  })
}

fn is_same_client(saved: &Window, real: &Window) -> Result<Differs, Never> {
  let alike = saved.first_class == real.first_class && saved.first_title == real.first_title;

  Ok(match ::core::primitive::bool::then_some(alike, ()) {
    ::core::option::Option::Some(()) => Differs::No,
    ::core::option::Option::None => Differs::Yes,
  })
}

fn differs<T: PartialEq>(
  real: &Window,
  saved: &Window,
  of: fn(&Window) -> T,
) -> Result<Differs, Never> {
  Ok(
    match ::core::primitive::bool::then_some(of(real) == of(saved), ()) {
      ::core::option::Option::Some(()) => Differs::No,
      ::core::option::Option::None => Differs::Yes,
    },
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Dispatch<'a> {
  operation: Operation<'a>,
  about: &'a str,
}

fn asked_of_the_compositor(asking: Dispatch<'_>, window: &str) -> Result<(), Never> {
  let Ok(done) = asking.operation.apply();

  match done {
    DispatchResult::Success => {}
    DispatchResult::Failure(why) => {
      console_core_logging::warn!("{window}: {about}: {why}", about = asking.about);
    }
  }

  Ok(())
}

fn restore_field<T: PartialEq>(
  real: &Window,
  saved: &Window,
  of: fn(&Window) -> T,
  really: Really,
  asking: Dispatch<'_>,
) -> Result<(), Never> {
  let Ok(differs) = differs(real, saved, of);

  match differs {
    Differs::No => return Ok(()),
    Differs::Yes => {}
  }

  println!("{}: {about}", real.title, about = asking.about);

  match really {
    Really::Simulated => return Ok(()),
    Really::Truly => {}
  }

  asked_of_the_compositor(asking, &real.title)
}

fn put_back_as(saved: Filling) -> Result<Filling, Never> {
  Ok(match saved {
    Filling::Maximized | Filling::None => Filling::None,
    Filling::Screen => Filling::Screen,
  })
}

fn fill(real: &Window, saved: &Window, really: Really) -> Result<(), Never> {
  let Ok(wanted) = put_back_as(saved.filling);

  match ::core::primitive::bool::then_some(real.filling == wanted, ()) {
    ::core::option::Option::Some(()) => return Ok(()),
    ::core::option::Option::None => {}
  }

  let mode = match real.filling {
    Filling::Maximized => FullscreenMode::Maximized,
    Filling::Screen | Filling::None => FullscreenMode::Fullscreen,
  };

  println!("{}: filling the screen", real.title);

  match really {
    Really::Simulated => return Ok(()),
    Really::Truly => {}
  }

  let Ok(()) = asked_of_the_compositor(
    Dispatch {
      operation: Operation::Window {
        address: &real.address,
        action: WindowAction::Focus,
      },
      about: "taking the focus first",
    },
    &real.title,
  );

  asked_of_the_compositor(
    Dispatch {
      operation: Operation::ChangeWindow {
        address: &real.address,
        change: WindowChange::ToggleFullscreen(mode),
      },
      about: "filling the screen",
    },
    &real.title,
  )
}

fn adjust(real: &Window, saved: &Window, really: Really) -> Result<(), Never> {
  let Ok(workspace) = workspace_selector(saved);
  let Ok(()) = restore_field(
    real,
    saved,
    |window| {
      let Ok(selector) = workspace_selector(window);

      selector
    },
    really,
    Dispatch {
      operation: Operation::ChangeWindow {
        address: &real.address,
        change: WindowChange::MoveToWorkspace(&workspace),
      },
      about: "onto the workspace it was on",
    },
  );

  match saved.monitor {
    Some(on) => {
      let Ok(()) = restore_field(
        real,
        saved,
        |window| window.monitor,
        really,
        Dispatch {
          operation: Operation::MoveWorkspaceToMonitor {
            workspace: &workspace,
            monitor: on,
          },
          about: "onto the screen it was on",
        },
      );
    }
    ::core::option::Option::None => {}
  }

  let Ok(()) = restore_field(
    real,
    saved,
    |window| window.floating,
    really,
    Dispatch {
      operation: Operation::ChangeWindow {
        address: &real.address,
        change: WindowChange::ToggleFloating,
      },
      about: "floating the way it was",
    },
  );

  let Ok(()) = restore_field(
    real,
    saved,
    |window| window.pinned,
    really,
    Dispatch {
      operation: Operation::ChangeWindow {
        address: &real.address,
        change: WindowChange::TogglePinned,
      },
      about: "pinned the way it was",
    },
  );

  let Ok(()) = fill(real, saved, really);

  adjust_pixels(real, saved, really)
}

fn adjust_pixels(real: &Window, saved: &Window, really: Really) -> Result<(), Never> {
  let Ok(placing) = placement(saved);

  match placing {
    Placing::ByTheLayout => return Ok(()),
    Placing::ByPixels => {}
  }

  let Ok(()) = restore_field(
    real,
    saved,
    |window| window.size,
    really,
    Dispatch {
      operation: Operation::ChangeWindow {
        address: &real.address,
        change: WindowChange::Resize {
          x: saved.size.0,
          y: saved.size.1,
          coordinates: Coordinates::Absolute,
        },
      },
      about: "the size it was",
    },
  );

  restore_field(
    real,
    saved,
    |window| window.at,
    really,
    Dispatch {
      operation: Operation::ChangeWindow {
        address: &real.address,
        change: WindowChange::MoveTo(saved.at.0, saved.at.1),
      },
      about: "back where it was",
    },
  )
}

fn restore_windows(
  saved: &[Window],
  tags: &BTreeMap<String, String>,
  adjusting_for: Duration,
  really: Really,
) -> Result<BTreeMap<String, String>, Never> {
  match {
    ::core::primitive::bool::then_some(
      really == Really::Simulated || saved.is_empty() || adjusting_for.is_zero(),
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return Ok(BTreeMap::new());
    }
    ::core::option::Option::None => {}
  }

  let targets = Targets {
    windows: saved,
    tags,
  };
  let Ok(initial) = Restoration::initial_transition(&targets, None);
  let observing = initial.effects.contains(&RestoreEffect::Observe);
  let mut observation = (Some(initial.state), observing);
  let Ok(patience) = Schedule::asking_every(adjusting_for, WINDOW_POLL);

  let Ok(_settled) =
    console_waiting::until_handed(patience, &mut observation, |(state, observe)| {
      match { ::core::primitive::bool::then_some(!*observe, ()) } {
        ::core::option::Option::Some(()) => {
          return Ok(Ready::Yes);
        }
        ::core::option::Option::None => {}
      }

      let clients = match console_compositor::ask(console_compositor::setup::SessionClients) {
        Ok(clients) => clients,
        Err(fault) => {
          console_core_logging::debug!(
            "console-resume: waiting to observe restored windows: {fault}"
          );
          return Ok(Ready::NotYet);
        }
      };

      let current = {
        let optional_current = state.take();

        match optional_current {
          Some(matched_current) => matched_current,
          ::core::option::Option::None => return Ok(Ready::Yes),
        }
      };

      let Ok(next) = Restoration::transition(current, Observed(clients));
      *state = Some(next.state);
      *observe = false;

      for effect in next.effects {
        match effect {
          RestoreEffect::Observe => *observe = true,
          RestoreEffect::Place {
            real,
            saved: saved_2,
          } => {
            let Ok(()) = adjust(&real, saved_2, really);
          }
        }
      }

      Ok(match ::core::primitive::bool::then_some(*observe, ()) {
        ::core::option::Option::Some(()) => Ready::NotYet,
        ::core::option::Option::None => Ready::Yes,
      })
    });

  let (state, _) = observation;
  Ok(state.map_or_else(BTreeMap::new, |state_2| state_2.placed))
}

fn saved_windows(at: &Path) -> Result<Vec<Saved>, Unresumed> {
  let at_2 = at.join(CLIENTS_NAME);

  let Ok(held) = console_core_atomic_writes::read(&at_2);

  let said = match held {
    Stored::Text(said) => said,
    Stored::Absent => return Ok(Vec::new()),
    Stored::Failed(fault) => return Err(Unresumed::Read(at_2, fault)),
  };

  serde_json::from_str(&said).map_err(|fault| Unresumed::Unparsed(at_2, fault))
}

fn what_to_start(at: &Path) -> Result<Vec<String>, Unresumed> {
  let at_2 = at.join(EXEC_NAME);

  let Ok(held) = console_core_atomic_writes::read(&at_2);

  let said = match held {
    Stored::Text(said) => said,
    Stored::Absent => return Ok(Vec::new()),
    Stored::Failed(fault) => return Err(Unresumed::Read(at_2, fault)),
  };

  Ok(
    said
      .lines()
      .filter(|line| !line.trim().is_empty())
      .map(str::to_string)
      .collect(),
  )
}

fn start_programs(
  lines: &[String],
  really: Really,
  workspace_outputs: Option<&BTreeMap<&str, &str>>,
) -> Result<(), Never> {
  for line in lines {
    let Ok(start) = crate::lua::split_exec_line_on(line, workspace_outputs);
    let Ok(command) = crate::starting::launch_command(start.command);

    println!("starting {command}");

    match really {
      Really::Simulated => {}
      Really::Truly => {
        let Ok(_started) = crate::lua::exec(&crate::lua::Start {
          rules: start.rules,
          command: &command,
        });
      }
    }
  }

  Ok(())
}

fn report_setup(result: Result<(), Unresumed>) -> Result<(), Never> {
  match result {
    Err(fault) => {
      console_core_logging::warn!("console-resume: {fault}");
    }
    ::core::result::Result::Ok(_) => {}
  }

  Ok(())
}

impl Sessions {
  pub fn save(&self, name: &str) -> Result<(), Unresumed> {
    let at = under(&self.at, name)?;
    let _coordination = crate::ending::Coordination::lock()?;
    Self::save_setup(&at)?;
    Self::save_windows(&at, self.duplicates)
  }

  pub(super) fn save_setup(at: &Path) -> Result<(), Unresumed> {
    let _editing = setup::edit()?;
    let live = setup::Live::read()?;
    setup::save(at, &live)
  }

  pub(super) fn save_windows(at: &Path, duplicates: Duplicates) -> Result<(), Unresumed> {
    let open = open_windows()?;

    match open.first() {
      Some(_something_is_open) => {}
      None => {
        println!("nothing is open, so the previously saved windows stand");

        return Ok(());
      }
    }

    create_dir_all(at).map_err(|fault| Unresumed::Making(at.to_owned(), fault))?;

    let mut lines = String::new();

    let mut pids: BTreeSet<i64> = BTreeSet::new();
    let mut written: Vec<Saved> = Vec::new();

    let Ok(known) = crate::starting::from_desktop_files();
    let focused = match console_compositor::ask(console_compositor::ActiveWindow) {
      Ok(window) => window.map(|window_2| window_2.address),
      Err(fault) => {
        console_core_logging::debug!("console-resume: active window unavailable: {fault}");
        None
      }
    };

    for window in open.iter().rev() {
      let Ok(mut keeping) = to_saved(window);
      keeping.focused = focused.as_deref() == Some(&window.address);

      let again = match duplicates {
        Duplicates::OnePerWindow => false,
        Duplicates::OnePerProgram => pids.contains(&window.pid),
      };

      match ::core::primitive::bool::then_some(again, ()) {
        ::core::option::Option::Some(()) => {
          written.push(keeping);
          continue;
        }
        ::core::option::Option::None => {}
      }

      let command = match what_starts_it(window, &known) {
        Ok(command) => command,
        Err(_nothing_says_what_started_it) => {
          written.push(keeping);
          continue;
        }
      };

      let _ = pids.insert(window.pid);

      let Ok(rules) = Self::rules_for(window);

      match { ::core::primitive::bool::then_some(!command.contains('\n'), ()) } {
        ::core::option::Option::Some(()) => {
          lines.push_str(&format!("[{rules}] {command}\n"));
        }
        ::core::option::Option::None => {}
      }

      keeping.command = Some(command);
      written.push(keeping);
    }

    console_core_atomic_writes::whole(&at.join(EXEC_NAME), lines.as_bytes())
      .map_err(Unresumed::Unwritten)?;

    let json =
      serde_json::to_vec(&written).map_err(|fault| Unresumed::Unparsed(at.to_owned(), fault))?;

    console_core_atomic_writes::whole(&at.join(CLIENTS_NAME), &json).map_err(Unresumed::Unwritten)
  }

  fn rules_for(window: &Window) -> Result<String, Never> {
    let Ok(selector) = workspace_selector(window);
    let workspace = format!("workspace {selector} silent");

    let floating = match window.floating {
      Floating::Yes => Some("float".to_string()),
      Floating::No => None,
    };

    let pinned = match window.pinned {
      Pinned::Yes => Some("pin".to_string()),
      Pinned::No => None,
    };

    let filling = match window.filling {
      Filling::None => 0,
      Filling::Maximized => 1,
      Filling::Screen => 2,
    };

    Ok(
      [
        Some(format!(
          "monitor {}",
          match window.monitor {
            Some(on) => on,
            None => NO_SCREEN_NAMED,
          }
        )),
        Some(workspace),
        floating,
        Some(format!("move {} {}", window.at.0, window.at.1)),
        Some(format!("size {} {}", window.size.0, window.size.1)),
        pinned,
        Some(format!("fullscreenstate {filling}")),
      ]
      .into_iter()
      .flatten()
      .collect::<Vec<String>>()
      .join(";"),
    )
  }

  pub fn clear(&self) -> Result<(), Unresumed> {
    match self.really {
      Really::Simulated => return Ok(()),
      Really::Truly => {}
    }

    let open = open_windows()?;

    for window in open {
      let Ok(()) = asked_of_the_compositor(
        Dispatch {
          operation: Operation::Window {
            address: &window.address,
            action: WindowAction::Close,
          },
          about: "closing it",
        },
        &window.title,
      );
    }

    let Ok(patience) = Schedule::asking_every(CLOSING_SETTLES, TICK);

    let Ok(waited) = console_waiting::until(patience, || {
      Ok(match open_windows() {
        Ok(open_2) => match open_2.first() {
          Some(_still_open) => Ready::NotYet,
          None => Ready::Yes,
        },
        Err(_the_compositor_said_nothing) => Ready::Yes,
      })
    });

    match waited {
      Outcome::Happened => Ok(()),
      Outcome::RanOut => Err(Unresumed::StillOpen),
    }
  }

  fn launch_tagged(
    &self,
    records: &[Saved],
    saved: &[Window],
    workspace_outputs: Option<&BTreeMap<&str, &str>>,
  ) -> Result<BTreeMap<String, String>, Never> {
    let run = std::process::id();
    let mut tags = BTreeMap::new();

    for (index, (record, target)) in records.iter().zip(saved).enumerate() {
      let recorded_command = match record.command.as_deref() {
        Some(command) => command,
        None => continue,
      };
      let Ok(command) = crate::starting::launch_command(recorded_command);
      let tag = format!("console-resume-{run}-{index}");
      let Ok(restored_rules) = Self::rules_for(target);
      let prefix = format!("[{restored_rules}]");
      let Ok(start) = crate::lua::split_exec_line_on(&prefix, workspace_outputs);
      let Ok(rules) = start.rules.with_tag(&tag);
      println!("starting {command}");

      match self.really {
        Really::Truly => {
          let Ok(result) = crate::lua::exec(&crate::lua::Start {
            rules,
            command: &command,
          });

          match result {
            DispatchResult::Failure(fault) => {
              console_core_logging::warn!(
                "console-resume: launching {}: {fault}",
                target.first_class
              );
            }
            DispatchResult::Success => {}
          }
        }
        Really::Simulated => {}
      }

      let _old_tag = tags.insert(target.address.clone(), tag);
    }

    Ok(tags)
  }

  pub fn load(&self, name: &str) -> Result<Restore, Unresumed> {
    let Ok(mut waiting) = Waiting::here(Wait {
      who: "resume",
      what: "putting back",
    });
    let at = under(&self.at, name)?;

    let records = saved_windows(&at)?;
    let mut saved: Vec<_> = records
      .iter()
      .map(|record| {
        let Ok(window) = as_window(record);
        window
      })
      .collect();
    let modern = records.iter().any(|record| record.command.is_some());
    let starting = match ::core::primitive::bool::then_some(modern, ()) {
      ::core::option::Option::Some(()) => Vec::new(),
      ::core::option::Option::None => what_to_start(&at)?,
    };
    let (profiles, legacy) = match setup::read(&at) {
      Ok(profiles) => {
        let legacy = profiles.is_empty();

        (profiles, legacy)
      }
      Err(fault) => {
        let Ok(()) = report_setup(Err(fault));

        (Vec::new(), false)
      }
    };
    let editing = match { ::core::primitive::bool::then_some(profiles.is_empty(), ()) } {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => Some({
        let checked_value = setup::edit()?;
        checked_value
      }),
    };
    let live = match editing {
      ::core::option::Option::None => None,
      ::core::option::Option::Some(_) => match setup::Live::read() {
        Ok(live) => Some(live),
        Err(fault) => {
          let Ok(()) = report_setup(Err(fault));

          None
        }
      },
    };
    let profile = live.as_ref().and_then(|current| {
      let Ok(matching) = setup::matching(&profiles, current);
      matching.map(|chosen| (chosen, current))
    });
    let workspace_outputs = bool::then_some(!legacy, ()).map(|()| {
      profile.map_or_else(BTreeMap::new, |(chosen, current)| {
        let Ok(outputs) = chosen.workspace_outputs(current);
        outputs
      })
    });

    match { ::core::primitive::bool::then_some(!legacy, ()) } {
      ::core::option::Option::Some(()) => {
        for window in &mut saved {
          window.monitor = None;
        }
      }
      ::core::option::Option::None => {}
    }

    match profile {
      Some((profile_3, live_4)) => {
        let Ok(()) = report_setup(profile_3.apply_displays(live_4, self.really));
      }
      ::core::option::Option::None => {}
    }

    drop(editing);
    let restore_workspaces = || match profile {
      Some((profile_4, _)) => {
        let workspaces =
          setup::Live::read().and_then(|live_5| profile_4.apply_workspaces(&live_5, self.really));
        let Ok(()) = report_setup(workspaces);
      }
      None => {}
    };

    let Ok(()) = waiting.mark("read");

    let tags = match self.restoring {
      Restoring::StartingItAgain => {
        match (!modern && starting.is_empty()).then_some(()) {
          Some(()) => {
            restore_workspaces();
            return Ok(Restore::NothingSaved(at));
          }
          None => {}
        }

        self.clear()?;
        let Ok(()) = waiting.mark("cleared");
        let launched = match modern.then_some(()) {
          Some(()) => {
            let Ok(tags) = self.launch_tagged(&records, &saved, workspace_outputs.as_ref());
            tags
          }
          None => {
            let Ok(()) = start_programs(&starting, self.really, workspace_outputs.as_ref());
            BTreeMap::new()
          }
        };
        let Ok(()) = waiting.mark("started");
        let Ok(()) = waiting.finish();
        launched
      }
      Restoring::MovingWhatIsOpen => BTreeMap::new(),
    };

    let Ok(bound) = restore_windows(&saved, &tags, self.adjusting_for, self.really);

    match { ::core::primitive::bool::then_some(self.really == Really::Truly, ()) } {
      ::core::option::Option::Some(()) => {
        let Ok(()) = tiling::restore(&saved, &bound);
      }
      ::core::option::Option::None => {}
    }

    restore_workspaces();

    let focused = records
      .iter()
      .find(|record| record.focused)
      .and_then(|record| bound.get(&record.address.0));

    match (self.really, focused) {
      (Really::Truly, Some(address)) => {
        let Ok(()) = asked_of_the_compositor(
          Dispatch {
            operation: Operation::Window {
              address,
              action: WindowAction::Focus,
            },
            about: "restoring focus",
          },
          "console-resume",
        );
      }
      (Really::Simulated, _) | (Really::Truly, None) => {}
    }

    Ok(Restore::Windows)
  }

  pub fn recall(&self, name: &str) -> Result<(), Unresumed> {
    let at = under(&self.at, name)?;
    let _editing = setup::edit()?;
    let profiles = setup::read(&at)?;
    let live = setup::Live::read()?;
    let Ok(matched) = setup::matching(&profiles, &live);
    let profile = matched.ok_or(Unresumed::NoSetup(at))?;

    profile.recall(&live, self.really)
  }

  #[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
      explicit039_no_reading_the_clock,
      reason = "the cadence this loop keeps for itself, which is what a watcher is: `worth_saving` is handed the instant and decides from it, and the marks around it are this loop measuring its own waiting"
    )
  )]
  fn watch_setups(
    &self,
    name: &str,
    interval: Duration,
    recall: setup::RecallMode,
  ) -> Result<(), Unresumed> {
    let at = under(&self.at, name)?;
    let Ok(subscriber) = console_events::subscription::connect(&[EventGroup::Compositor]);
    let Ok(received) = subscriber.received();
    let Ok(patience) = Schedule::asking_every(interval, TICK);
    let Ok(batch) = console_core_number_conversion::index(EVENTS_PER_CHECK);
    let initial = (Instant::now(), setup::Watcher::default());
    let watched = console_core_iteration::iterate(initial, |(saved_when, watcher)| {
      let mut observation = (Changes::default(), watcher);
      let Ok(_either_way_it_is_time) =
        console_waiting::until_handed(patience, &mut observation, |(changes, setup)| {
          for event in received.try_iter().take(batch) {
            match event {
              Received::Connected => {}
              Received::Event(change) => {
                use console_compositor::events::OutputChange;
                let Ok(outputs) = console_compositor::events::outputs_changed(&change.text);

                match outputs {
                  OutputChange::Changed => {
                    let Ok(()) = setup.changed();
                  }
                  OutputChange::Unchanged => {}
                }

                let Ok(stirred) = console_compositor::events::read(&change.text);
                let Ok(()) = note(changes, &stirred);
              }
            }
          }

          let Ok(()) = report_setup(setup.step(Instant::now(), &at, self.really, recall));

          let Ok(worth) = changes.worth_saving(saved_when, interval);

          Ok(match worth {
            Worth::Saving => Ready::Yes,
            Worth::Waiting => Ready::NotYet,
          })
        });

      let (_, observed_setup) = observation;
      let coordination = match crate::ending::Coordination::lock() {
        Ok(coordination) => coordination,
        Err(fault) => return Ok(Step::Halt(fault)),
      };

      match coordination.as_ref().map(Coordination::status) {
        Some(Ok(ShutdownStatus::Ending)) => {
          return Ok(Step::Again((Instant::now(), observed_setup)));
        }
        Some(Ok(ShutdownStatus::Available)) | None => {}
      }

      Ok(match Self::save_windows(&at, self.duplicates) {
        Ok(()) => Step::Again((Instant::now(), observed_setup)),
        Err(fault) => Step::Halt(fault),
      })
    });

    match watched {
      Ok(fault) => Err(fault),
      Err(_endless) => Ok(()),
    }
  }

  pub fn watch(&self, name: &str, interval: Duration) -> Result<(), Unresumed> {
    self.watch_setups(name, interval, setup::RecallMode::Remember)
  }

  pub fn watch_and_recall(&self, name: &str, interval: Duration) -> Result<(), Unresumed> {
    self.watch_setups(name, interval, setup::RecallMode::Recall)
  }

  pub fn list(&self) -> Result<Vec<String>, Never> {
    let read = match std::fs::read_dir(&self.at) {
      Ok(read) => read,
      Err(_there_are_none) => return Ok(Vec::new()),
    };

    Ok(
      read
        .flatten()
        .filter(|entry| std::fs::metadata(entry.path()).is_ok_and(|metadata| metadata.is_dir()))
        .filter_map(|entry| match entry.file_name().into_string() {
          Ok(name) => Some(name),
          Err(_name_is_not_utf8) => None,
        })
        .collect(),
    )
  }

  pub fn delete(&self, name: &str) -> Result<(), Unresumed> {
    let at = under(&self.at, name)?;

    remove_dir_all(&at).map_err(|fault| Unresumed::Removing(at, fault))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct Class<'a>(&'a str);

  fn window(class: Class<'_>, title: &str) -> Result<Window, Never> {
    Ok(Window {
      address: "0x1".to_string(),
      title: title.to_string(),
      first_class: class.0.to_string(),
      first_title: title.to_string(),
      workspace: 2,
      workspace_named: "2".to_string(),
      monitor: Some(0),
      floating: Floating::No,
      pinned: Pinned::No,
      filling: Filling::None,
      at: (2, 2),
      size: (800, 600),
      pid: 42,
    })
  }

  #[test]
  fn identical_terminals_go_to_their_own_workspace_even_when_they_open_in_reverse_order() {
    use console_compositor::setup::SessionClient;

    let Ok(first) = window(Class("terminal"), "terminal");
    let second = Window {
      address: "0x2".to_owned(),
      workspace: 3,
      workspace_named: "3".to_owned(),
      ..first.clone()
    };
    let saved = [first.clone(), second.clone()];
    let tags = BTreeMap::from([
      (first.address.clone(), "console-resume-first".to_owned()),
      (second.address.clone(), "console-resume-second".to_owned()),
    ]);
    let Ok(initial) = Restoration::initial_transition(
      &Targets {
        windows: &saved,
        tags: &tags,
      },
      None,
    );
    let newer = Window {
      address: "0x3".to_owned(),
      ..first.clone()
    };
    let earlier = Window {
      address: "0x4".to_owned(),
      ..first.clone()
    };
    let Ok(restored) = Restoration::transition(
      initial.state,
      Observed(vec![
        SessionClient {
          window: newer.clone(),
          tags: vec!["console-resume-second*".to_owned()],
        },
        SessionClient {
          window: earlier.clone(),
          tags: vec!["console-resume-first*".to_owned()],
        },
      ]),
    );
    assert_eq!(
      restored.effects,
      [
        RestoreEffect::Place {
          real: newer,
          saved: &second
        },
        RestoreEffect::Place {
          real: earlier,
          saved: &first
        },
      ]
    );
  }

  fn worth(changes: &Changes) -> Result<Worth, Never> {
    changes.worth_saving(Instant::now(), Duration::from_secs(600))
  }

  fn on_special(named: &str, number: i64) -> Result<Window, Never> {
    let Ok(mut special) = window(Class("imv"), "pictures");

    special.workspace = number;
    special.workspace_named = named.to_string();

    Ok(special)
  }

  fn sessions(at: &str) -> Result<Sessions, Never> {
    Ok(Sessions {
      at: PathBuf::from(at),
      adjusting_for: Duration::from_secs(1),
      really: Really::Simulated,
      restoring: Restoring::StartingItAgain,
      duplicates: Duplicates::OnePerProgram,
    })
  }

  #[test]
  fn what_a_window_was_doing_survives_the_trip_through_the_file() {
    let Ok(mut was) = window(Class("foot"), "a shell");
    was.floating = Floating::Yes;
    was.pinned = Pinned::Yes;
    was.filling = Filling::Screen;

    let Ok(saved) = to_saved(&was);
    let Ok(again) = as_window(&saved);

    assert_eq!(again, was);
  }

  #[test]
  fn a_special_workspace_goes_by_the_name_no_number_would_carry() {
    let Ok(special) = on_special("special:sky", -99);

    assert_eq!(workspace_selector(&special), Ok("special:sky".to_string()));

    let Ok(window) = window(Class("foot"), "a shell");

    assert_eq!(workspace_selector(&window), Ok("2".to_string()));
  }

  #[test]
  fn the_second_special_workspace_is_not_the_first_one() {
    let Ok(second) = on_special("special:magic", -98);
    let Ok(rules) = Sessions::rules_for(&second);

    assert_eq!(workspace_selector(&second), Ok("special:magic".to_string()));
    assert!(rules.contains("workspace special:magic silent"), "{rules}");
    assert!(
      !rules.contains("-98"),
      "a number below -99 is a workspace no one can ask for: {rules}"
    );
  }

  #[test]
  fn a_workspace_of_someones_own_called_specials_is_not_a_special_one() {
    let Ok(mut named) = window(Class("foot"), "a shell");
    named.workspace = 4;
    named.workspace_named = "specials".to_string();

    assert_eq!(workspace_selector(&named), Ok("4".to_string()));
  }

  #[test]
  fn a_window_is_the_same_one_by_what_it_was_called_rather_than_what_it_shows() {
    let Ok(saved) = window(Class("foot"), "foot");
    let Ok(mut now) = window(Class("foot"), "foot");
    now.title = "half a build".to_string();

    assert_eq!(is_same_client(&saved, &now), Ok(Differs::No));

    let Ok(window) = window(Class("librewolf"), "foot");

    assert_eq!(is_same_client(&saved, &window), Ok(Differs::Yes));
  }

  #[test]
  fn a_special_workspace_is_asked_for_by_the_word_the_rules_take() {
    let Ok(special) = on_special("special:sky", -99);
    let Ok(rules) = Sessions::rules_for(&special);

    assert!(rules.contains("workspace special:sky silent"), "{rules}");
  }

  #[test]
  fn the_special_workspace_no_one_named_is_still_a_special_one() {
    let Ok(special) = on_special("special", -99);
    let Ok(rules) = Sessions::rules_for(&special);

    assert!(rules.contains("workspace special silent"), "{rules}");
  }

  #[test]
  fn a_tiled_window_is_left_where_the_layout_puts_it() {
    let Ok(window) = window(Class("foot"), "a shell");

    assert_eq!(placement(&window), Ok(Placing::ByTheLayout));
  }

  #[test]
  fn a_floating_window_is_put_back_by_the_corner_and_the_size_it_had() {
    let Ok(mut floats) = window(Class("imv"), "pictures");
    floats.floating = Floating::Yes;

    assert_eq!(placement(&floats), Ok(Placing::ByPixels));
  }

  #[test]
  fn a_window_filling_the_screen_is_not_shifted_about_underneath_it() {
    let Ok(mut filling) = window(Class("imv"), "pictures");
    filling.floating = Floating::Yes;
    filling.filling = Filling::Screen;

    assert_eq!(placement(&filling), Ok(Placing::ByTheLayout));
  }

  #[test]
  fn a_window_saved_maximized_comes_back_sharing_its_place_and_one_filling_the_screen_still_fills_it()
   {
    assert_eq!(put_back_as(Filling::Maximized), Ok(Filling::None));
    assert_eq!(put_back_as(Filling::Screen), Ok(Filling::Screen));
    assert_eq!(put_back_as(Filling::None), Ok(Filling::None));
  }

  #[test]
  fn a_rule_a_window_does_not_want_is_left_out_rather_than_left_empty() {
    let Ok(window) = window(Class("foot"), "a shell");
    let Ok(plain) = Sessions::rules_for(&window);

    assert!(!plain.contains("float"), "{plain}");
    assert!(
      !plain.contains(";;"),
      "an empty rule is a rule no one wrote: {plain}"
    );
  }

  #[test]
  fn a_window_that_floats_and_is_pinned_says_both() {
    let Ok(mut both) = window(Class("imv"), "pictures");
    both.floating = Floating::Yes;
    both.pinned = Pinned::Yes;

    let Ok(rules) = Sessions::rules_for(&both);

    assert!(rules.contains(";float;"), "{rules}");
    assert!(rules.contains(";pin;"), "{rules}");
  }

  #[test]
  fn a_session_with_no_name_is_the_directory_the_sessions_live_in() -> Result<(), Unresumed> {
    let at = PathBuf::from("/home/someone/.local/share/console/resume");
    let default_path = under(&at, "")?;
    let named_path = under(&at, "yesterday")?;

    assert_eq!(default_path, at);
    assert_eq!(named_path, at.join("yesterday"));

    Ok(())
  }

  #[test]
  fn a_session_name_cannot_escape_its_directory() {
    let at = Path::new("/home/someone/.local/share/console/resume");

    for invalid in ["..", "../yesterday", "/other", "a/b", "."] {
      assert!(matches!(under(at, invalid), Err(Unresumed::InvalidName(name)) if name == invalid));
    }
  }

  #[test]
  fn saved_window_text_keeps_the_existing_json_shape() -> Result<(), serde_json::Error> {
    let previous = r#"{
      "address": "0x1", "title": "notes", "first_class": "editor", "first_title": "new",
      "workspace": 2, "workspace_named": "2", "floating": false, "pinned": false,
      "fullscreen": 0, "at": [2, 2], "size": [800, 600], "pid": 42
    }"#;
    let saved: Saved = serde_json::from_str(previous)?;
    let Ok(restored) = as_window(&saved);
    let rewritten = serde_json::to_string(&saved)?;

    assert_eq!(restored.title, "notes");
    assert_eq!(restored.monitor, None);
    assert!(rewritten.contains(r#""title":"notes""#));

    let unexpected = previous.replacen('{', r#"{"unrecognized":true,"#, 1);
    let unread = serde_json::from_str::<Saved>(&unexpected);

    assert!(matches!(unread, Err(_unknown_field)));

    Ok(())
  }

  #[test]
  fn a_change_to_a_window_is_worth_saving_and_the_screen_moving_is_not() {
    let mut changes = Changes::default();
    let Ok(()) = note(&mut changes, &CompositorEvent::WorkspaceChanged);

    assert_eq!(worth(&changes), Ok(Worth::Waiting));

    let Ok(()) = note(&mut changes, &CompositorEvent::WindowMoved);

    assert_eq!(worth(&changes), Ok(Worth::Saving));
  }

  #[test]
  fn a_window_that_has_just_closed_is_not_written_down_yet() {
    let mut changes = Changes::default();
    let Ok(()) = note(
      &mut changes,
      &CompositorEvent::WindowClosed("0x1".to_string()),
    );

    assert_eq!(
      worth(&changes),
      Ok(Worth::Waiting),
      "the first thing logging out looks like is one window closing"
    );
  }

  #[test]
  fn listing_where_no_session_has_ever_been_saved_is_no_sessions_rather_than_a_fault() {
    let Ok(sessions) = sessions("/nowhere/at/all");

    assert_eq!(sessions.list(), Ok(Vec::new()));
  }
}

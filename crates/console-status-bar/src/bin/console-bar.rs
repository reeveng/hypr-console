//! The bar this desktop draws itself.
//!
//!     console-bar
//!
//! waybar drew it until this file existed, and what waybar contributed was a
//! layer surface, a stylesheet and a loop that read a pipe: every word on the
//! bar was already ours, arriving as JSON on seven programs' standard output
//! and being matched again against a class name in CSS. What that cost is
//! written down in three places -- an icon whose meaning crossed out of Rust as
//! a word and back in as a selector, a stylesheet read once at startup so that
//! writing the palette did not reach a bar that was up, and no way at all to
//! say how long anything took, because the moment a label appeared happened
//! inside a loop this repository does not compile. All three are gone with it.
//!
//! **One process, not one per module with its own watcher.** Seven programs sat
//! on seven subscriptions to the same four sources and asked the compositor
//! seven times over what was on the screen. This asks once per pass and answers
//! every icon out of the one reply. That is most of the reason to do it at all:
//! a handheld pays for each of those wake-ups, and the bar is up for as long as
//! the session is.
//!
//! **It is not what a panel's statelessness argues against.** A panel is asked
//! for and should be born when it is asked for; a bar is always there by
//! definition, so one resident program drawing it loses no state property,
//! because there is no opening to be stateless across. What it holds is the
//! last thing each source said, which is a cache of someone else's facts
//! rather than a state anything decides from.
//!
//! **Nothing here decides anything about the bar.** `showing` says where each
//! slab lands and what a thumb on it hits, `holding` says which icon and which
//! tone, `reading` and `notifications` say what the machine answered. This is the
//! three machines those were written to be kept away from: a compositor, a
//! pile of subscriptions, and a clock.
//!
//! **Waking.** Every source is a channel of *ask again*, and a thread per
//! source writes to its own nonblocking pipe alongside the compositor's
//! socket. The descriptor identifies the source, so a workspace switch does
//! not re-run `wpctl`. When something has just
//! happened the next wait is short rather than long, which is how a burst --
//! and a layer opening is always several lines -- is absorbed in one extra pass
//! instead of one pass each.
//!
//! The compositor supplies the bar's length along the chosen edge. The font
//! size sets its depth, which the surface reserves for itself. Changing the
//! edge replaces the layer surface so the reserved room moves with it.
//!
//! **The one wait that is a duration is an apply.** While the strip is filling
//! there is nothing to subscribe to: the engine writes a number to a file as it
//! goes, and how often the bar looks at it is the bar's own frame rate for a
//! thing that is moving. That an apply has *started* is heard rather than
//! polled: the folder that file is in, and the one the tab in front is noted
//! in, are watched with inotify in the same `poll` as the compositor's socket,
//! so an idle bar waits a minute at a time and a filling one is still on the
//! first frame of it. A folder that is not there yet -- `/run/console` before
//! the first apply since boot -- is watched from the nearest one that is, and
//! the watch moves down each pass until it reaches it. Every other wait here
//! is until something says so.
//!
//! The engine used to send `SIGRTMIN+4` to a pid the bar had written down, and
//! the bar blocked it and read it off a `signalfd`. That was a pid file, a name
//! checked against a truncated `comm`, and a signal whose default is to end
//! the bar if it lands before the mask is up -- all to say what the file
//! changing already says, to a bar that was reading the file anyway.
//!
//! **Everything a tap starts is reaped, and a child ending wakes the loop.**
//! Only the last program a tap started used to be held, for the lit icon's
//! sake, and every one before it was dropped without anyone waiting on it. A
//! hundred taps on the device was a hundred dead entries under the bar, one per
//! panel, for as long as the session lasted. They are all held now, each beside
//! the panel it was started for, and let go of as they end: the ending is told
//! to the bar's `panels` machine, and the compositor is asked again on the same
//! pass. Each one is watched through a pidfd in the same `poll`, because a
//! panel that ends between two things happening on the screen would otherwise
//! lie there until the clock next turned over. A pidfd rather than
//! `SIGCHLD`: a child that has ended and not been reaped is still there to be
//! opened, so asking again every pass cannot miss one, and nothing about it is
//! inherited by the panels it watches.

use std::collections::BTreeSet;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::process::{ExitCode, Stdio};
use std::sync::mpsc::sync_channel;
use std::time::{Duration, Instant};

use console_application_music::player::Sound;
use console_compositor::{Carrying, Layer, Workspace, WorkspaceTarget};
use console_core_color::palette::{self, PaletteError, ThemeSettings, WearingError};
use console_core_geometry::{Point, Size};
use console_core_internal_programs::InternalProgram;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_i32};
use console_core_shapes::Shape;
use console_draw_painting::{Frame, onto};
use console_draw_surface::layer_focus::Selection;
use console_draw_surface::standing::{Anchor, Closed, Keyboard, Margin, Room, Under, Wanted};
use console_draw_surface::{KeyboardEvent, PointerEvent, Surface, SurfaceError};
use console_notifications::updating;
use console_onscreen::focus;
use console_program_contract::EventGroup;
use console_program_lifetime::{Detached, Still};
use console_status_bar::applets::{self, Applet};
use console_status_bar::dwindling::Watching;
use console_status_bar::measuring::{self, sized};
use console_status_bar::notifications::{Waiting, notifications};
use console_status_bar::panels::{self, Panel, PanelEvent, Panels};
use console_status_bar::preferences::{Placement, Preferences};
use console_status_bar::reading::{Reading, StatusItem};
use console_status_bar::showing::{self, BarAction, Filling, Fitting, Rendered, Slot, Wearing};
use console_status_bar::state::{self, BarState, Paused, Playing};
use console_status_bar::watch;
use console_status_bar::{clock, preferences};
use rustix::fs::inotify::{self, CreateFlags, WatchFlags};
use rustix::io::Errno;
use rustix::process::{Pid, PidfdFlags, pidfd_open};

#[path = "console-bar/waking.rs"]
mod waking;

const GATHERING: Duration = Duration::from_millis(120);

const FOLLOWING: Duration = Duration::from_millis(200);

const AT_MOST: Duration = Duration::from_secs(60);

#[derive(Debug)]
enum Cannot {
  Palette(WearingError),
  Pipe(std::io::Error),
  Deaf(rustix::io::Errno),
  Screenless,
  Compositor(SurfaceError),
}

impl std::fmt::Display for Cannot {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Cannot::Palette(why) => write!(to, "{why}"),
      Cannot::Pipe(why) => write!(to, "nothing to be woken down: {why}"),
      Cannot::Deaf(why) => write!(to, "nothing to hear an apply or a tab on: {why}"),
      Cannot::Screenless => {
        write!(to, "the compositor named no screen to draw a bar across")
      }
      Cannot::Compositor(why) => write!(to, "{why}"),
    }
  }
}

impl std::error::Error for Cannot {}

impl From<WearingError> for Cannot {
  fn from(why: WearingError) -> Cannot {
    Cannot::Palette(why)
  }
}

impl From<PaletteError> for Cannot {
  fn from(why: PaletteError) -> Cannot {
    Cannot::Palette(WearingError::Palette(why))
  }
}

impl From<SurfaceError> for Cannot {
  fn from(why: SurfaceError) -> Cannot {
    Cannot::Compositor(why)
  }
}

fn main() -> ExitCode {
  match ::core::primitive::bool::then_some(
    {
      std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == focus::FLAG)
    },
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return match focus::Layer::Bar.take() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
          eprintln!("console-bar: focus: {why}");

          ExitCode::FAILURE
        }
      };
    }
    ::core::option::Option::None => {}
  }

  match drawing() {
    Ok(()) => ExitCode::SUCCESS,
    Err(why) => {
      eprintln!("console-bar: {why}");

      ExitCode::FAILURE
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Woke {
  Surfaces,
  Sound,
  Bluetooth,
  Network,
  Battery,
  Notifications,
  Player,
}

fn wakes(item: StatusItem) -> Result<Woke, Never> {
  Ok(match item {
    StatusItem::Battery => Woke::Battery,
    StatusItem::Bluetooth => Woke::Bluetooth,
    StatusItem::Network => Woke::Network,
    StatusItem::Sound => Woke::Sound,
  })
}

struct Tracked {
  item: StatusItem,
  reading: Reading,
  due: Option<Instant>,
}

struct ContentFiles {
  preferences: Option<PathBuf>,
  manifests: Vec<PathBuf>,
  applications: Vec<PathBuf>,
  live: Option<PathBuf>,
  folders: Vec<PathBuf>,
  hearing: OwnedFd,
}

impl ContentFiles {
  fn open() -> Result<Self, Cannot> {
    let Ok(preferences) = preferences::path();
    let Ok(manifests) = applets::folders();
    let Ok(applications) = console_core_places::applications();
    let Ok(live) = applets::live_folder();
    let mut folders = manifests.clone();
    folders.extend(applications.iter().cloned());
    folders.extend(live.clone());
    folders.extend(
      preferences
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf),
    );
    let hearing =
      inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK).map_err(Cannot::Deaf)?;
    Ok(Self {
      preferences,
      manifests,
      applications,
      live,
      folders,
      hearing,
    })
  }

  fn read(&self, content: &mut Content) -> Result<(), Never> {
    let Ok(()) = watch_folders(&self.hearing, &self.folders);

    match &self.preferences {
      Some(at) => match Preferences::load(at) {
        Ok(preferences) => content.preferences = preferences,
        Err(error) => eprintln!("console-bar: {}: {error}", at.display()),
      },
      ::core::option::Option::None => {}
    }

    let Ok(applets) = applets::discover(&self.manifests, &self.applications);
    let Ok(slots) = applets::slots(&applets, &content.preferences, self.live.as_deref());
    content.applets = applets;
    content.slots = slots;

    Ok(())
  }
}

#[cfg(test)]
mod content_tests {
  use super::*;

  #[test]
  fn creating_and_updating_applet_files_wakes_the_bar() -> Result<(), Box<dyn std::error::Error>>
  {
    let root = console_core_temporary_directories::fresh("bar-content-watch")?;
    let manifests = root.join("data/bar-applets");
    let live = root.join("runtime/bar-applets");
    let applications = root.join("applications");
    let at = root.join("status-bar.json");
    let hearing = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK)?;
    let files = ContentFiles {
      preferences: Some(at.clone()),
      manifests: vec![manifests.clone()],
      applications: vec![applications.clone()],
      live: Some(live.clone()),
      folders: vec![root, manifests.clone(), live.clone(), applications.clone()],
      hearing,
    };
    let mut content = Content::default();
    let Ok(()) = files.read(&mut content);

    std::fs::create_dir_all(&manifests)?;
    console_core_atomic_writes::whole(&manifests.join("example.json"), br#"{"name":"Example"}"#)?;
    Preferences::update(&at, |preferences| {
      preferences.placement = Placement::Bottom;
      let Ok(()) = preferences.toggle_applet("example");
    })?;
    assert_eq!(heard_again(&files.hearing), Ok(FileEvents::Received));
    let Ok(()) = files.read(&mut content);
    assert_eq!(content.preferences.placement, Placement::Bottom);
    assert_eq!(content.slots.len(), 1);

    std::fs::create_dir_all(&live)?;
    let status = live.join("example.txt");
    console_core_atomic_writes::whole(&status, b"")?;
    assert_eq!(heard_again(&files.hearing), Ok(FileEvents::Received));
    let Ok(()) = files.read(&mut content);
    assert!(content.slots.is_empty());

    console_core_atomic_writes::whole(&status, b"Visible again")?;
    assert_eq!(heard_again(&files.hearing), Ok(FileEvents::Received));
    let Ok(()) = files.read(&mut content);
    assert_eq!(content.slots.len(), 1);

    console_core_atomic_writes::whole(
      &manifests.join("obs.json"),
      include_bytes!("../../../../files/usr/local/share/console/bar-applets/obs.json"),
    )?;
    Preferences::update(&at, |preferences| {
      let Ok(()) = preferences.toggle_applet("obs");
    })?;
    assert_eq!(heard_again(&files.hearing), Ok(FileEvents::Received));
    let Ok(()) = files.read(&mut content);
    assert_eq!(content.slots.len(), 1);

    std::fs::create_dir(&applications)?;
    let desktop_entry = applications.join("com.obsproject.Studio.desktop");
    console_core_atomic_writes::whole(&desktop_entry, b"[Desktop Entry]\nName=OBS Studio\n")?;
    assert_eq!(heard_again(&files.hearing), Ok(FileEvents::Received));
    let Ok(()) = files.read(&mut content);
    assert_eq!(content.slots.len(), 2);

    std::fs::remove_file(&desktop_entry)?;
    assert_eq!(heard_again(&files.hearing), Ok(FileEvents::Received));
    let Ok(()) = files.read(&mut content);
    assert_eq!(content.slots.len(), 1);

    Ok(())
  }
}

#[derive(Default)]
struct Content {
  preferences: Preferences,
  applets: Vec<Applet>,
  slots: Vec<Slot>,
}

fn read_wearing(theme: &ThemeSettings) -> Result<Wearing, WearingError> {
  let spent_with = palette::spent_with(theme)?;
  let colors = Wearing::out_of(&spent_with)?;

  Ok(colors)
}

fn drawing() -> Result<(), Cannot> {
  let Ok(theme) = ThemeSettings::current();
  let wearing = read_wearing(&theme)?;
  let mut surface = Surface::connect()?;
  let hearing =
    inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK).map_err(Cannot::Deaf)?;
  let Ok(folders) = heard_in();
  let content_files = ContentFiles::open()?;
  let mut content = Content::default();
  let Ok(()) = content_files.read(&mut content);
  let waking = sources().map_err(Cannot::Pipe)?;

  let whole = screen()?;
  let Ok(fitting) = Fitting::of_em();

  let placement = content.preferences.placement;
  raised(&mut surface, fitting, placement, LayerMode::Passive)?;
  let focus_requests = match focus::Layer::Bar.listen() {
    Ok(socket) => Some(socket),
    Err(error) => {
      eprintln!("console-bar: focus requests: {error}");

      None
    }
  };

  let mut dwindling = Watching::default();
  let Ok(readings) = first_readings(&mut dwindling);
  let Ok(when) = clock::current();
  let Ok(layers) = listed(Vec::new());
  let Ok(held) = first_held(&readings, &layers);
  let turning = Turning {
    surface,
    theme,
    wearing,
    dwindling,
    readings,
    held,
    was: when,
    settling: Settling::No,
    last: None,
    requested: Vec::new(),
    started: Vec::new(),
    layers,
    selection: Selection::Idle,
    focus_layer: LayerMode::Passive,
    placement,
    content,
    waking,
  };
  let around = Around {
    whole,
    fitting,
    hearing: &hearing,
    folders: &folders,
    focus_requests: focus_requests.as_ref(),
    content_files: &content_files,
  };

  match console_core_iteration::iterate(turning, |turning_2| turned(turning_2, around)) {
    Ok(drawn) => drawn,
    Err(_endless) => Ok(()),
  }
}

#[derive(Clone, Copy)]
enum LayerMode {
  Focused,
  Passive,
}

#[derive(Debug, PartialEq, Eq)]
enum FileEvents {
  Received,
  Idle,
}

enum AppletChange {
  Different,
  Same,
}

impl From<bool> for AppletChange {
  fn from(changed: bool) -> Self {
    match changed {
      true => Self::Different,
      false => Self::Same,
    }
  }
}

struct Turning {
  surface: Surface,
  theme: ThemeSettings,
  wearing: Wearing,
  dwindling: Watching,
  readings: Vec<Tracked>,
  held: BarState,
  was: clock::Standing,
  settling: Settling,
  last: Option<Rendered>,
  requested: Vec<Requested>,
  started: Vec<Started>,
  layers: Vec<Layer>,
  selection: Selection<BarAction>,
  focus_layer: LayerMode,
  placement: Placement,
  content: Content,
  waking: Vec<waking::Source>,
}

#[derive(Clone, Copy)]
struct Around<'a> {
  whole: Size<u32>,
  fitting: Fitting,
  hearing: &'a OwnedFd,
  folders: &'a [PathBuf],
  focus_requests: Option<&'a UnixDatagram>,
  content_files: &'a ContentFiles,
}

fn turned(
  mut turning: Turning,
  around: Around<'_>,
) -> Result<Step<Turning, Result<(), Cannot>>, Never> {
  let (drawn, filling, actions) = match draw_turn(&mut turning, around) {
    Ok(painted) => painted,
    Err(fault) => return Ok(Step::Halt(Err(fault))),
  };
  let Ok(()) = begun(&mut turning.requested, &mut turning.started);
  let Ok(closed) = turning.surface.closed();

  match closed {
    Closed::Yes => return Ok(Step::Halt(Ok(()))),
    Closed::No => {}
  }

  match wait_for_turn(&mut turning, around, filling) {
    Ok(()) => {}
    Err(fault) => return Ok(Step::Halt(Err(fault))),
  }

  let Ok(()) = user_input(&mut turning, &drawn, &actions, around.focus_requests);
  let Ok(_preferences_events) = heard_again(around.hearing);
  let Ok(()) = reload_content(&mut turning, around.content_files);
  let Ok(()) = refresh_turn(&mut turning);

  Ok(Step::Again(turning))
}

fn draw_turn(
  turning: &mut Turning,
  around: Around<'_>,
) -> Result<(Rendered, Filling, Vec<BarAction>), Cannot> {
  let wished = turning.content.preferences.placement;

  match (turning.placement != wished).then_some(()) {
    Some(()) => {
      turning.placement = wished;
      turning.selection = Selection::Idle;
      turning.focus_layer = LayerMode::Passive;
      turning.last = None;
      let Ok(()) = turning.surface.hide();

      raised(
        &mut turning.surface,
        around.fitting,
        wished,
        LayerMode::Passive,
      )?;
    }
    None => {}
  }

  let Ok(progress) = updating::progress();
  let filling = match progress {
    Some(progress_2) => Filling::At(progress_2.permille),
    None => Filling::None,
  };
  let Ok(layout) = turning
    .held
    .configured(&turning.content.preferences, filling);
  let Ok(mut drawn) = render_turn(turning, &layout, around);
  let actions: Vec<_> = drawn.touching.iter().map(|region| region.action).collect();

  focus_outline(turning, around, &mut drawn, &actions)?;

  match (turning.last.as_ref() == Some(&drawn)).then_some(()) {
    Some(()) => {}
    None => {
      let Ok(()) = painted(&mut turning.surface, &drawn);

      turning.last = Some(drawn.clone());
    }
  }

  Ok((drawn, layout.filling, actions))
}

fn render_turn(
  turning: &Turning,
  layout: &showing::Layout,
  around: Around<'_>,
) -> Result<Rendered, Never> {
  let fitting = around.fitting;
  let Ok(mut bar) = sized(layout, fitting);
  let Ok(shortcuts) = turning.held.settings_applets(&turning.content.preferences);
  let Ok(mut applets) = measuring::every(&shortcuts, fitting);
  let Ok(published) = measuring::every(&turning.content.slots, fitting);
  applets.extend(published);
  let Ok(logical) = turning.surface.logical();

  match turning.placement {
    Placement::Top | Placement::Bottom => {
      let room = match logical {
        Some(room) => room,
        None => around.whole,
      };

      showing::along_with_applets(&bar, &applets, &turning.wearing, room, turning.placement)
    }
    Placement::Left | Placement::Right => {
      let room = match logical {
        Some(room) => room,
        None => {
          let Ok(width) = fitting.sidebar_width();

          Size {
            width,
            height: around.whole.height,
          }
        }
      };
      let width = room.width.saturating_sub(fitting.thin).saturating_sub(
        fitting
          .margin
          .saturating_add(fitting.between)
          .saturating_mul(2),
      );

      for group in [&mut bar.left, &mut bar.middle, &mut bar.right, &mut applets] {
        let Ok(()) = measuring::compact(group, fitting, width);
      }

      showing::sidebar(&bar, &applets, &turning.wearing, room, turning.placement)
    }
  }
}

fn focus_outline(
  turning: &mut Turning,
  around: Around<'_>,
  drawn: &mut Rendered,
  actions: &[BarAction],
) -> Result<(), Cannot> {
  let Ok(()) = turning.selection.keep(actions);
  let Ok(target) = turning.selection.target();
  let layer = match target {
    Some(_action) => LayerMode::Focused,
    None => LayerMode::Passive,
  };

  match (turning.focus_layer, layer) {
    (LayerMode::Focused, LayerMode::Passive) | (LayerMode::Passive, LayerMode::Focused) => {
      let Ok(()) = turning.surface.hide();
      raised(
        &mut turning.surface,
        around.fitting,
        turning.placement,
        layer,
      )?;
      turning.focus_layer = layer;
      turning.last = None;
    }
    (LayerMode::Focused, LayerMode::Focused) | (LayerMode::Passive, LayerMode::Passive) => {}
  }

  match target.and_then(|action| drawn.touching.iter().find(|region| region.action == action)) {
    Some(region) => {
      let Ok(outline) = region.panel.focus_outline(turning.wearing.text);
      drawn.shapes.extend(outline.map(Shape::Line));
    }
    None => {}
  }

  Ok(())
}

fn wait_for_turn(
  turning: &mut Turning,
  around: Around<'_>,
  filling: Filling,
) -> Result<(), Cannot> {
  let Ok(wait) = waiting(&turning.readings, filling, turning.settling);
  let Ok(()) = watch_folders(around.hearing, around.folders);
  let Ok(ending) = endings(&turning.started);
  let mut also = vec![around.hearing.as_fd(), around.content_files.hearing.as_fd()];

  also.extend(turning.waking.iter().map(AsFd::as_fd));
  also.extend(ending.iter().map(AsFd::as_fd));
  also.extend(around.focus_requests.map(AsFd::as_fd));
  let Ok(focus_timeout) = turning.selection.timeout(Instant::now());
  let until = focus_timeout.map_or(wait, |focus| focus.min(wait));
  let _woke = turning.surface.wait(&also, Some(until))?;

  Ok(())
}

fn user_input(
  turning: &mut Turning,
  drawn: &Rendered,
  actions: &[BarAction],
  listening: Option<&UnixDatagram>,
) -> Result<(), Never> {
  let focus_request = match listening {
    Some(socket) => match focus::heard(socket) {
      Ok(request) => request,
      Err(fault) => {
        eprintln!("console-bar: could not read a focus request: {fault}");

        None
      }
    },
    None => None,
  };
  let Ok(owns_keyboard) = turning.surface.keyboard_focus();
  let Ok(()) = turning.selection.observe(owns_keyboard, Instant::now());

  match focus_request {
    Some(focus::Request::Release) => turning.selection = Selection::Idle,
    Some(focus::Request::Take) | None => {}
  }

  let Ok(keys) = turning.surface.keyboard_events();

  for KeyboardEvent::Down { key } in keys {
    let Ok(activated) = turning.selection.pressed(key, actions);

    match activated {
      Some(action) => {
        let Ok(()) = queue(
          action,
          &mut turning.held,
          &turning.content.applets,
          &mut turning.requested,
        );
        turning.selection = Selection::Idle;
      }
      None => {}
    }
  }

  let Ok(tapped) = tapped(&mut turning.surface, drawn, &mut turning.selection);

  for action in tapped {
    let Ok(()) = queue(
      action,
      &mut turning.held,
      &turning.content.applets,
      &mut turning.requested,
    );
  }

  match focus_request {
    Some(focus::Request::Take) => {
      let Ok(()) = turning.selection.enter(actions, Instant::now());
      let Ok(target) = turning.selection.target();

      match target {
        Some(_target) => {
          let _peer_may_be_absent = focus::Layer::Bar.release_peer();
        }
        None => {}
      }
    }
    Some(focus::Request::Release) | None => {}
  }

  Ok(())
}

fn reload_content(turning: &mut Turning, files: &ContentFiles) -> Result<(), Never> {
  let Ok(events) = heard_again(&files.hearing);

  match events {
    FileEvents::Idle => return Ok(()),
    FileEvents::Received => {}
  }

  let Ok(theme) = ThemeSettings::current();

  match { ::core::primitive::bool::then_some(theme != turning.theme, ()) } {
    ::core::option::Option::Some(()) => match read_wearing(&theme) {
      Ok(wearing) => {
        turning.theme = theme;
        turning.wearing = wearing;
      }
      Err(fault) => console_core_logging::warn!("console-bar: loading theme: {fault}"),
    },
    ::core::option::Option::None => {}
  }

  let Ok(target) = turning.selection.target();
  let position = match target {
    Some(BarAction::Applet(position)) => Some(position),
    Some(_) | None => None,
  };

  match position {
    Some(position_2) => {
      let Ok(offset) = index(position_2);
      let previous = turning.content.applets.get(offset).cloned();
      let Ok(()) = files.read(&mut turning.content);

      match AppletChange::from(turning.content.applets.get(offset) != previous.as_ref()) {
        AppletChange::Different => turning.selection = Selection::Idle,
        AppletChange::Same => {}
      }
    }
    None => {
      let Ok(()) = files.read(&mut turning.content);
    }
  }

  Ok(())
}

fn refresh_turn(turning: &mut Turning) -> Result<(), Never> {
  let Ok((running, ended)) = reaped(std::mem::take(&mut turning.started));
  turning.started = running;
  let Ok(woke) = waking::take_woken(&mut turning.waking);
  turning.settling = match woke.is_empty().then_some(()) {
    Some(()) => Settling::No,
    None => Settling::Yes,
  };
  let Ok(looked) = again(&woke, &mut turning.readings, &mut turning.dwindling);
  let Ok(()) = ticked(&mut turning.readings, &mut turning.dwindling);
  let Ok(()) = refreshed(&woke, looked, &mut turning.held);

  match (woke.contains(&Woke::Surfaces) || looked == Looked::Again || !ended.is_empty())
    .then_some(())
  {
    Some(()) => {
      let Ok(asked) = listed(std::mem::take(&mut turning.layers));

      turning.layers = asked;
    }
    None => {}
  }

  for panel in ended {
    let Ok(_starts_nothing) = turning.held.told(PanelEvent::Ended(panel));
  }

  let Ok(up) = on_screen(&turning.layers);
  let Ok(_starts_nothing) = turning.held.told(PanelEvent::Shown(up));
  let Ok(now) = clock::current();

  match (now == turning.was).then_some(()) {
    Some(()) => {}
    None => {
      let Ok(said) = clock::now();

      turning.held.clock = said;
      turning.was = now;
    }
  }

  turning.held.readings = turning
    .readings
    .iter()
    .map(|one| (one.item, one.reading.clone()))
    .collect();

  Ok(())
}

fn raised(
  surface: &mut Surface,
  fitting: Fitting,
  placement: Placement,
  layer: LayerMode,
) -> Result<(), Cannot> {
  let Ok(tall) = fitting.height();
  let Ok(wide) = fitting.sidebar_width();
  let Ok(margin) = Margin::none();
  let horizontal = Size {
    width: 0,
    height: tall,
  };
  let vertical = Size {
    width: wide,
    height: 0,
  };
  let (anchor, size) = match placement {
    Placement::Top => (Anchor::Top, horizontal),
    Placement::Bottom => (Anchor::Bottom, horizontal),
    Placement::Left => (Anchor::Left, vertical),
    Placement::Right => (Anchor::Right, vertical),
  };

  let (namespace, keyboard, under) = match layer {
    LayerMode::Focused => {
      let Ok(namespace) = focus::Layer::Bar.namespace();

      (namespace, Keyboard::Takes, Under::None)
    }
    LayerMode::Passive => (showing::WHO, Keyboard::Declines, Under::Anything),
  };

  surface.show(&Wanted {
    namespace: namespace.to_owned(),
    anchor,
    size,
    margin,
    keyboard,
    room: Room::Reserves,
    under,
  })?;

  Ok(())
}

fn first_readings(dwindling: &mut Watching) -> Result<Vec<Tracked>, Never> {
  let mut readings = Vec::new();

  for item in state::ALONG {
    let Ok(reading) = read_item(item, dwindling);
    let Ok(due) = due(item, dwindling);

    readings.push(Tracked { item, reading, due });
  }

  Ok(readings)
}

fn first_held(readings: &[Tracked], layers: &[Layer]) -> Result<BarState, Never> {
  let Ok(bell) = rung();
  let Ok(music) = playing();
  let Ok(said) = clock::now();
  let Ok((workspaces, front)) = walked();
  let Ok(up) = on_screen(layers);
  let mut held = BarState {
    readings: readings
      .iter()
      .map(|one| (one.item, one.reading.clone()))
      .collect(),
    bell,
    music,
    clock: said,
    workspaces,
    front,
    panels: Panels::default(),
  };
  let Ok(_starts_nothing) = held.told(PanelEvent::Shown(up));

  Ok(held)
}

fn refreshed(woke: &BTreeSet<Woke>, looked: Looked, held: &mut BarState) -> Result<(), Never> {
  match ::core::primitive::bool::then_some(woke.contains(&Woke::Notifications), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(bell) = rung();

      held.bell = bell;
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(woke.contains(&Woke::Player), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(music) = playing();

      held.music = music;
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    woke.contains(&Woke::Surfaces) || looked == Looked::Again,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok((workspaces, front)) = walked();

      held.workspaces = workspaces;
      held.front = front;
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Settling {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Looked {
  Again,
  Not,
}

fn due(item: StatusItem, dwindling: &Watching) -> Result<Option<Instant>, Never> {
  let Ok(usual) = watch::tick(item);
  let every = match item {
    StatusItem::Battery => {
      let checked_value = dwindling.quick_poll()?;
      checked_value
    }
    .or(usual),
    StatusItem::Bluetooth | StatusItem::Network | StatusItem::Sound => usual,
  };
  let now = Instant::now();

  Ok(every.map(|every_2| match now.checked_add(every_2) {
    Some(due) => due,
    None => now,
  }))
}

fn heard_in() -> Result<Vec<PathBuf>, Never> {
  let Ok(progress) = updating::at();
  let tab = match console_onscreen::note() {
    Ok(note) => Some(note),
    Err(fault) => {
      eprintln!("console-bar: which tab is in front will not be heard: {fault}");

      None
    }
  };

  Ok(
    [Some(progress), tab]
      .into_iter()
      .flatten()
      .filter_map(|file| file.parent().map(Path::to_path_buf))
      .collect(),
  )
}

fn watch_folders(hearing: &OwnedFd, folders: &[PathBuf]) -> Result<(), Never> {
  let asked = WatchFlags::CLOSE_WRITE
    | WatchFlags::CREATE
    | WatchFlags::DELETE
    | WatchFlags::DELETE_SELF
    | WatchFlags::MOVED_FROM
    | WatchFlags::MOVED_TO
    | WatchFlags::MOVE_SELF
    | WatchFlags::ONLYDIR;

  for folder in folders {
    'over_ancestors: for ancestor in folder.ancestors() {
      match inotify::add_watch(hearing, ancestor, asked) {
        Ok(_watched) => break 'over_ancestors,
        Err(Errno::NOENT | Errno::NOTDIR) => continue 'over_ancestors,
        Err(fault) => {
          eprintln!(
            "console-bar: could not watch {}: {fault}",
            ancestor.display()
          );

          break 'over_ancestors;
        }
      }
    }
  }

  Ok(())
}

fn heard_again(hearing: &OwnedFd) -> Result<FileEvents, Never> {
  let mut buffer = [0_u8; 4096];
  let mut events = FileEvents::Idle;

  for _ in 0..16 {
    match rustix::io::read(hearing, &mut buffer) {
      Ok(0) | Err(Errno::AGAIN) => break,
      Ok(_events) => events = FileEvents::Received,
      Err(Errno::INTR) => continue,
      Err(fault) => {
        eprintln!("console-bar: could not read file changes: {fault}");

        break;
      }
    }
  }

  Ok(events)
}

fn endings(started: &[Started]) -> Result<Vec<OwnedFd>, Never> {
  Ok(
    started
      .iter()
      .filter_map(
        |Started {
           panel: _,
           program: one,
         }| {
          let Ok(id) = one.id();
          let Ok(raw) = fitted::<u32, i32>(id);

          match Pid::from_raw(raw) {
            Some(pid) => match pidfd_open(pid, PidfdFlags::empty()) {
              Ok(ending) => Some(ending),
              Err(fault) => {
                eprintln!(
                  "console-bar: {id} will be reaped when something else wakes the bar: {fault}"
                );

                None
              }
            },
            None => None,
          }
        },
      )
      .collect(),
  )
}

fn waiting(
  readings: &[Tracked],
  filling: Filling,
  settling: Settling,
) -> Result<Duration, Never> {
  match settling {
    Settling::Yes => return Ok(GATHERING),
    Settling::No => {}
  }

  match filling {
    Filling::At(_) => return Ok(FOLLOWING),
    Filling::None => {}
  }

  let now = Instant::now();
  let soonest = readings.iter().filter_map(|one| one.due).min();
  let Ok(minute) = clock::until_the_minute_turns();

  let until = match soonest {
    Some(soonest_2) => soonest_2.saturating_duration_since(now).min(minute),
    None => minute,
  };

  Ok(until.clamp(Duration::from_millis(1), AT_MOST))
}

fn again(
  woke: &BTreeSet<Woke>,
  readings: &mut [Tracked],
  dwindling: &mut Watching,
) -> Result<Looked, Never> {
  let mut any = Looked::Not;

  for one in readings.iter_mut() {
    let Ok(mine) = wakes(one.item);

    match ::core::primitive::bool::then_some(woke.contains(&mine), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(reading) = read_item(one.item, dwindling);
        let Ok(due) = due(one.item, dwindling);

        one.reading = reading;
        one.due = due;
        any = Looked::Again;
      }
      ::core::option::Option::None => {}
    }
  }

  Ok(any)
}

fn ticked(readings: &mut [Tracked], dwindling: &mut Watching) -> Result<(), Never> {
  let now = Instant::now();

  for one in readings.iter_mut() {
    match ::core::primitive::bool::then_some(one.due.is_some_and(|due| due <= now), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(reading) = read_item(one.item, dwindling);
        let Ok(due) = due(one.item, dwindling);

        one.reading = reading;
        one.due = due;
      }
      ::core::option::Option::None => {}
    }
  }

  Ok(())
}

fn read_item(item: StatusItem, dwindling: &mut Watching) -> Result<Reading, Never> {
  match item {
    StatusItem::Battery => {}
    StatusItem::Bluetooth | StatusItem::Network | StatusItem::Sound => return item.reading(),
  }

  let Ok(said) = console_battery::charge();
  let Ok(()) = dwindling.record(&said);

  console_status_bar::reading::battery(&said)
}

fn rung() -> Result<Reading, Never> {
  let Ok(whole) = console_notifications::serving::load_inbox();
  let Ok(quiet) = whole.effective_do_not_disturb();
  let Ok(waiting) = Waiting::of(&whole.waiting, quiet);

  notifications(waiting)
}

fn playing() -> Result<Reading, Never> {
  let Ok(requested) = console_application_music::player::playing();

  let requested_3 = match requested {
    Some(requested_2) => requested_2,
    None => return state::music(Paused::No, Playing::None),
  };

  let playing = match requested_3.sound {
    Sound::Stopped => Playing::None,
    Sound::Playing | Sound::Paused => Playing::Some,
  };
  let paused = match requested_3.sound {
    Sound::Paused => Paused::Yes,
    Sound::Playing | Sound::Stopped => Paused::No,
  };

  state::music(paused, playing)
}

fn listed(before: Vec<Layer>) -> Result<Vec<Layer>, Never> {
  match console_compositor::ask(console_compositor::Layers) {
    Ok(layers) => Ok(layers),
    Err(why) => {
      eprintln!("console-bar: {why}");

      Ok(before)
    }
  }
}

fn on_screen(layers: &[Layer]) -> Result<Vec<Panel>, Never> {
  let tab = match console_onscreen::tab() {
    Ok(tab) => tab,
    Err(_nothing_has_said_which_tab_is_in_front) => None,
  };

  panels::on_screen(layers, tab.as_deref())
}

fn walked() -> Result<(Vec<Workspace>, Option<i64>), Never> {
  let there = match console_compositor::ask(console_compositor::Workspaces) {
    Ok(there) => there,
    Err(why) => {
      eprintln!("console-bar: {why}");

      Vec::new()
    }
  };
  let front = match console_compositor::ask(console_compositor::ActiveWorkspace) {
    Ok(front) => front.map(|workspace| workspace.id),
    Err(_the_compositor_would_not_say_which_one_is_in_front) => None,
  };

  Ok((there, front))
}

fn painted(surface: &mut Surface, drawn: &Rendered) -> Result<(), Never> {
  let shapes = drawn.shapes.clone();
  let points = drawn.room;
  let put = surface.draw(move |pixels, device, _scale| {
    match onto(pixels, Frame { device, points }, &shapes) {
      Ok(()) => {}
      Err(why) => eprintln!("console-bar: {why}"),
    }

    Ok(())
  });

  match put {
    Ok(()) => {}
    Err(why) => eprintln!("console-bar: {why}"),
  }

  Ok(())
}

fn tapped(
  surface: &mut Surface,
  drawn: &Rendered,
  selection: &mut Selection<BarAction>,
) -> Result<Vec<BarAction>, Never> {
  let Ok(pointer_events) = surface.pointer_events();
  let mut requested = Vec::new();

  for event in pointer_events {
    let at = match event {
      PointerEvent::Up
      | PointerEvent::Left
      | PointerEvent::Moved { .. }
      | PointerEvent::Scrolled { .. }
      | PointerEvent::Pinched { .. } => continue,
      PointerEvent::Down { at } => at,
    };
    *selection = Selection::Idle;
    let Ok(across) = toward_zero_i32(at.0);
    let Ok(down) = toward_zero_i32(at.1);
    let Ok(on) = drawn.on(Point { x: across, y: down });

    match on {
      Some(action) => requested.push(action),
      None => {}
    }
  }

  Ok(requested)
}

struct Started {
  panel: Option<Panel>,
  program: Detached,
}

enum Requested {
  BuiltIn(BarAction),
  Applet(Applet),
}

fn queue(
  action: BarAction,
  held: &mut BarState,
  applets: &[Applet],
  requested: &mut Vec<Requested>,
) -> Result<(), Never> {
  match action {
    BarAction::Applet(position) => {
      let Ok(offset) = index(position);

      match applets.get(offset) {
        Some(applet) => requested.push(Requested::Applet(applet.clone())),
        None => {}
      }
    }
    BarAction::Calendar
    | BarAction::Launcher
    | BarAction::Keyboard
    | BarAction::Clipboard
    | BarAction::Workspace(_)
    | BarAction::Settings(_)
    | BarAction::Music
    | BarAction::Notifications
    | BarAction::Menu(_)
    | BarAction::Volume(_)
    | BarAction::Power(_) => {
      let Ok(asked) = held.pressed(action);
      requested.extend(asked.into_iter().map(Requested::BuiltIn));
    }
  }

  Ok(())
}

fn begun(requested: &mut Vec<Requested>, started: &mut Vec<Started>) -> Result<(), Never> {
  for request in requested.drain(..) {
    let (panel, begun) = match request {
      Requested::BuiltIn(action) => {
        let Ok(panel) = Panel::of(action);
        let Ok(begun) = doing(action);

        (panel, begun)
      }
      Requested::Applet(applet) => {
        let Ok(command) = applet.command();
        let started_3 = match command {
          Some(command_2) => {
            let Ok(started_2) = start(command_2);
            started_2
          }
          None => None,
        };

        (None, started_3)
      }
    };

    match begun {
      Some(program) => {
        started.push(Started { panel, program });
      }
      ::core::option::Option::None => {}
    }
  }

  Ok(())
}

fn reaped(started: Vec<Started>) -> Result<(Vec<Started>, Vec<Panel>), Never> {
  let mut running = Vec::new();
  let mut ended = Vec::new();

  for Started { panel, mut program } in started {
    let Ok(still) = program.still();

    match still {
      Still::Running => running.push(Started { panel, program }),
      Still::Ended => ended.extend(panel),
    }
  }

  Ok((running, ended))
}

fn doing(action: BarAction) -> Result<Option<Detached>, Never> {
  let starting = match action {
    BarAction::Launcher => {
      let Ok(command) = InternalProgram::Launcher.command();

      command
    }
    BarAction::Keyboard => {
      let Ok(command) = InternalProgram::KeyboardToggle.command();

      command
    }
    BarAction::Clipboard => {
      let Ok(command) = InternalProgram::Clipboard.command();

      command
    }
    BarAction::Music => {
      let Ok(command) = InternalProgram::MusicPanel.command();

      command
    }
    BarAction::Notifications => {
      let Ok(command) = InternalProgram::NotificationsPanel.command();

      command
    }
    BarAction::Calendar => {
      let Ok(command) = InternalProgram::CalendarPanel.command();

      command
    }
    BarAction::Settings(section) => {
      let Ok(mut command) = InternalProgram::SettingsPanel.command();
      let Ok(tab) = section.title();

      let _ = command.arg(tab);

      command
    }
    BarAction::Volume(volume) => {
      let Ok(command) = volume.command();

      command
    }
    BarAction::Power(power) => {
      let Ok(command) = power.command();

      command
    }
    BarAction::Workspace(id) => return switched(id),
    BarAction::Menu(_) | BarAction::Applet(_) => return Ok(None),
  };

  start(starting)
}

fn start(mut starting: std::process::Command) -> Result<Option<Detached>, Never> {
  let Ok(()) = console_response_times::pressed_here(&mut starting);

  let _ = starting.stdout(Stdio::null()).stderr(Stdio::null());

  Ok(match console_program_lifetime::let_go(&mut starting) {
    Ok(started) => Some(started),
    Err(why) => {
      eprintln!("console-bar: {why}");

      None
    }
  })
}

fn switched(id: i64) -> Result<Option<Detached>, Never> {
  let Ok(_done) = console_compositor::Action::MoveWorkspace {
    target: WorkspaceTarget::Number(id),
    carrying: Carrying::None,
  }
  .dispatch();

  Ok(None)
}

fn screen() -> Result<Size<u32>, Cannot> {
  let monitors = match console_compositor::ask(console_compositor::Monitors) {
    Ok(monitors) => monitors,
    Err(why) => {
      eprintln!("console-bar: {why}");

      return Err(Cannot::Screenless);
    }
  };
  let logical = monitors.iter().find_map(|monitor| match monitor.logical() {
    Ok(Some(size)) => Some(size),
    Ok(None) | Err(_) => None,
  });

  match logical {
    Some(logical_2) => Ok(logical_2),
    None => Err(Cannot::Screenless),
  }
}

fn sources() -> std::io::Result<Vec<waking::Source>> {
  let mut sources = Vec::new();

  for item in state::ALONG {
    let (say, heard) = sync_channel(1);
    let Ok(()) = watch::subscribe(item, say);
    let Ok(source) = wakes(item);
    let forwarded = waking::forwarded(source, heard)?;

    sources.push(forwarded);
  }

  let (said, heard) = sync_channel(1);
  let Ok(()) = watch::telling_surfaces(said);
  let surfaces = waking::forwarded(Woke::Surfaces, heard)?;
  sources.push(surfaces);

  let (said_notifications, heard_notifications) = sync_channel(1);
  let Ok(()) = watch::telling_notifications(said_notifications);
  let notifications = waking::forwarded(Woke::Notifications, heard_notifications)?;
  sources.push(notifications);

  let (said_player, heard_player) = sync_channel(1);
  let Ok(()) = console_events::again::about(
    &EventGroup::Player,
    console_application_music::player::worth_asking_after,
    said_player,
  );
  let player = waking::forwarded(Woke::Player, heard_player)?;
  sources.push(player);

  Ok(sources)
}

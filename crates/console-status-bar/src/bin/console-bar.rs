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
//! source turns that into a byte down a pipe the loop is already polling
//! beside the compositor's socket. What the byte carries is which source it
//! was, so a workspace switch does not re-run `wpctl`. When something has just
//! happened the next wait is short rather than long, which is how a burst --
//! and a layer opening is always several lines -- is absorbed in one extra pass
//! instead of one pass each.
//!
//! **The screen's height is read once and the bar's width is asked every pass.**
//! How deep the bar is, how big its letters are and how much padding a thumb
//! lands on are all shares of the height, and a height that changed under a
//! running layer surface would need the reserved zone given back and taken
//! again. It never has to be: `console-scale` restarts this program in the same
//! transaction as it restarts the home screen, for the reason `docs/screen.md`
//! gives, so a density change arrives as a new process. The width is different
//! -- the compositor hands it back every time it configures the surface -- and
//! that is what the arrangement is laid out across.
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
use std::io::Write;
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{ExitCode, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use console_compositor::{Carrying, Layer, Request, Workspace};
use console_core_color::palette::{self, PaletteError, WearingError};
use console_core_geometry::{Point, Size};
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_i32};
use console_core_internal_programs::InternalProgram;
use console_draw_painting::{Frame, onto};
use console_draw_surface::standing::{
    Anchor, Closed, Keyboard, Margin, Room, Under, Wanted,
};
use console_draw_surface::{SurfaceError, PointerEvent, Surface};
use console_music::player::Sound;
use console_program_contract::EventGroup;
use console_status_bar::clock;
use console_status_bar::dwindling::Watching;
use console_program_lifetime::{Detached, Still};
use console_status_bar::panels::{self, Panel, PanelEvent, Panels};
use console_status_bar::state::{self, BarState, Paused, Playing};
use console_status_bar::notifications::{Waiting, notifications};
use console_status_bar::reading::{Reading, StatusItem};
use console_status_bar::measuring::sized;
use console_status_bar::showing::{self, BarAction, Rendered, Filling, Fitting, Wearing};
use console_notifications::updating;
use console_status_bar::watch;
use console_waiting::latch;
use rustix::fs::inotify::{self, CreateFlags, WatchFlags};
use rustix::process::{Pid, PidfdFlags, pidfd_open};

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

fn drawing() -> Result<(), Cannot> {
    let spent = palette::spent()?;
    let wearing = Wearing::out_of(&spent)?;
    let mut surface = Surface::connect()?;
    let waking = latch::pipe().map_err(Cannot::Pipe)?;
    let hearing = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK).map_err(Cannot::Deaf)?;
    let Ok(folders) = heard_in();

    let seen = Arc::new(Mutex::new(BTreeSet::new()));
    let saying = Arc::new(waking.saying);
    let Ok(()) = sources(&seen, &saying);

    let whole = screen()?;
    let Ok(fitting) = Fitting::of_em();

    raised(&mut surface, fitting)?;

    let mut dwindling = Watching::default();
    let Ok(readings) = first_readings(&mut dwindling);
    let Ok(when) = clock::current();
    let Ok(layers) = listed(Vec::new());
    let Ok(held) = first_held(&readings, &layers);
    let turning = Turning {
        surface,
        dwindling,
        readings,
        held,
        was: when,
        settling: Settling::No,
        last: None,
        requested: Vec::new(),
        started: Vec::new(),
        layers,
    };
    let around = Around {
        wearing: &wearing,
        whole,
        fitting,
        woken: &waking.waiting,
        hearing: &hearing,
        folders: &folders,
        seen: &seen,
    };

    match console_core_iteration::iterate(turning, |turning| turned(turning, around)) {
        Ok(drawn) => drawn,
        Err(_endless) => Ok(()),
    }
}

struct Turning {
    surface: Surface,
    dwindling: Watching,
    readings: Vec<Tracked>,
    held: BarState,
    was: clock::Standing,
    settling: Settling,
    last: Option<Rendered>,
    requested: Vec<BarAction>,
    started: Vec<Started>,
    layers: Vec<Layer>,
}

#[derive(Clone, Copy)]
struct Around<'a> {
    wearing: &'a Wearing,
    whole: Size<u32>,
    fitting: Fitting,
    woken: &'a OwnedFd,
    hearing: &'a OwnedFd,
    folders: &'a [PathBuf],
    seen: &'a Arc<Mutex<BTreeSet<Woke>>>,
}

fn turned(turning: Turning, around: Around<'_>) -> Result<Step<Turning, Result<(), Cannot>>, Never> {
    let Around { wearing, whole, fitting, woken, hearing, folders, seen } = around;
    let waking_fd = woken;
    let woken = woken.as_fd();
    let Turning {
        mut surface,
        mut dwindling,
        mut readings,
        mut held,
        mut was,
        mut settling,
        mut last,
        mut requested,
        mut started,
        mut layers,
    } = turning;

    let Ok(progress) = updating::progress();
    let filling = match &progress {
        Some(progress) => Filling::At(progress.permille),
        None => Filling::None,
    };
    let Ok(layout) = held.layout(filling);
    let Ok(bar) = sized(&layout, fitting);
    let logical = match surface.logical() {
        Ok(Some(logical)) => logical,
        Ok(None) | Err(_) => whole,
    };
    let Ok(drawn) =
        showing::along(&bar, wearing, Size { width: logical.width, height: whole.height });

    match last.as_ref() == Some(&drawn) {
        true => {}
        false => {
            let Ok(()) = painted(&mut surface, &drawn);

            last = Some(drawn.clone());
        }
    }

    let Ok(()) = begun(&mut requested, &mut started);

    let Ok(closed) = surface.closed();

    match closed {
        Closed::Yes => return Ok(Step::Halt(Ok(()))),
        Closed::No => {}
    }

    let Ok(until) = waiting(&readings, filling, settling);

    let Ok(()) = watch_folders(hearing, folders);
    let Ok(ending) = endings(&started);
    let mut also = vec![woken, hearing.as_fd()];

    also.extend(ending.iter().map(AsFd::as_fd));

    match surface.wait(&also, Some(until)) {
        Ok(_woke) => {},
        Err(fault) => return Ok(Step::Halt(Err(Cannot::from(fault)))),
    }

    let Ok(tapped) = tapped(&mut surface, &drawn);

    for action in tapped {
        let Ok(asked) = held.pressed(action);

        requested.extend(asked);
    }

    let Ok(()) = latch::drain(waking_fd);
    let Ok(()) = heard_again(hearing);
    let Ok((still, ended)) = reaped(started);

    started = still;

    let Ok(woke) = take_woken(seen);

    settling = match woke.is_empty() {
        true => Settling::No,
        false => Settling::Yes,
    };

    let Ok(looked) = again(&woke, &mut readings, &mut dwindling);
    let Ok(()) = ticked(&mut readings, &mut dwindling);
    let Ok(()) = refreshed(&woke, looked, &mut held);

    match woke.contains(&Woke::Surfaces) || looked == Looked::Again || !ended.is_empty() {
        true => {
            let Ok(asked) = listed(layers);

            layers = asked;
        }
        false => {}
    }

    for panel in ended {
        let Ok(_starts_nothing) = held.told(PanelEvent::Ended(panel));
    }

    let Ok(up) = on_screen(&layers);
    let Ok(_starts_nothing) = held.told(PanelEvent::Shown(up));

    let Ok(now) = clock::current();

    match now == was {
        true => {}
        false => {
            let Ok(said) = clock::now();

            held.clock = said;
            was = now;
        }
    }

    held.readings = readings.iter().map(|one| (one.item, one.reading.clone())).collect();
    Ok(Step::Again(Turning {
        surface, dwindling, readings, held, was, settling, last, requested, started, layers,
    }))
}

fn raised(surface: &mut Surface, fitting: Fitting) -> Result<(), Cannot> {
    let Ok(tall) = fitting.height();
    let Ok(margin) = Margin::none();

    surface.show(&Wanted {
        namespace: showing::WHO.to_string(),
        anchor: Anchor::Top,
        size: Size { width: 0, height: tall },
        margin,
        keyboard: Keyboard::Declines,
        room: Room::Reserves,
        under: Under::Anything,
    })?;

    Ok(())
}

fn first_readings(dwindling: &mut Watching) -> Result<Vec<Tracked>, Never> {
    let mut readings = Vec::new();

    for item in state::ALONG {
        let Ok(reading) = read_item(item, dwindling);
        let Ok(due) = due(item);

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
        readings: readings.iter().map(|one| (one.item, one.reading.clone())).collect(),
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
    match woke.contains(&Woke::Notifications) {
        true => {
            let Ok(bell) = rung();

            held.bell = bell;
        }
        false => {}
    }

    match woke.contains(&Woke::Player) {
        true => {
            let Ok(music) = playing();

            held.music = music;
        }
        false => {}
    }

    match woke.contains(&Woke::Surfaces) || looked == Looked::Again {
        true => {
            let Ok((workspaces, front)) = walked();

            held.workspaces = workspaces;
            held.front = front;
        }
        false => {}
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

fn due(item: StatusItem) -> Result<Option<Instant>, Never> {
    let Ok(every) = watch::tick(item);
    let now = Instant::now();

    Ok(every.map(|every| match now.checked_add(every) {
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

    Ok([Some(progress), tab]
        .into_iter()
        .flatten()
        .filter_map(|file| file.parent().map(Path::to_path_buf))
        .collect())
}

fn watch_folders(hearing: &OwnedFd, folders: &[PathBuf]) -> Result<(), Never> {
    let asked = WatchFlags::CLOSE_WRITE
        | WatchFlags::CREATE
        | WatchFlags::DELETE
        | WatchFlags::MOVED_TO
        | WatchFlags::ONLYDIR;

    for folder in folders {
        let nearest = folder.ancestors().find(|one| one.is_dir());

        match nearest {
            Some(nearest) => match inotify::add_watch(hearing, nearest, asked) {
                Ok(_watched) => {}
                Err(_the_folder_went_between_being_found_and_being_watched) => {}
            },
            None => {}
        }
    }

    Ok(())
}

fn heard_again(hearing: &OwnedFd) -> Result<(), Never> {
    let _drained = std::iter::from_fn(|| {
        let mut heard = [0_u8; 4096];

        match rustix::io::read(hearing, &mut heard) {
            Ok(_more_was_said) => Some(()),
            Err(_nothing_more_was_said) => None,
        }
    })
    .count();

    Ok(())
}

fn endings(started: &[Started]) -> Result<Vec<OwnedFd>, Never> {
    Ok(started
        .iter()
        .filter_map(|Started { panel: _, program: one }| {
            let Ok(id) = one.id();
            let Ok(raw) = fitted::<u32, i32>(id);

            match Pid::from_raw(raw) {
                Some(pid) => match pidfd_open(pid, PidfdFlags::empty()) {
                    Ok(ending) => Some(ending),
                    Err(fault) => {
                        eprintln!("console-bar: {id} will be reaped when something else wakes the bar: {fault}");

                        None
                    }
                },
                None => None,
            }
        })
        .collect())
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
        Some(soonest) => soonest.saturating_duration_since(now).min(minute),
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

        match woke.contains(&mine) {
            true => {
                let Ok(reading) = read_item(one.item, dwindling);
                let Ok(due) = due(one.item);

                one.reading = reading;
                one.due = due;
                any = Looked::Again;
            }
            false => {}
        }
    }

    Ok(any)
}

fn ticked(readings: &mut [Tracked], dwindling: &mut Watching) -> Result<(), Never> {
    let now = Instant::now();

    for one in readings.iter_mut() {
        match one.due.is_some_and(|due| due <= now) {
            true => {
                let Ok(reading) = read_item(one.item, dwindling);
                let Ok(due) = due(one.item);

                one.reading = reading;
                one.due = due;
            }
            false => {}
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
    let Ok(waiting) = Waiting::of(&whole.waiting, whole.do_not_disturb);

    notifications(waiting)
}

fn playing() -> Result<Reading, Never> {
    let Ok(requested) = console_music::player::playing();

    let requested = match requested {
        Some(requested) => requested,
        None => return state::music(Paused::No, Playing::None),
    };

    let playing = match requested.sound {
        Sound::Stopped => Playing::None,
        Sound::Playing | Sound::Paused => Playing::Some,
    };
    let paused = match requested.sound {
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
        Ok(front) => {
            front.map(|workspace| workspace.id)
        }
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

fn tapped(surface: &mut Surface, drawn: &Rendered) -> Result<Vec<BarAction>, Never> {
    let Ok(pointer_events) = surface.pointer_events();
    let mut requested = Vec::new();

    for event in pointer_events {
        let at = match event {
            PointerEvent::Up | PointerEvent::Left | PointerEvent::Moved { .. } | PointerEvent::Scrolled { .. } | PointerEvent::Pinched { .. } => continue,
            PointerEvent::Down { at } => at,
        };
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
    panel: Panel,
    program: Detached,
}

fn begun(requested: &mut Vec<BarAction>, started: &mut Vec<Started>) -> Result<(), Never> {
    for action in requested.drain(..) {
        let Ok(begun) = doing(action);
        let Ok(panel) = Panel::of(action);

        match (panel, begun) {
            (Some(panel), Some(program)) => started.push(Started { panel, program }),
            (Some(_), None) | (None, _) => {}
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
            Still::Ended => ended.push(panel),
        }
    }

    Ok((running, ended))
}

fn doing(action: BarAction) -> Result<Option<Detached>, Never> {
    let mut starting = match action {
        BarAction::Launcher => {
            let Ok(command) = InternalProgram::Launcher.command();

            command
        }
        BarAction::Keyboard => {
            let Ok(command) = InternalProgram::KeyboardToggle.command();

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
        BarAction::Settings(item) => {
            let Ok(mut command) = InternalProgram::SettingsPanel.command();
            let Ok(tab) = item.tab();

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
        BarAction::Menu(_) => return Ok(None),
    };

    let Ok(()) = console_response_times::pressed_here(&mut starting);

    let _ = starting.stdout(Stdio::null()).stderr(Stdio::null());

    match console_program_lifetime::let_go(&mut starting) {
        Ok(started) => Ok(Some(started)),
        Err(why) => {
            eprintln!("console-bar: {why}");

            Ok(None)
        }
    }
}

fn switched(id: i64) -> Result<Option<Detached>, Never> {
    let Ok(lua) = console_compositor::onto(&id.to_string(), Carrying::None);
    let Ok(_done) = console_compositor::request(Request::Dispatch, &lua);

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
        Some(logical) => Ok(logical),
        None => Err(Cannot::Screenless),
    }
}

fn take_woken(seen: &Arc<Mutex<BTreeSet<Woke>>>) -> Result<BTreeSet<Woke>, Never> {
    Ok(match seen.lock() {
        Ok(mut seen) => std::mem::take(&mut seen),
        Err(_nothing_is_telling_this_bar_anything) => BTreeSet::new(),
    })
}

fn sources(
    seen: &Arc<Mutex<BTreeSet<Woke>>>,
    saying: &Arc<std::fs::File>,
) -> Result<(), Never> {
    for item in state::ALONG {
        let (say, heard) = channel();
        let Ok(()) = watch::subscribe(item, say);
        let Ok(mine) = wakes(item);
        let Ok(()) = forwarded(mine, heard, seen, saying);
    }

    let (say, heard) = channel();
    let Ok(()) = watch::telling_surfaces(say);
    let Ok(()) = forwarded(Woke::Surfaces, heard, seen, saying);

    let (say, heard) = channel();
    let Ok(()) = watch::telling_notifications(say);
    let Ok(()) = forwarded(Woke::Notifications, heard, seen, saying);

    let (say, heard) = channel();
    let Ok(()) =
        console_events::again::about(&EventGroup::Player, console_music::player::worth_asking_after, say);
    let Ok(()) = forwarded(Woke::Player, heard, seen, saying);

    Ok(())
}

fn forwarded(
    woke: Woke,
    heard: Receiver<()>,
    seen: &Arc<Mutex<BTreeSet<Woke>>>,
    saying: &Arc<std::fs::File>,
) -> Result<(), Never> {
    let seen = Arc::clone(seen);
    let saying = Arc::clone(saying);

    console_program_lifetime::threads::let_go(std::thread::spawn(move || {
        for () in heard.iter() {
            match seen.lock() {
                Ok(mut seen) => {
                    let _ = seen.insert(woke);
                }
                Err(_no_one_is_reading_these) => return,
            }

            let mut down = saying.as_ref();

            match down.write_all(&[1]) {
                Ok(()) => {}
                Err(_the_loop_has_gone) => return,
            }
        }
    }))
}

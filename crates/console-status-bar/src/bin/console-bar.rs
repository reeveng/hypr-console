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
//! thing that is moving. That an apply has *started* is told rather than
//! polled: the engine sends `SIGRTMIN+4`, which this blocks and reads off a
//! `signalfd` in the same `poll` as the compositor's socket, so an idle bar
//! waits a minute at a time and a filling one is still on the first frame of
//! it. Every other wait here is until something says so.
//!
//! **Everything a tap starts is reaped, and a child ending wakes the loop.**
//! Only the last program a tap started used to be held, for the lit icon's
//! sake, and every one before it was dropped without anyone waiting on it. A
//! hundred taps on the device was a hundred dead entries under the bar, one per
//! panel, for as long as the session lasted. They are all held now and let go
//! of as they end, and `SIGCHLD` comes down the same `signalfd` as an apply,
//! because a panel that ends between two things happening on the screen would
//! otherwise lie there until the clock next turned over.

use std::collections::BTreeSet;
use std::io::Write;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::process::{ExitCode, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use console_compositor::{Query, Carrying, Request, Workspace};
use console_core_color::palette::{self, PaletteError, WearingError};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_i32};
use console_core_internal_programs::InternalProgram;
use console_draw_painting::{Frame, Run, measured, onto};
use console_draw_surface::standing::{
    Anchor, Closed, Keyboard, Margin, Room, Under, Wanted,
};
use console_draw_surface::{SurfaceError, PointerEvent, Surface};
use console_music::player::Sound;
use console_onscreen::Up;
use console_program_contract::Topic;
use console_status_bar::clock;
use console_status_bar::dwindling::Watching;
use console_program_lifetime::{Detached, Still};
use console_status_bar::state::{self, BarState, Open, Paused, Playing, Promise};
use console_status_bar::notifications::{Waiting, notifications};
use console_status_bar::reading::{Reading, StatusItem};
use console_status_bar::showing::{
    self, Bar, BarAction, Rendered, Face, Filling, Fitting, Measured, Slot, Wearing,
};
use console_notifications::updating;
use console_status_bar::watch;
use console_waiting::woken;

const GATHERING: Duration = Duration::from_millis(120);

const FOLLOWING: Duration = Duration::from_millis(200);

const AT_MOST: Duration = Duration::from_secs(60);

#[derive(Debug)]
enum Cannot {
    Palette(WearingError),
    Pipe(std::io::Error),
    Deaf(std::io::Error),
    Screenless,
    Compositor(SurfaceError),
}

impl std::fmt::Display for Cannot {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cannot::Palette(why) => write!(to, "{why}"),
            Cannot::Pipe(why) => write!(to, "nothing to be woken down: {why}"),
            Cannot::Deaf(why) => write!(to, "nothing to hear an apply on: {why}"),
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
    let waking = woken::pipe().map_err(Cannot::Pipe)?;
    let applying = listening().map_err(Cannot::Deaf)?;

    match console_onscreen::bar_started(std::process::id()) {
        Ok(()) => {},
        Err(fault) => eprintln!("console-bar: nothing can wake it but an apply: {fault}"),
    }

    let seen = Arc::new(Mutex::new(BTreeSet::new()));
    let saying = Arc::new(waking.saying);
    let Ok(()) = sources(&seen, &saying);

    let whole = screen()?;
    let Ok(fitting) = Fitting::of_em();

    raised(&mut surface, fitting)?;

    let mut dwindling = Watching::default();
    let Ok(mut readings) = first_readings(&mut dwindling);
    let Ok(when) = clock::standing();
    let Ok(mut held) = first_held(&readings);
    let mut was = when;
    let mut settling = Settling::No;
    let mut last: Option<Rendered> = None;
    let mut requested: Vec<BarAction> = Vec::new();
    let mut promised: Option<Promised> = None;
    let mut started: Vec<Detached> = Vec::new();
    let woken = waking.waiting.as_fd();

    loop {
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
            showing::along(&bar, &wearing, Size { width: logical.width, height: whole.height });

        match last.as_ref() == Some(&drawn) {
            true => {}
            false => {
                let Ok(()) = painted(&mut surface, &drawn);

                last = Some(drawn.clone());
            }
        }

        let Ok(()) = begun(&mut requested, &mut promised, &mut started);

        let Ok(closed) = surface.closed();

        match closed {
            Closed::Yes => return Ok(()),
            Closed::No => {}
        }

        let Ok(until) = waiting(&readings, filling, settling);

        surface.wait(&[woken, applying.as_fd()], Some(until))?;

        let Ok(()) = tapped(&mut surface, &drawn, &mut requested);

        for action in &requested {
            let Ok(()) = held.pressed(*action);

            promised = Some(Promised { action: *action, pressed: held.open.clone(), started: None });
        }

        let Ok(()) = woken::drained(&waking.waiting);
        let Ok(()) = told_again(applying.as_fd());
        let Ok(still) = console_program_lifetime::reaped(started);

        started = still;

        held.open.tab = match console_onscreen::tab() {
            Ok(tab) => tab,
            Err(_nothing_has_said_which_tab_is_in_front) => None,
        };
        let Ok(woke) = heard(&seen);

        settling = match woke.is_empty() {
            true => Settling::No,
            false => Settling::Yes,
        };

        let Ok(looked) = again(&woke, &mut readings, &mut dwindling);
        let Ok(()) = ticked(&mut readings, &mut dwindling);
        let Ok(()) = refreshed(&woke, looked, &mut held);
        let Ok(()) = kept(&mut promised, &mut held, &started);

        let Ok(now) = clock::standing();

        match now == was {
            true => {}
            false => {
                let Ok(said) = clock::now();

                held.clock = said;
                was = now;
            }
        }

        held.readings = readings.iter().map(|one| (one.item, one.reading.clone())).collect();
    }
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
        let Ok(reading) = taken(item, dwindling);
        let Ok(due) = due(item);

        readings.push(Tracked { item, reading, due });
    }

    Ok(readings)
}

fn first_held(readings: &[Tracked]) -> Result<BarState, Never> {
    let Ok(bell) = rung();
    let Ok(music) = playing();
    let Ok(open) = shown(Open {
        launcher: Up::NotThere,
        keyboard: Up::NotThere,
        music: Up::NotThere,
        notifications: Up::NotThere,
        calendar: Up::NotThere,
        settings: Up::NotThere,
        tab: None,
    });
    let Ok(said) = clock::now();
    let Ok((workspaces, front)) = walked();

    Ok(BarState {
        readings: readings.iter().map(|one| (one.item, one.reading.clone())).collect(),
        bell,
        music,
        clock: said,
        workspaces,
        front,
        open,
    })
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
            let Ok(open) = shown(held.open.clone());
            let Ok((workspaces, front)) = walked();

            held.open = open;
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

fn listening() -> Result<OwnedFd, std::io::Error> {
    let told = libc::SIGRTMIN().saturating_add(console_onscreen::WAKES_AT);

    // SAFETY: a mask on the stack, filled and applied by the calls that own it,
    // a descriptor the kernel opens for exactly the signals in it, and nothing
    // else holding it when it is handed to `OwnedFd`.
    unsafe {
        let mut mask: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut mask);
        libc::sigaddset(&mut mask, told);
        libc::sigaddset(&mut mask, libc::SIGCHLD);

        match libc::pthread_sigmask(libc::SIG_BLOCK, &mask, std::ptr::null_mut()) != 0 {
            true => return Err(std::io::Error::last_os_error()),
            false => {},
        }

        let heard = libc::signalfd(-1, &mask, libc::SFD_CLOEXEC | libc::SFD_NONBLOCK);

        match heard < 0 {
            true => Err(std::io::Error::last_os_error()),
            false => Ok(OwnedFd::from_raw_fd(heard)),
        }
    }
}

fn told_again(heard: BorrowedFd<'_>) -> Result<(), Never> {
    let Ok(whole) = fitted::<_, i64>(std::mem::size_of::<libc::signalfd_siginfo>());

    loop {
        // SAFETY: a struct this frame owns, filled by the kernel or not at all,
        // on a descriptor that never blocks.
        let Ok(read) = fitted::<_, i64>(unsafe {
            let mut said: libc::signalfd_siginfo = std::mem::zeroed();

            libc::read(heard.as_raw_fd(), std::ptr::from_mut(&mut said).cast(), std::mem::size_of::<libc::signalfd_siginfo>())
        });

        match read == whole {
            true => {},
            false => return Ok(()),
        }
    }
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
                let Ok(reading) = taken(one.item, dwindling);
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
                let Ok(reading) = taken(one.item, dwindling);
                let Ok(due) = due(one.item);

                one.reading = reading;
                one.due = due;
            }
            false => {}
        }
    }

    Ok(())
}

fn taken(item: StatusItem, dwindling: &mut Watching) -> Result<Reading, Never> {
    match item {
        StatusItem::Battery => {}
        StatusItem::Bluetooth | StatusItem::Network | StatusItem::Sound => return item.reading(),
    }

    let Ok(said) = console_battery::charge();
    let Ok(()) = dwindling.seen(&said);

    console_status_bar::reading::battery(&said)
}

fn rung() -> Result<Reading, Never> {
    let Ok(whole) = console_notifications::serving::held();
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

fn shown(before: Open) -> Result<Open, Never> {
    let screens = match console_compositor::query(Query::Layers) {
        Ok(console_compositor::Answer::Layers(screens)) => screens,
        Ok(_not_what_was_asked) => return Ok(before),
        Err(why) => {
            eprintln!("console-bar: {why}");

            return Ok(before);
        }
    };
    let up = |namespace: &str| {
        let Ok(up) = console_onscreen::up(&screens, namespace);

        up
    };
    let tab = match console_onscreen::tab() {
        Ok(tab) => tab,
        Err(_nothing_has_said_which_tab_is_in_front) => None,
    };

    let Ok(launcher) = InternalProgram::Launcher.name();
    let Ok(music) = InternalProgram::MusicPanel.name();
    let Ok(notifications) = InternalProgram::NotificationsPanel.name();
    let Ok(calendar) = InternalProgram::CalendarPanel.name();
    let Ok(settings) = InternalProgram::SettingsPanel.name();

    Ok(Open {
        launcher: up(launcher),
        keyboard: up(console_onscreen::KEYBOARD),
        music: up(music),
        notifications: up(notifications),
        calendar: up(calendar),
        settings: up(settings),
        tab,
    })
}

fn walked() -> Result<(Vec<Workspace>, Option<i64>), Never> {
    let there = match console_compositor::query(Query::Workspaces) {
        Ok(console_compositor::Answer::Workspaces(there)) => there,
        Ok(_not_what_was_asked) => Vec::new(),
        Err(why) => {
            eprintln!("console-bar: {why}");

            Vec::new()
        }
    };
    let front = match console_compositor::query(Query::ActiveWorkspace) {
        Ok(console_compositor::Answer::ActiveWorkspace(front)) => {
            front.map(|workspace| workspace.id)
        }
        Ok(_not_what_was_asked) => None,
        Err(_the_compositor_would_not_say_which_one_is_in_front) => None,
    };

    Ok((there, front))
}

fn sized(layout: &showing::Layout, fitting: Fitting) -> Result<Bar, Never> {
    let Ok(left) = every(&layout.left, fitting);
    let Ok(middle) = every(&layout.middle, fitting);
    let Ok(right) = every(&layout.right, fitting);

    Ok(Bar { left, middle, right, filling: layout.filling })
}

fn every(slots: &[Slot], fitting: Fitting) -> Result<Vec<Measured>, Never> {
    let mut measured = Vec::new();

    for slot in slots {
        let mut runs = Vec::new();

        for span in &slot.spans {
            let Ok(one) = measure(span.text.as_str(), span.face, fitting);

            runs.push(one);
        }

        measured.push(Measured { slot: slot.clone(), runs });
    }

    Ok(measured)
}

fn measure(text: &str, face: Face, fitting: Fitting) -> Result<Size<u32>, Never> {
    let Ok(font) = fitting.font(face);
    let Ok(weight) = face.weight();

    measured(Run { said: text, weight, width: u32::MAX }, &font)
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

fn tapped(surface: &mut Surface, drawn: &Rendered, requested: &mut Vec<BarAction>) -> Result<(), Never> {
    let Ok(pointer_events) = surface.pointer_events();

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

    Ok(())
}

struct Promised {
    action: BarAction,
    pressed: Open,
    started: Option<u32>,
}

fn begun(requested: &mut Vec<BarAction>, promised: &mut Option<Promised>, started: &mut Vec<Detached>) -> Result<(), Never> {
    for action in requested.drain(..) {
        let Ok(begun) = doing(action);
        let pid = match &begun {
            Some(one) => {
                let Ok(pid) = one.id();

                Some(pid)
            }
            None => None,
        };

        match promised.as_mut() {
            Some(promise) => match promise.action == action {
                true => promise.started = pid,
                false => {},
            },
            None => {},
        }

        started.extend(begun);
    }

    Ok(())
}

fn kept(promised: &mut Option<Promised>, held: &mut BarState, started: &[Detached]) -> Result<(), Never> {
    let promise = match promised.as_mut() {
        Some(promise) => promise,
        None => return Ok(()),
    };

    let still = match promise.started {
        Some(pid) => match started.iter().any(|one| one.id() == Ok(pid)) {
            true => Still::Running,
            false => Still::Ended,
        },
        None => Still::Running,
    };

    match still {
        Still::Ended => {
            *promised = None;

            return Ok(());
        }
        Still::Running => {},
    }

    let Ok((open, promise)) = held.open.promised(promise.action, &promise.pressed);

    held.open = open;

    match promise {
        Promise::Fulfilled => *promised = None,
        Promise::Waiting => {},
    }

    Ok(())
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
        BarAction::Workspace(id) => return switched(id),
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
    let monitors = match console_compositor::query(Query::Monitors) {
        Ok(console_compositor::Answer::Monitors(monitors)) => monitors,
        Ok(_not_what_was_asked) => return Err(Cannot::Screenless),
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

fn heard(seen: &Arc<Mutex<BTreeSet<Woke>>>) -> Result<BTreeSet<Woke>, Never> {
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
        console_events::again::about(&Topic::Player, console_music::player::worth_asking_after, say);
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

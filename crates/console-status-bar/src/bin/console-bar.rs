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
//! last thing each source said, which is a cache of somebody else's facts
//! rather than a state anything decides from.
//!
//! **Nothing here decides anything about the bar.** `showing` says where each
//! slab lands and what a thumb on it hits, `holding` says which icon and which
//! tone, `reading` and `notices` say what the machine answered. This is the
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

use std::collections::BTreeSet;
use std::io::Write;
use std::os::fd::{AsRawFd, RawFd};
use std::process::{ExitCode, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use console_compositor::{Asked, Carrying, Told, Workspace};
use console_core_colour::spent::{Undressed, beside, read};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_i32};
use console_core_our_programs::Ours;
use console_draw_painting::{Frame, Run, measured, onto};
use console_draw_surface::standing::{
    Anchor, Gone, Keyboard, Margin, Room, Under, Wanted,
};
use console_draw_surface::{Missing, Poke, Surface};
use console_music::player::Sound;
use console_onscreen::Up;
use console_program_contract::Topic;
use console_status_bar::clock;
use console_status_bar::dwindling::Watching;
use console_status_bar::holding::{self, Held, Open, Paused, Playing};
use console_status_bar::notices::{Waiting, notices};
use console_status_bar::reading::{Says, What};
use console_status_bar::showing::{
    self, Bar, Does, Drawn, Face, Filling, Fitting, Measured, Slot, Wearing,
};
use console_notifications::updating;
use console_status_bar::watch;
use console_waiting::woken;

const GATHERING: Duration = Duration::from_millis(120);

const FOLLOWING: Duration = Duration::from_millis(200);

const AT_MOST: Duration = Duration::from_secs(60);

#[derive(Debug)]
enum Cannot {
    Lost(std::io::Error),
    NoPalette { at: std::path::PathBuf, why: std::io::Error },
    Undressed(Undressed),
    Pipe(std::io::Error),
    Deaf(std::io::Error),
    Screenless,
    Compositor(Missing),
}

impl std::fmt::Display for Cannot {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cannot::Lost(why) => write!(to, "this program cannot find itself: {why}"),
            Cannot::NoPalette { at, why } => write!(to, "no palette at {}: {why}", at.display()),
            Cannot::Undressed(why) => write!(to, "{why}"),
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

impl From<Undressed> for Cannot {
    fn from(why: Undressed) -> Cannot {
        Cannot::Undressed(why)
    }
}

impl From<Missing> for Cannot {
    fn from(why: Missing) -> Cannot {
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
    Notices,
    Player,
}

fn wakes(what: What) -> Result<Woke, Never> {
    Ok(match what {
        What::Battery => Woke::Battery,
        What::Bluetooth => Woke::Bluetooth,
        What::Network => Woke::Network,
        What::Sound => Woke::Sound,
    })
}

struct Reading {
    what: What,
    says: Says,
    due: Instant,
}

fn drawing() -> Result<(), Cannot> {
    let wearing = dressed()?;
    let mut surface = Surface::connect()?;
    let waking = woken::pipe().map_err(Cannot::Pipe)?;
    let applying = listening().map_err(Cannot::Deaf)?;
    let seen = Arc::new(Mutex::new(BTreeSet::new()));
    let saying = Arc::new(waking.saying);
    let Ok(()) = sources(&seen, &saying);

    let whole = screen()?;
    let Ok(fitting) = Fitting::of(whole);
    let Ok(tall) = fitting.tall();
    let Ok(margin) = Margin::none();

    surface.show(&Wanted {
        namespace: showing::WHO.to_string(),
        anchor: Anchor::Top,
        size: Size { wide: 0, tall },
        margin,
        keyboard: Keyboard::Declines,
        room: Room::Reserves,
        under: Under::Anything,
    })?;

    let mut dwindling = Watching::default();
    let mut readings = Vec::new();

    for what in holding::ALONG {
        let Ok(says) = taken(what, &mut dwindling);
        let Ok(due) = due(what);

        readings.push(Reading { what, says, due });
    }

    let Ok(bell) = rung();
    let Ok(music) = playing();
    let Ok(open) = shown(Open {
        launcher: Up::NotThere,
        keyboard: Up::NotThere,
        music: Up::NotThere,
        notices: Up::NotThere,
        calendar: Up::NotThere,
        settings: Up::NotThere,
        tab: None,
    });
    let Ok(when) = clock::standing();
    let Ok(said) = clock::now();
    let Ok((workspaces, front)) = walked();

    let mut held = Held {
        readings: readings.iter().map(|one| (one.what, one.says.clone())).collect(),
        bell,
        music,
        clock: said,
        workspaces,
        front,
        open,
    };
    let mut was = when;
    let mut settling = Settling::No;
    let mut last: Option<Drawn> = None;
    let woken = waking.waiting.as_raw_fd();

    loop {
        let Ok(far) = updating::far();
        let filling = match &far {
            Some(far) => Filling::At(far.thousandths),
            None => Filling::Nothing,
        };
        let Ok(saying) = held.saying(filling);
        let Ok(bar) = sized(&saying, fitting);
        let logical = match surface.logical() {
            Ok(Some(logical)) => logical,
            Ok(None) | Err(_) => whole,
        };
        let Ok(drawn) =
            showing::along(&bar, &wearing, Size { wide: logical.wide, tall: whole.tall });

        match last.as_ref() == Some(&drawn) {
            true => {}
            false => {
                let Ok(()) = painted(&mut surface, &drawn);

                last = Some(drawn.clone());
            }
        }

        let Ok(closed) = surface.closed();

        match closed {
            Gone::Yes => return Ok(()),
            Gone::No => {}
        }

        let Ok(until) = waiting(&readings, filling, settling);

        surface.wait(&[woken, applying], Some(until))?;

        let Ok(()) = tapped(&mut surface, &drawn);
        let Ok(()) = woken::drained(&waking.waiting);
        let Ok(()) = told_again(applying);

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

        match woke.contains(&Woke::Notices) {
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

        let Ok(now) = clock::standing();

        match now == was {
            true => {}
            false => {
                let Ok(said) = clock::now();

                held.clock = said;
                was = now;
            }
        }

        held.readings = readings.iter().map(|one| (one.what, one.says.clone())).collect();
    }
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

fn due(what: What) -> Result<Instant, Never> {
    let Ok(every) = watch::tick(what);
    let now = Instant::now();

    Ok(match now.checked_add(every) {
        Some(due) => due,
        None => now,
    })
}

fn listening() -> Result<RawFd, std::io::Error> {
    let told = libc::SIGRTMIN().saturating_add(console_onscreen::WAKES_AT);

    // SAFETY: a mask on the stack, filled and applied by the calls that own it,
    // and a descriptor the kernel opens for exactly the signals in it.
    unsafe {
        let mut mask: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut mask);
        libc::sigaddset(&mut mask, told);

        match libc::pthread_sigmask(libc::SIG_BLOCK, &mask, std::ptr::null_mut()) != 0 {
            true => return Err(std::io::Error::last_os_error()),
            false => {},
        }

        let heard = libc::signalfd(-1, &mask, libc::SFD_CLOEXEC | libc::SFD_NONBLOCK);

        match heard < 0 {
            true => Err(std::io::Error::last_os_error()),
            false => Ok(heard),
        }
    }
}

fn told_again(heard: RawFd) -> Result<(), Never> {
    let size = std::mem::size_of::<libc::signalfd_siginfo>();
    let Ok(whole) = fitted::<usize, isize>(size);

    loop {
        // SAFETY: a struct this frame owns, filled by the kernel or not at all,
        // on a descriptor that never blocks.
        let read = unsafe {
            let mut said: libc::signalfd_siginfo = std::mem::zeroed();

            libc::read(heard, std::ptr::from_mut(&mut said).cast(), size)
        };

        match read == whole {
            true => {},
            false => return Ok(()),
        }
    }
}

fn waiting(
    readings: &[Reading],
    filling: Filling,
    settling: Settling,
) -> Result<Duration, Never> {
    match settling {
        Settling::Yes => return Ok(GATHERING),
        Settling::No => {}
    }

    match filling {
        Filling::At(_) => return Ok(FOLLOWING),
        Filling::Nothing => {}
    }

    let now = Instant::now();
    let soonest = readings.iter().map(|one| one.due).min();
    let Ok(minute) = clock::until_the_minute_turns();

    let until = match soonest {
        Some(soonest) => soonest.saturating_duration_since(now).min(minute),
        None => minute,
    };

    Ok(until.clamp(Duration::from_millis(1), AT_MOST))
}

fn again(
    woke: &BTreeSet<Woke>,
    readings: &mut [Reading],
    dwindling: &mut Watching,
) -> Result<Looked, Never> {
    let mut any = Looked::Not;

    for one in readings.iter_mut() {
        let Ok(mine) = wakes(one.what);

        match woke.contains(&mine) {
            true => {
                let Ok(says) = taken(one.what, dwindling);
                let Ok(due) = due(one.what);

                one.says = says;
                one.due = due;
                any = Looked::Again;
            }
            false => {}
        }
    }

    Ok(any)
}

fn ticked(readings: &mut [Reading], dwindling: &mut Watching) -> Result<(), Never> {
    let now = Instant::now();

    for one in readings.iter_mut() {
        match one.due <= now {
            true => {
                let Ok(says) = taken(one.what, dwindling);
                let Ok(due) = due(one.what);

                one.says = says;
                one.due = due;
            }
            false => {}
        }
    }

    Ok(())
}

fn taken(what: What, dwindling: &mut Watching) -> Result<Says, Never> {
    match what {
        What::Battery => {}
        What::Bluetooth | What::Network | What::Sound => return what.says(),
    }

    let Ok(said) = console_default_applications::battery::charge();
    let Ok(()) = dwindling.seen(&said);

    console_status_bar::reading::battery(&said)
}

fn rung() -> Result<Says, Never> {
    let Ok(whole) = console_notifications::serving::held();
    let Ok(waiting) = Waiting::of(&whole.waiting, whole.quiet);

    notices(waiting)
}

fn playing() -> Result<Says, Never> {
    let Ok(asked) = console_music::player::playing();

    let asked = match asked {
        Some(asked) => asked,
        None => return holding::music(Paused::No, Playing::Nothing),
    };

    let playing = match asked.sound {
        Sound::Stopped => Playing::Nothing,
        Sound::Playing | Sound::Paused => Playing::Something,
    };
    let paused = match asked.sound {
        Sound::Paused => Paused::Yes,
        Sound::Playing | Sound::Stopped => Paused::No,
    };

    holding::music(paused, playing)
}

fn shown(before: Open) -> Result<Open, Never> {
    let screens = match console_compositor::asked(Asked::Layers) {
        Ok(screens) => screens,
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

    let Ok(launcher) = Ours::Launcher.name();
    let Ok(music) = Ours::MusicPanel.name();
    let Ok(notices) = Ours::NotificationsPanel.name();
    let Ok(calendar) = Ours::CalendarPanel.name();
    let Ok(settings) = Ours::SettingsPanel.name();

    Ok(Open {
        launcher: up(launcher),
        keyboard: up(console_onscreen::KEYBOARD),
        music: up(music),
        notices: up(notices),
        calendar: up(calendar),
        settings: up(settings),
        tab,
    })
}

fn walked() -> Result<(Vec<Workspace>, Option<i64>), Never> {
    let there = match console_compositor::asked(Asked::Workspaces) {
        Ok(said) => {
            let Ok(there) = console_compositor::workspaces(&said);

            there
        }
        Err(why) => {
            eprintln!("console-bar: {why}");

            Vec::new()
        }
    };
    let front = match console_compositor::asked(Asked::ActiveWorkspace) {
        Ok(said) => {
            let Ok(front) = console_compositor::in_front(&said);

            front.map(|workspace| workspace.id)
        }
        Err(_the_compositor_would_not_say_which_one_is_in_front) => None,
    };

    Ok((there, front))
}

fn sized(saying: &showing::Saying, fitting: Fitting) -> Result<Bar, Never> {
    let Ok(left) = every(&saying.left, fitting);
    let Ok(middle) = every(&saying.middle, fitting);
    let Ok(right) = every(&saying.right, fitting);

    Ok(Bar { left, middle, right, filling: saying.filling })
}

fn every(slots: &[Slot], fitting: Fitting) -> Result<Vec<Measured>, Never> {
    let mut measured = Vec::new();

    for slot in slots {
        let mut runs = Vec::new();

        for said in &slot.said {
            let Ok(one) = wide(said.said.as_str(), said.face, fitting);

            runs.push(one);
        }

        measured.push(Measured { slot: slot.clone(), runs });
    }

    Ok(measured)
}

fn wide(said: &str, face: Face, fitting: Fitting) -> Result<Size<u32>, Never> {
    let Ok(font) = fitting.font(face);
    let Ok(weight) = face.weight();

    measured(Run { said, weight, wide: u32::MAX }, &font)
}

fn painted(surface: &mut Surface, drawn: &Drawn) -> Result<(), Never> {
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

fn tapped(surface: &mut Surface, drawn: &Drawn) -> Result<(), Never> {
    let Ok(pokes) = surface.pokes();

    for poke in pokes {
        let at = match poke {
            Poke::Up | Poke::Moved { .. } => continue,
            Poke::Down { at } => at,
        };
        let Ok(across) = toward_zero_i32(at.0);
        let Ok(down) = toward_zero_i32(at.1);
        let Ok(on) = drawn.on(Point { across, down });

        match on {
            Some(does) => {
                let Ok(()) = doing(does);
            }
            None => {}
        }
    }

    Ok(())
}

fn doing(does: Does) -> Result<(), Never> {
    let mut starting = match does {
        Does::Launcher => {
            let Ok(command) = Ours::Launcher.command();

            command
        }
        Does::Keyboard => {
            let Ok(command) = Ours::KeyboardToggle.command();

            command
        }
        Does::Music => {
            let Ok(command) = Ours::MusicPanel.command();

            command
        }
        Does::Notices => {
            let Ok(command) = Ours::NotificationsPanel.command();

            command
        }
        Does::Calendar => {
            let Ok(command) = Ours::CalendarPanel.command();

            command
        }
        Does::Settings(what) => {
            let Ok(mut command) = Ours::SettingsPanel.command();
            let Ok(tab) = what.tab();

            let _ = command.arg(tab);

            command
        }
        Does::Workspace(id) => return switched(id),
    };

    let Ok(()) = console_response_times::pressed_here(&mut starting);

    let _ = starting.stdout(Stdio::null()).stderr(Stdio::null());

    match console_program_lifetime::let_go(&mut starting) {
        Ok(_it_outlives_this_bar) => {}
        Err(why) => eprintln!("console-bar: {why}"),
    }

    Ok(())
}

fn switched(id: i64) -> Result<(), Never> {
    let Ok(lua) = console_compositor::onto(&id.to_string(), Carrying::Nothing);
    let Ok(_done) = console_compositor::told(Told::Dispatch, &lua);

    Ok(())
}

fn screen() -> Result<Size<u32>, Cannot> {
    let said = match console_compositor::asked(Asked::Monitors) {
        Ok(said) => said,
        Err(why) => {
            eprintln!("console-bar: {why}");

            return Err(Cannot::Screenless);
        }
    };
    let Ok(monitors) = console_compositor::monitors(&said);
    let logical = monitors.iter().find_map(|monitor| match monitor.logical() {
        Ok(Some(size)) => Some(size),
        Ok(None) | Err(_) => None,
    });

    match logical {
        Some(logical) => Ok(logical),
        None => Err(Cannot::Screenless),
    }
}

fn dressed() -> Result<Wearing, Cannot> {
    let me = std::env::current_exe().map_err(Cannot::Lost)?;
    let Ok(at) = beside(&me);
    let held =
        std::fs::read_to_string(&at).map_err(|why| Cannot::NoPalette { at: at.clone(), why })?;
    let Ok(spent) = read(&held);
    let wearing = Wearing::out_of(&spent)?;

    Ok(wearing)
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
    for what in holding::ALONG {
        let (say, heard) = channel();
        let Ok(()) = watch::telling(what, say);
        let Ok(mine) = wakes(what);
        let Ok(()) = forwarded(mine, heard, seen, saying);
    }

    let (say, heard) = channel();
    let Ok(()) = watch::telling_surfaces(say);
    let Ok(()) = forwarded(Woke::Surfaces, heard, seen, saying);

    let (say, heard) = channel();
    let Ok(()) = watch::telling_notices(say);
    let Ok(()) = forwarded(Woke::Notices, heard, seen, saying);

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
                Err(_nobody_is_reading_these) => return,
            }

            let mut down = saying.as_ref();

            match down.write_all(&[1]) {
                Ok(()) => {}
                Err(_the_loop_has_gone) => return,
            }
        }
    }))
}

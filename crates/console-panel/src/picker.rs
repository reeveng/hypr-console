//! One picker at a time, and the door that opened it closes it.
//!
//! A picker changes what the buttons do while it is up: they stop being the
//! desktop's and become move the highlight, confirm, and back out. The daemon
//! decides that by asking the compositor whether a picker is on the screen,
//! which is one question with one answer -- so with two of them up, the answer
//! is right and the picker it is about is the wrong one. What you are looking
//! at is being driven for something you cannot see.
//!
//! It used to be worse and it is worth knowing what it was, because the shape
//! of the fix is left over from it: each picker loaded a profile of its own on
//! the way in and put the desktop's back on the way out, so the second to open
//! took the profile the first was relying on and the first to close handed the
//! buttons back over the top of the other. Nothing loads a profile to open a
//! menu any more.
//!
//! It is invisible while it happens. Two of the same picker are drawn in the
//! same place, so backing out of one leaves you looking at what appears to be
//! the same picker that just ignored you. Pressing back harder is the natural
//! thing to try and it does nothing, because every press is closing a real
//! picker and there is another behind it.
//!
//! Nothing stopped it before: the menu is on a button, on a paddle and on a
//! key, the settings are on a button and on the bar, and every one of those
//! roads started a new process that knew nothing about the others.
//!
//! A second picker is not turned away, though, because the bar can be tapped
//! while one is up and a tap that does nothing at all is a bar that looks
//! broken. The one on screen goes and the new one takes its place. Asked for
//! through the same door it came out of, it goes and nothing replaces it: the
//! icon that brought a panel out is the icon that puts it away, which is the
//! only way a finger has of closing anything the settings icons open.
//!
//! The lock is a file in the session's own runtime directory, held open for as
//! long as the process lives. The kernel drops it when the process ends however
//! it ends, so a picker that is killed outright leaves nothing behind to
//! clear, and waiting for the lock rather than for the process is what puts the
//! two in order: the one going holds it until the kernel closes its files, so
//! the lock being free is the screen being free.
//!
//! A panel the host draws takes its turn the other way round. The one going
//! still holds the lock, so the one coming asks the host to be drawn first and
//! only then asks the one going to stop; the host closes it after the new one's
//! first frame, and the lock follows a moment later. Waiting for the lock first
//! was a screen with nothing on it for as long as the host took to tear one
//! panel down and build the next. `alone_once_drawn` is that order, and a
//! panel with no host to ask falls back to the old one in `handoff::stood_in`,
//! where drawing itself over a panel that is still up would be two of them.
//!
//! And one that will not go is taken off the screen. A panel whose compositor
//! has gone spins on a socket that is hung up and answers nothing, which is the
//! whole of `asked`'s argument, and it holds the lock while it spins: every
//! panel asked for after it is turned away, and the button that opens one looks
//! broken to the hand that keeps pressing it. Being asked is the politeness
//! there is, and it is bounded. After that the screen is taken, because the
//! kernel drops the lock as the process ends and stopping is the one thing a
//! wedged picker can still be made to do.
//!
//! An app is not a picker and does not take this lock. It is somewhere a person
//! stays -- the files, the pictures -- drawn by its own process across the whole
//! screen, and a picker opened over it goes and leaves it where it was. What it
//! keeps instead is one lock of its own, so there is never a second copy of it
//! underneath the first: asked for again with the same arguments it is already
//! up and nothing starts, and asked for anything else the one up stands down
//! for it. `alone_as` is that, and it is `choosing` over a different file.


use console_core_never::Never;
use console_waiting::{Schedule, Ready, Outcome, found_handed, until};
use console_core_number_conversion::fitted;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

const NO_ONE_NAMED: &str = "";


pub const BREATH: Duration = Duration::from_millis(20);

pub const PATIENCE: Duration = Duration::from_secs(10);

pub const COMING: Duration = Duration::from_secs(2);

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the process is what holds the screen: the lock is open for exactly as long as this program lives and the kernel drops it however the program ends, so a handle someone could drop early is a lock released while the picker is still up"
    )
)]
static HELD: Mutex<Option<Holding>> = Mutex::new(None);

struct Holding {
    handle: File,
    name: String,
    lock: Lock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lock {
    Acquired,
    AfterDrawing(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Handover {
    Now,
    OnceDrawn,
}

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "a signal handler is handed nothing and can find nothing: the pid it passes on the signal to has to be somewhere the handler can read without a lock and without allocating"
    )
)]
static SHOWING: AtomicI32 = AtomicI32::new(0);

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the screen is taken before there is a panel or a stopwatch to hold the measurement, and it is read when the panel goes up; `opening`'s own head makes this argument for the four marks after it"
    )
)]
static WAITED: AtomicU64 = AtomicU64::new(0);

pub fn waited_for_screen() -> Result<Duration, Never> {
    Ok(Duration::from_nanos(WAITED.load(Ordering::SeqCst)))
}

struct Stopwatch(Instant);

impl Drop for Stopwatch {
    fn drop(&mut self) {
        let Ok(whole) = fitted(self.0.elapsed().as_nanos());
        WAITED.store(whole, Ordering::SeqCst);
    }
}

pub fn showing(pid: i32) -> Result<(), Never> {
    SHOWING.store(pid, Ordering::SeqCst);
    // SAFETY: the handler stores nothing and calls nothing that allocates.
    let answering = unsafe { console_signals::answered(&console_signals::STOPPING, asked) };

    match answering {
        Ok(()) => {},
        Err(fault) => eprintln!("console-panel: {fault}"),
    }

    Ok(())
}

pub fn showing_nothing() -> Result<(), Never> {
    SHOWING.store(0, Ordering::SeqCst);

    Ok(())
}

extern "C" fn asked(_number: core::ffi::c_int) {
    let pid = SHOWING.load(Ordering::SeqCst);

    match pid > 0 {
        true => {
            let Ok(()) = console_program_lifetime::signal(pid, rustix::process::Signal::TERM);
        }
        false => {},
    }
}

pub fn where_() -> Result<PathBuf, Never> {
    locked("picker")
}

pub fn where_for(app: &str) -> Result<PathBuf, Never> {
    locked(&format!("app-{app}"))
}

fn locked(stem: &str) -> Result<PathBuf, Never> {
    let held = console_core_places::runtime()?;
    let runtime = held.as_deref().and_then(Path::to_str);

    #[cfg_attr(
        dylint_lib = "explicit026_env_read_once",
        allow(
            explicit026_env_read_once,
            reason = "WAYLAND_DISPLAY is which screen this process is drawing on, and the lock this names is one per screen. Nothing else here asks which screen it is"
        )
    )]
    let screen = match std::env::var("WAYLAND_DISPLAY") {
        Ok(screen) => Some(screen),
        Err(_unset) => None,
    };

    under(runtime, screen.as_deref(), stem)
}

fn under(runtime: Option<&str>, screen: Option<&str>, stem: &str) -> Result<PathBuf, Never> {
    let runtime = match runtime {
        Some(runtime) => match runtime.is_empty() {
            true => "/tmp",
            false => runtime,
        },
        None => "/tmp",
    };

    let screen = match screen {
        Some(screen) => match screen.is_empty() {
            true => "no-screen-named",
            false => screen,
        },
        None => "no-screen-named",
    };

    Ok(Path::new(runtime).join("console").join(format!("{stem}-{screen}.lock")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Took {
    It,
    Not,
}

pub fn take(handle: &File) -> Result<Took, Never> {
    Ok(match rustix::fs::flock(handle, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Took::It,
        Err(_someone_else_holds_it) => Took::Not,
    })
}

pub fn holder(said: &str) -> Result<(i32, &str), Never> {
    let (pid, name) = match said.trim().split_once(' ') {
        Some(both) => both,
        None => (said.trim(), NO_ONE_NAMED),
    };

    let pid = match pid.parse::<i32>() {
        Ok(pid) => pid,
        Err(_not_a_pid) => return Ok((0, name)),
    };

    Ok((pid, name))
}

fn door(name: &str) -> Result<&str, Never> {
    Ok(name.trim())
}

enum Meanwhile {
    Rendered,
    Free,
    Stuck,
}

fn meanwhile(handle: &mut File) -> Result<Meanwhile, Never> {
    let Ok(patience) = Schedule::asking_every(COMING, BREATH);

    let Ok(answer) = found_handed(patience, handle, |handle| {
        let Ok(took) = take(handle);

        match took == Took::It {
            true => return Ok(Some(Meanwhile::Free)),
            false => {},
        }

        let Ok(said) = read(handle);
        let Ok((_pid, drawn)) = holder(&said);

        Ok(match !drawn.is_empty() {
            true => Some(Meanwhile::Rendered),
            false => None,
        })
    });

    Ok(match answer {
        Some(answer) => answer,
        None => Meanwhile::Stuck,
    })
}

fn read(handle: &mut File) -> Result<String, Never> {
    let mut said = String::new();
    let _ = handle.seek(SeekFrom::Start(0));
    let _ = handle.read_to_string(&mut said);

    Ok(said)
}

fn written(handle: &mut File, name: &str) -> Result<(), Never> {
    let _ = handle.seek(SeekFrom::Start(0));
    let _ = handle.set_len(0);
    let _ = write!(handle, "{} {name}", std::process::id());
    let _ = handle.flush();

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Again {
    Closes,
    Keeps,
}

pub fn console_put_away() -> Result<Away, Never> {
    let Ok(where_) = where_();

    told_to_go(&where_)
}

pub fn app_put_away() -> Result<Away, Never> {
    let Ok(picker) = where_();
    let Ok(on_top) = app_on_top(&picker);

    match on_top {
        Some(at) => told_to_go(&at),
        None => Ok(Away::None),
    }
}

fn app_on_top(picker: &Path) -> Result<Option<PathBuf>, Never> {
    let folder = match picker.parent() {
        Some(folder) => folder,
        None => return Ok(None),
    };

    let screen = match picker.file_name().and_then(|name| name.to_str()).and_then(|name| name.strip_prefix("picker")) {
        Some(screen) => screen.to_string(),
        None => return Ok(None),
    };

    let mut held: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();

    for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();

        match name.starts_with("app-") && name.ends_with(&screen) {
            true => {},
            false => continue,
        }

        let at = entry.path();
        let Ok(holding) = someone_holds(&at);

        match (holding, std::fs::metadata(&at).and_then(|found| found.modified())) {
            (Owned::Yes, Ok(when)) => held.push((when, at)),
            (Owned::No, _) => {},
            (Owned::Yes, Err(_unstamped)) => {},
        }
    }

    held.sort();

    Ok(held.pop().map(|(_when, at)| at))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owned {
    Yes,
    No,
}

fn someone_holds(at: &Path) -> Result<Owned, Never> {
    #[cfg_attr(
        dylint_lib = "explicit040_no_torn_write",
        allow(
            explicit040_no_torn_write,
            reason = "the lock, opened only to ask whether it is held; nothing is written through it"
        )
    )]
    let handle = match OpenOptions::new().read(true).open(at) {
        Ok(handle) => handle,
        Err(_unreadable) => return Ok(Owned::No),
    };

    let Ok(took) = take(&handle);

    Ok(match took {
        Took::It => Owned::No,
        Took::Not => Owned::Yes,
    })
}

fn told_to_go(where_: &Path) -> Result<Away, Never> {
    #[cfg_attr(
        dylint_lib = "explicit040_no_torn_write",
        allow(
            explicit040_no_torn_write,
            reason = "the lock, whose whole point is the open file and not its bytes: a rename would give the next process a different file to take the lock on, which is the one thing this must never do"
        )
    )]
    let mut handle = match OpenOptions::new().read(true).write(true).open(where_) {
        Ok(handle) => handle,
        Err(_unreadable) => return Ok(Away::None),
    };

    let Ok(took) = take(&handle);

    match took == Took::It {
        true => return Ok(Away::None),
        false => {},
    }

    let Ok(said) = read(&mut handle);
    let Ok((pid, _door)) = holder(&said);

    match pid <= 0 {
        true => return Ok(Away::None),
        false => {},
    }

    let Ok(()) = console_program_lifetime::signal(pid, rustix::process::Signal::TERM);

    Ok(Away::Notified)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Away {
    Notified,
    None,
}

fn holding() -> Result<std::sync::MutexGuard<'static, Option<Holding>>, Never> {
    Ok(match HELD.lock() {
        Ok(held) => held,

        Err(poisoned) => poisoned.into_inner(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alone {
    Yes,
    No,
}

pub fn alone(name: &str, again: Again) -> Result<Alone, Never> {
    let Ok(path) = where_();

    choosing(name, again, Handover::Now, &path)
}

pub fn alone_once_drawn(name: &str, again: Again) -> Result<Alone, Never> {
    let Ok(path) = where_();

    choosing(name, again, Handover::OnceDrawn, &path)
}

pub fn alone_as(app: &str, asked: &[String]) -> Result<Alone, Never> {
    let Ok(path) = where_for(app);
    let door = std::iter::once(app).chain(asked.iter().map(String::as_str)).collect::<Vec<_>>().join(" ");
    let Ok(alone) = choosing(&door, Again::Keeps, Handover::Now, &path);

    match alone {
        Alone::Yes => {
            let Ok(()) = drawn();
        },
        Alone::No => {},
    }

    Ok(alone)
}

fn choosing(name: &str, again: Again, handover: Handover, path: &Path) -> Result<Alone, Never> {
    #[cfg_attr(
        dylint_lib = "explicit039_no_reading_the_clock",
        allow(
            explicit039_no_reading_the_clock,
            reason = "the stopwatch over taking the screen, which is this function measuring its own waiting rather than deciding anything from the clock; every way out of here is a return and the drop is what makes the measurement whole"
        )
    )]
    let _asking = Stopwatch(Instant::now());
    let Ok(name) = door(name);
    let Ok(mut held) = holding();

    match held.is_some() {
        true => return Ok(Alone::Yes),
        false => {},
    }

    #[cfg_attr(
        dylint_lib = "explicit040_no_torn_write",
        allow(
            explicit040_no_torn_write,
            reason = "the lock, whose whole point is the open file and not its bytes -- `truncate(false)` is what keeps what is already in it -- and a rename would hand the next process a different file to take the lock on"
        )
    )]
    let opened = path
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path));

    let mut handle = match opened {
        Ok(handle) => handle,
        Err(_unreadable) => return Ok(Alone::Yes),
    };

    let Ok(took) = take(&handle);

    match took == Took::Not {
        true => {
            let Ok(said) = read(&mut handle);
            let Ok((pid, holding)) = holder(&said);

            let Ok(ours) = fitted::<u32, i32>(std::process::id());

            match pid == 0 || pid == ours {
                true => return Ok(Alone::No),
                false => {},
            }

            match holding == name && again == Again::Keeps {
                true => {
                    eprintln!("{name}: {pid} is showing it, and this door only opens");

                    return Ok(Alone::No);
                }
                false => {},
            }

            match holding.is_empty() {
                true => {
                    let Ok(meanwhile) = meanwhile(&mut handle);

                    match meanwhile {
                        Meanwhile::Rendered => return Ok(Alone::No),
                        Meanwhile::Free => return kept(&mut held, handle, name),
                        Meanwhile::Stuck => {}
                    }
                }
                false => {},
            }

            match (handover, holding == name) {
                (Handover::OnceDrawn, false) => {
                    *held = Some(Holding { handle, name: name.to_string(), lock: Lock::AfterDrawing(pid) });

                    return Ok(Alone::Yes);
                }
                (Handover::OnceDrawn, true) | (Handover::Now, _) => {},
            }

            let Ok(freed) = freed_from(&handle, pid, name);

            match freed {
                Outcome::Happened => {},
                Outcome::RanOut => return Ok(Alone::No),
            }

            match holding == name {
                true => {
                    let Ok(()) = crate::left_open::put_away();

                    return Ok(Alone::No);
                }
                false => {},
            }
        }
        false => {},
    }

    kept(&mut held, handle, name)
}

fn freed_from(handle: &File, pid: i32, name: &str) -> Result<Outcome, Never> {
    let Ok(()) = console_program_lifetime::signal(pid, rustix::process::Signal::TERM);

    let Ok(asked) = given_up(handle, PATIENCE);

    match asked {
        Outcome::Happened => return Ok(Outcome::Happened),
        Outcome::RanOut => {},
    }

    eprintln!("{name}: {pid} would not give the screen up, and is taken off it");

    let Ok(()) = console_program_lifetime::signal(pid, rustix::process::Signal::KILL);

    let Ok(taken) = given_up(handle, COMING);

    match taken {
        Outcome::Happened => {},
        Outcome::RanOut => eprintln!("{name}: {pid} was taken off the screen and it is not free"),
    }

    Ok(taken)
}

pub fn taken_over() -> Result<(), Never> {
    let Ok(mut held) = holding();

    let holding = match held.as_mut() {
        Some(holding) => holding,
        None => return Ok(()),
    };

    let pid = match holding.lock {
        Lock::Acquired => return Ok(()),
        Lock::AfterDrawing(pid) => pid,
    };

    let Ok(freed) = freed_from(&holding.handle, pid, &holding.name);

    match freed {
        Outcome::Happened => holding.lock = Lock::Acquired,
        Outcome::RanOut => {},
    }

    Ok(())
}

fn given_up(handle: &File, patience: Duration) -> Result<Outcome, Never> {
    let Ok(patience) = Schedule::asking_every(patience, BREATH);

    until(patience, || {
        let Ok(got) = take(handle);

        Ok(match got == Took::It {
            true => Ready::Yes,
            false => Ready::NotYet,
        })
    })
}

fn kept(held: &mut Option<Holding>, mut handle: File, name: &str) -> Result<Alone, Never> {
    let Ok(()) = written(&mut handle, "");

    *held = Some(Holding { handle, name: name.to_string(), lock: Lock::Acquired });

    Ok(Alone::Yes)
}

pub fn drawn() -> Result<(), Never> {
    let Ok(()) = taken_over();
    let Ok(mut held) = holding();

    let holding = match held.as_mut() {
        Some(holding) => holding,
        None => return Ok(()),
    };

    let name = holding.name.clone();

    match holding.lock {
        Lock::Acquired => {
            let Ok(()) = written(&mut holding.handle, &name);
        }
        Lock::AfterDrawing(_still_the_one_before) => {},
    }

    Ok(())
}

pub fn gone() -> Result<(), Never> {
    let Ok(mut held) = holding();

    let holding = match held.as_mut() {
        Some(holding) => holding,
        None => return Ok(()),
    };

    match holding.lock {
        Lock::Acquired => {
            let Ok(()) = written(&mut holding.handle, "");
        }
        Lock::AfterDrawing(_still_the_one_before) => {},
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_lives_under_the_sessions_own_runtime() {
        assert_eq!(
            under(Some("/run/user/1000"), Some("wayland-1"), "picker"),
            Ok(PathBuf::from("/run/user/1000/console/picker-wayland-1.lock"))
        );
        assert_eq!(
            under(None, Some("wayland-1"), "picker"),
            Ok(PathBuf::from("/tmp/console/picker-wayland-1.lock"))
        );
        assert_eq!(under(None, None, "picker"), Ok(PathBuf::from("/tmp/console/picker-no-screen-named.lock")));
    }

    #[test]
    fn a_variable_set_to_nothing_has_not_named_a_screen() {
        assert_eq!(
            under(Some(""), Some(""), "picker"),
            Ok(PathBuf::from("/tmp/console/picker-no-screen-named.lock"))
        );
    }

    #[test]
    fn two_screens_under_one_runtime_are_two_locks() {
        let Ok(login) = under(Some("/run/user/1000"), Some("wayland-1"), "picker");
        let Ok(nested) = under(Some("/run/user/1000"), Some("wayland-7"), "picker");

        assert_ne!(login, nested);
    }

    #[test]
    fn the_file_says_who_is_holding_it_and_which_door_they_came_out_of() {
        assert_eq!(holder("1234 settings sound"), Ok((1234, "settings sound")));
        assert_eq!(holder("1234 "), Ok((1234, "")));
    }

    #[test]
    fn a_picker_on_its_way_has_a_pid_and_no_door() {
        assert_eq!(holder("1234"), Ok((1234, "")));
    }

    #[test]
    fn the_paddle_puts_away_the_app_that_came_up_last_and_nothing_left_behind() {
        let folder = std::env::temp_dir().join(format!("console-apps-on-top-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("a runtime folder");
        let picker = folder.join("picker-wayland-1.lock");
        let lock = |name: &str, seconds: u64| {
            let at = folder.join(name);
            let file = File::create(&at).expect("a lock");
            file.set_modified(std::time::UNIX_EPOCH + Duration::from_secs(seconds)).expect("a time");

            (at, file)
        };

        let (files, files_held) = lock("app-files-wayland-1.lock", 100);
        let (viewer, viewer_held) = lock("app-viewer-wayland-1.lock", 200);
        let (_music, _music_let_go) = lock("app-music-wayland-1.lock", 300);
        let (_other, other_held) = lock("app-files-wayland-7.lock", 400);

        for held in [&files_held, &viewer_held, &other_held] {
            assert_eq!(take(held), Ok(Took::It));
        }

        assert_eq!(app_on_top(&picker), Ok(Some(viewer)), "the music's lock is free, and wayland-7 is another screen");

        viewer_held.unlock().expect("the viewer let go");

        assert_eq!(app_on_top(&picker), Ok(Some(files)), "the viewer is gone and the files are still up under it");

        files_held.unlock().expect("the files let go");

        assert_eq!(app_on_top(&picker), Ok(None), "an app that has gone left a lock nobody holds");

        std::fs::remove_dir_all(&folder).expect("the made-up runtime taken away");
    }

    #[test]
    fn a_door_named_for_a_tab_it_was_not_given_is_the_name_on_its_own() {
        let Ok(name) = door("notifications ");

        assert_eq!(name, "notifications");
        assert_eq!(holder(&format!("1234 {name}")), Ok((1234, name)));
    }

    #[test]
    fn a_file_saying_nothing_names_no_one() {
        assert_eq!(holder(""), Ok((0, "")));
        assert_eq!(holder("what"), Ok((0, "")));
    }
}

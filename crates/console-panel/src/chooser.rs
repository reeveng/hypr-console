//! One chooser at a time, and the door that opened it closes it.
//!
//! A chooser changes what the buttons do while it is up: they stop being the
//! desktop's and become move the highlight, confirm, and back out. The daemon
//! decides that by asking the compositor whether a chooser is on the screen,
//! which is one question with one answer -- so with two of them up, the answer
//! is right and the chooser it is about is the wrong one. What you are looking
//! at is being driven for something you cannot see.
//!
//! It used to be worse and it is worth knowing what it was, because the shape
//! of the fix is left over from it: each chooser loaded a profile of its own on
//! the way in and put the desktop's back on the way out, so the second to open
//! took the profile the first was relying on and the first to close handed the
//! buttons back over the top of the other. Nothing loads a profile to open a
//! menu any more.
//!
//! It is invisible while it happens. Two of the same chooser are drawn in the
//! same place, so backing out of one leaves you looking at what appears to be
//! the same chooser that just ignored you. Pressing back harder is the natural
//! thing to try and it does nothing, because every press is closing a real
//! chooser and there is another behind it.
//!
//! Nothing stopped it before: the menu is on a button, on a paddle and on a
//! key, the settings are on a button and on the bar, and every one of those
//! roads started a new process that knew nothing about the others.
//!
//! A second chooser is not turned away, though, because the bar can be tapped
//! while one is up and a tap that does nothing at all is a bar that looks
//! broken. The one on screen goes and the new one takes its place. Asked for
//! through the same door it came out of, it goes and nothing replaces it: the
//! icon that brought a panel out is the icon that puts it away, which is the
//! only way a finger has of closing anything the settings icons open.
//!
//! The lock is a file in the session's own runtime directory, held open for as
//! long as the process lives. The kernel drops it when the process ends however
//! it ends, so a chooser that is killed outright leaves nothing behind to
//! clear, and waiting for the lock rather than for the process is what puts the
//! two in order: the one going holds it until the kernel closes its files, so
//! the lock being free is the screen being free.


use console_number::fitted;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The wait between tries at the lock, and how many of them.
///
/// Long enough to outlast the pad being handed back, which is the slowest
/// thing the one going does and the reason it is still holding the lock while
/// it does it. Waiting less than that turns a panel asked for right after
/// another closed into a panel that never draws.
pub const BREATH: Duration = Duration::from_millis(20);

/// How long to keep trying, in breaths.
///
/// Ten seconds. Nothing between two choosers is slow -- the one going drops the
/// lock as the kernel closes its files -- so this is not a budget for the
/// handover, it is the point at which a panel that cannot get the lock gives up
/// and says so rather than hanging on a lock nothing is going to release.
pub const PATIENCE: u128 = Duration::from_secs(10).as_millis() / BREATH.as_millis();

/// How long a chooser that has the screen but has not drawn on it is given to
/// appear.
///
/// Longer than one takes to draw, and short enough that a chooser which hangs
/// before it draws cannot shut the screen for the rest of the session: waiting
/// for it forever would be a desktop where no button opens anything and
/// nothing says why.
pub const COMING: usize = 100;

/// What this process is holding, for as long as it lives.
///
/// A lock is released when the last handle to it is closed, and a caller who is
/// not expecting to be holding anything has no reason to keep one. The door is
/// kept beside it so that saying the chooser has appeared says nothing the
/// caller has to remember.
static HELD: Mutex<Option<Holding>> = Mutex::new(None);

struct Holding {
    handle: File,
    name: String,
}

/// What is on the screen, when that is another process.
static SHOWING: AtomicI32 = AtomicI32::new(0);

/// How long the last ask for the screen took, in nanoseconds.
///
/// Kept here rather than handed back, because `alone` answers a question with
/// one word -- may this process draw -- and the panel that draws is started
/// afterwards by a caller that never sees this call's clock. It is the one
/// stretch of an opening that is over before the panel exists, and on the road
/// where one chooser replaces another it is the whole of the difference
/// between a press that feels immediate and one that does not.
static WAITED: AtomicU64 = AtomicU64::new(0);

/// How long the ask for the screen took.
pub fn waited_for_screen() -> Duration {
    Duration::from_nanos(WAITED.load(Ordering::SeqCst))
}

/// The clock over one ask, however that ask ends.
///
/// A guard rather than a line before each `return`, because `alone` leaves by
/// eight roads and the two that wait are not the two anybody would remember to
/// stamp.
struct Asking(Instant);

impl Drop for Asking {
    fn drop(&mut self) {
        WAITED.store(fitted(self.0.elapsed().as_nanos()), Ordering::SeqCst);
    }
}

/// The chooser on the screen is drawn by something this one started.
///
/// The screen is handed over by signalling whoever holds the lock, and a
/// holder that dies without taking its window down leaves the screen occupied
/// by something nothing is waiting on any more: the menu stayed up, nothing
/// put the pad back, and picking a row ran nothing at all. So being asked to
/// go takes the window down and then leaves by the ordinary road, which hands
/// the pad back and lets the lock go as it always would.
pub fn showing(pid: i32) {
    SHOWING.store(pid, Ordering::SeqCst);
    // A function is turned into the number `signal` takes it as, which is
    // the one conversion in this workspace that has no `From` to call and no
    // console-number to call it through: there is no trait that turns a
    // function into an integer, because outside this call there is no reason
    // to want one.
    #[cfg_attr(
        dylint_lib = "explicit011_no_as_cast",
        allow(
            explicit011_no_as_cast,
            reason = "no trait turns a function into the number `signal` takes; the way out is a signalfd, which is its own decision"
        )
    )]
    let answer = asked as extern "C" fn(libc::c_int) as libc::sighandler_t;

    for number in [libc::SIGHUP, libc::SIGINT, libc::SIGTERM] {
        // SAFETY: the handler stores nothing and calls nothing that allocates.
        unsafe { libc::signal(number, answer) };
    }
}

/// Nothing is on the screen any more, so nothing is taken down.
pub fn showing_nothing() {
    SHOWING.store(0, Ordering::SeqCst);
}

extern "C" fn asked(_number: libc::c_int) {
    let pid = SHOWING.load(Ordering::SeqCst);

    match pid > 0 {
        true => {
            // SAFETY: a signal to a pid this process started and has not reaped.
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
        false => {},
    }
}

/// The lock's file, under whatever this session calls its runtime.
pub fn where_() -> PathBuf {
    let runtime = match std::env::var("XDG_RUNTIME_DIR") {
        Ok(runtime) if !runtime.is_empty() => runtime,
        _ => "/tmp".to_string(),
    };

    Path::new(&runtime).join("console").join("chooser.lock")
}

/// Whether the lock came free when it was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Took {
    /// It did, so nobody else is holding the screen.
    It,
    /// Somebody else has it.
    Not,
}

/// Try the lock once, without waiting for it.
pub fn take(handle: &File) -> Took {
    // SAFETY: the descriptor is this file's, and open for as long as the call.
    match unsafe { libc::flock(handle.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 } {
        true => Took::It,
        false => Took::Not,
    }
}

/// Who has it, and which door they came out of.
///
/// The name written beside the pid is the door, not the program. Two of the
/// bar's icons are the same program at different tabs, and tapping one while
/// the other is up should move the panel rather than close it.
pub fn holder(said: &str) -> (i32, &str) {
    let (pid, name) = said.trim().split_once(' ').unwrap_or((said.trim(), ""));

    // Nought is a pid nothing runs under, which every caller reads as nobody
    // holding the door -- the right answer for a line that is not one of ours.
    let Ok(pid) = pid.parse::<i32>() else { return (0, name) };

    (pid, name)
}

/// The name a door is written down under.
///
/// What is read back comes through `trim`, because a chooser on its way has a
/// pid and no door at all, and the line written then is a pid with nothing
/// after the space. So a name that ends in a space cannot come back out of the
/// file as it went in.
///
/// A panel that takes a tab names its door after the tab it was asked for, and
/// asked for without one -- the bell on the bar, the settings on the Menu
/// button -- the name ends in the space where the tab was not. The bell wrote
/// "notices " and read "notices", did not recognise its own door, and put the
/// panel away for being somebody else's and opened it again in the same press.
/// Every other icon along that edge names a tab, which is why it was only the
/// bell that flickered.
fn door(name: &str) -> &str {
    name.trim()
}

/// What became of the one that had the screen and had not drawn on it.
enum Meanwhile {
    Drawn,
    Free,
    Stuck,
}

/// Wait on the one holding the screen, for as long as one takes to appear.
///
/// The lock is asked about first: a chooser on its way out has let go of its
/// door before it lets go of the lock, and the press waiting on it wants the
/// screen the moment it is free rather than at the end of the wait.
fn meanwhile(handle: &mut File) -> Meanwhile {
    for _ in 0..COMING {
        std::thread::sleep(BREATH);

        if take(handle) == Took::It {
            return Meanwhile::Free;
        }

        if !holder(&read(handle)).1.is_empty() {
            return Meanwhile::Drawn;
        }
    }

    Meanwhile::Stuck
}

fn read(handle: &mut File) -> String {
    let mut said = String::new();
    let _ = handle.seek(SeekFrom::Start(0));
    let _ = handle.read_to_string(&mut said);
    said
}

fn written(handle: &mut File, name: &str) {
    let _ = handle.seek(SeekFrom::Start(0));
    let _ = handle.set_len(0);
    let _ = write!(handle, "{} {name}", std::process::id());
    let _ = handle.flush();
}

/// What the same door asked twice means.
///
/// A finger on the bar has no other way to put a panel away, so the icon that
/// brought it out closes it again. A paddle is not that: the left paddle opens
/// and the right paddle closes, in every profile and whatever is on screen, so
/// the menu asked for while the menu is up stays as it is.
#[derive(Clone, Copy, PartialEq)]
pub enum Again {
    Closes,
    Keeps,
}

/// Put away whatever is up, and say whether there was anything.
///
/// The holder is told rather than killed outright: it takes its own window
/// down, hands the desktop's buttons back and goes, which is the same road out
/// as every other.
pub fn put_away() -> Away {
    let Ok(mut handle) = OpenOptions::new().read(true).write(true).open(where_()) else {
        return Away::Nothing;
    };

    // Nobody is holding it, so there is nothing on screen to put away.
    if take(&handle) == Took::It {
        return Away::Nothing;
    }

    let (pid, _) = holder(&read(&mut handle));

    if pid <= 0 {
        return Away::Nothing;
    }

    // SAFETY: a signal to a pid, which is what the file said was there.
    unsafe { libc::kill(pid, libc::SIGTERM) };

    Away::Told
}

/// Whether there was anything on the screen to put away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Away {
    /// Somebody was holding it, and has been told to go.
    Told,
    /// Nobody was, so this press has to mean something else.
    Nothing,
}

/// True if this process may be the chooser, false if it may not.
///
/// False means there is nothing more to do: either the panel that was up has
/// been closed by this call, which is what the same door asked twice means, or
/// something else is holding the screen and will not let go, or one is on its
/// way and this press is somebody who has not seen it yet.
/// The door state, through a lock that a panic elsewhere cannot take away.
///
/// A poisoned mutex means some other thread died holding this, and the value
/// behind it is a door name and a file handle -- there is no half-written state
/// for a panic to have left. Refusing to open the chooser because of a thread
/// that already died would turn one fault into a desktop where the menu button
/// does nothing.
fn holding() -> std::sync::MutexGuard<'static, Option<Holding>> {
    match HELD.lock() {
        Ok(held) => held,

        // A thread that panicked while holding this is the case the paragraph
        // above is about: what it left behind is a door name and a file
        // handle, and refusing the chooser over it helps nobody.
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Whether this process may be the chooser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alone {
    /// It may: the screen is this process's now.
    Yes,
    /// It may not, and there is nothing more to do.
    No,
}

pub fn alone(name: &str, again: Again) -> Alone {
    let _asking = Asking(Instant::now());
    let name = door(name);
    let mut held = holding();

    if held.is_some() {
        return Alone::Yes;
    }

    let path = where_();
    let opened = path
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path));

    // Nowhere to keep a lock is not a reason to refuse.
    let Ok(mut handle) = opened else { return Alone::Yes };

    if take(&handle) == Took::Not {
        let said = read(&mut handle);
        let (pid, holding) = holder(&said);

        // A second chooser inside one process is a program asking twice, and
        // there is no taking the screen from yourself.
        if pid == 0 || pid == fitted::<u32, i32>(std::process::id()) {
            return Alone::No;
        }

        // Asked again through the door it came out of, by something that only
        // opens: it is open.
        if holding == name && again == Again::Keeps {
            eprintln!("{name}: {pid} is showing it, and this door only opens");
            return Alone::No;
        }

        // Somebody has the lock but has not drawn yet. A press now is a
        // thumb that has not seen the chooser rather than one putting it
        // away, and closing what has not appeared is how one press became
        // two: the paddle cancelled the menu it had just asked for, and the
        // next press was the one that seemed to work. So wait for it, and
        // take the screen only if it never comes.
        if holding.is_empty() {
            match meanwhile(&mut handle) {
                Meanwhile::Drawn => return Alone::No,
                Meanwhile::Free => return kept(&mut held, handle, name),
                Meanwhile::Stuck => {}
            }
        }

        // SAFETY: a signal to a pid, which is what the file said was there.
        unsafe { libc::kill(pid, libc::SIGTERM) };

        let waited = (0..PATIENCE).any(|_| {
            let got = take(&handle);

            if got == Took::Not {
                std::thread::sleep(BREATH);
            }

            got == Took::It
        });

        // It will not go, and two of them is worse. Say so: a press that
        // does nothing is otherwise a button reported as broken, with a
        // journal that shows the press arriving and the chooser running.
        if !waited {
            eprintln!("{name}: {pid} has the screen and will not give it up");
            return Alone::No;
        }

        if holding == name {
            return Alone::No;
        }
    }

    kept(&mut held, handle, name)
}

/// The screen is this process's now.
///
/// The door is left blank until it has been drawn on, so that a second press
/// is let through to the one already coming rather than closing it.
fn kept(held: &mut Option<Holding>, mut handle: File, name: &str) -> Alone {
    written(&mut handle, "");
    *held = Some(Holding { handle, name: name.to_string() });
    Alone::Yes
}

/// The chooser is on the screen.
///
/// Until this is said the door is left blank, so that a second press is let
/// through to the one already coming rather than closing it.
pub fn drawn() {
    let mut held = holding();

    let Some(holding) = held.as_mut() else { return };

    let name = holding.name.clone();
    written(&mut holding.handle, &name);
}

/// The chooser is off the screen, and what is left is this process going.
///
/// The door goes blank rather than staying named, so a press arriving while
/// the last of it is winding down waits for the screen instead of reading it
/// as a chooser to close. A chooser whose window has gone but which is still
/// holding the lock is what one press in every open-and-close disappeared
/// into.
pub fn gone() {
    let mut held = holding();

    let Some(holding) = held.as_mut() else { return };

    written(&mut holding.handle, "");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_lives_under_the_sessions_own_runtime() {
        // SAFETY: one thread, and the variable is put back before it ends.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000") };
        assert_eq!(where_(), Path::new("/run/user/1000/console/chooser.lock"));
        unsafe { std::env::remove_var("XDG_RUNTIME_DIR") };
        assert_eq!(where_(), Path::new("/tmp/console/chooser.lock"));
    }

    /// Two of the bar's icons are the same program at different tabs, so what
    /// is written down is the door rather than the program.
    #[test]
    fn the_file_says_who_is_holding_it_and_which_door_they_came_out_of() {
        assert_eq!(holder("1234 settings sound"), (1234, "settings sound"));
        assert_eq!(holder("1234 "), (1234, ""));
    }

    /// The door is left blank until the chooser is on the screen, which is
    /// how a press that arrives while one is coming is told from a press
    /// putting one away.
    #[test]
    fn a_chooser_on_its_way_has_a_pid_and_no_door() {
        assert_eq!(holder("1234"), (1234, ""));
    }

    /// Which is what the bell on the bar asks for: a panel with no tab named,
    /// whose door would otherwise be written with the space where the tab was
    /// not and read back without it.
    #[test]
    fn a_door_named_for_a_tab_it_was_not_given_is_the_name_on_its_own() {
        let name = door("notices ");
        assert_eq!(name, "notices");
        assert_eq!(holder(&format!("1234 {name}")), (1234, name));
    }

    #[test]
    fn a_file_saying_nothing_names_nobody() {
        assert_eq!(holder(""), (0, ""));
        assert_eq!(holder("what"), (0, ""));
    }
}

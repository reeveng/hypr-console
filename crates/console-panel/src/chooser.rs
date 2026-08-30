//! One chooser at a time, and the door that opened it closes it.
//!
//! A chooser takes the controller while it is up: the buttons stop being the
//! desktop's and become move the highlight, confirm, and back out. It gives
//! them back when it goes. Two choosers at once and that is no longer true of
//! either of them. The second to open takes the profile the first was relying
//! on, and the first to close hands the desktop's buttons back while the other
//! is still on screen, so what you are looking at is being driven by the
//! buttons of something you cannot see.
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
//! two in order: the one going hands the controller back before it dies, and
//! the lock is not free until it has.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

/// The wait between tries at the lock, and how many of them.
///
/// Long enough to outlast the pad being handed back, which is the slowest
/// thing the one going does and the reason it is still holding the lock while
/// it does it. Waiting less than that turns a panel asked for right after
/// another closed into a panel that never draws.
pub const BREATH: Duration = Duration::from_millis(20);
pub const PATIENCE: usize =
    2 * (crate::running::A_WORD.as_millis() / BREATH.as_millis()) as usize;

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
    if pid > 0 {
        // SAFETY: a signal to a pid this process started and has not reaped.
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }
}

/// The lock's file, under whatever this session calls its runtime.
pub fn where_() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    let runtime = match runtime.is_empty() {
        true => "/tmp".to_string(),
        false => runtime,
    };
    Path::new(&runtime).join("console").join("chooser.lock")
}

/// Try the lock once, without waiting for it.
pub fn take(handle: &File) -> bool {
    // SAFETY: the descriptor is this file's, and open for as long as the call.
    unsafe { libc::flock(handle.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 }
}

/// Who has it, and which door they came out of.
///
/// The name written beside the pid is the door, not the program. Two of the
/// bar's icons are the same program at different tabs, and tapping one while
/// the other is up should move the panel rather than close it.
pub fn holder(said: &str) -> (i32, &str) {
    let (pid, name) = said.trim().split_once(' ').unwrap_or((said.trim(), ""));
    (pid.parse().unwrap_or(0), name)
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
        if take(handle) {
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
pub fn put_away() -> bool {
    let Ok(mut handle) = OpenOptions::new().read(true).write(true).open(where_()) else {
        return false;
    };
    // Nobody is holding it, so there is nothing on screen to put away.
    if take(&handle) {
        return false;
    }
    let (pid, _) = holder(&read(&mut handle));
    if pid <= 0 {
        return false;
    }
    // SAFETY: a signal to a pid, which is what the file said was there.
    unsafe { libc::kill(pid, libc::SIGTERM) };
    true
}

/// True if this process may be the chooser, false if it may not.
///
/// False means there is nothing more to do: either the panel that was up has
/// been closed by this call, which is what the same door asked twice means, or
/// something else is holding the screen and will not let go, or one is on its
/// way and this press is somebody who has not seen it yet.
pub fn alone(name: &str, again: Again) -> bool {
    let name = door(name);
    let mut held = HELD.lock().expect("the lock's own lock");
    if held.is_some() {
        return true;
    }
    let path = where_();
    let opened = path
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path));
    // Nowhere to keep a lock is not a reason to refuse.
    let Ok(mut handle) = opened else { return true };

    if !take(&handle) {
        let said = read(&mut handle);
        let (pid, holding) = holder(&said);
        // A second chooser inside one process is a program asking twice, and
        // there is no taking the screen from yourself.
        if pid == 0 || pid == std::process::id() as i32 {
            return false;
        }
        // Asked again through the door it came out of, by something that only
        // opens: it is open.
        if holding == name && again == Again::Keeps {
            eprintln!("{name}: {pid} is showing it, and this door only opens");
            return false;
        }
        // Somebody has the lock but has not drawn yet. A press now is a
        // thumb that has not seen the chooser rather than one putting it
        // away, and closing what has not appeared is how one press became
        // two: the paddle cancelled the menu it had just asked for, and the
        // next press was the one that seemed to work. So wait for it, and
        // take the screen only if it never comes.
        if holding.is_empty() {
            match meanwhile(&mut handle) {
                Meanwhile::Drawn => return false,
                Meanwhile::Free => return kept(&mut held, handle, name),
                Meanwhile::Stuck => {}
            }
        }
        // SAFETY: a signal to a pid, which is what the file said was there.
        unsafe { libc::kill(pid, libc::SIGTERM) };
        let waited = (0..PATIENCE).any(|_| {
            let got = take(&handle);
            if !got {
                std::thread::sleep(BREATH);
            }
            got
        });
        // It will not go, and two of them is worse. Say so: a press that
        // does nothing is otherwise a button reported as broken, with a
        // journal that shows the press arriving and the chooser running.
        if !waited {
            eprintln!("{name}: {pid} has the screen and will not give it up");
            return false;
        }
        if holding == name {
            return false;
        }
    }
    kept(&mut held, handle, name)
}

/// The screen is this process's now.
///
/// The door is left blank until it has been drawn on, so that a second press
/// is let through to the one already coming rather than closing it.
fn kept(held: &mut Option<Holding>, mut handle: File, name: &str) -> bool {
    written(&mut handle, "");
    *held = Some(Holding { handle, name: name.to_string() });
    true
}

/// The chooser is on the screen.
///
/// Until this is said the door is left blank, so that a second press is let
/// through to the one already coming rather than closing it.
pub fn drawn() {
    let mut held = HELD.lock().expect("the lock's own lock");
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
    let mut held = HELD.lock().expect("the lock's own lock");
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

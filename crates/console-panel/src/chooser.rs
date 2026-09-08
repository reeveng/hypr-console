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
//!
//! And one that will not go is taken off the screen. A panel whose compositor
//! has gone spins on a socket that is hung up and answers nothing, which is the
//! whole of `asked`'s argument, and it holds the lock while it spins: every
//! panel asked for after it is turned away, and the button that opens one looks
//! broken to the hand that keeps pressing it. Being asked is the politeness
//! there is, and it is bounded. After that the screen is taken, because the
//! kernel drops the lock as the process ends and stopping is the one thing a
//! wedged chooser can still be made to do.


use console_core_never::Never;
use console_waiting::{Patience, Seen, Waited, until};
use console_core_number_conversion::fitted;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const BREATH: Duration = Duration::from_millis(20);

pub const PATIENCE: Duration = Duration::from_secs(10);

pub const COMING: Duration = Duration::from_secs(2);

static HELD: Mutex<Option<Holding>> = Mutex::new(None);

struct Holding {
    handle: File,
    name: String,
}

static SHOWING: AtomicI32 = AtomicI32::new(0);

static WAITED: AtomicU64 = AtomicU64::new(0);

pub fn waited_for_screen() -> Result<Duration, Never> {
    Ok(Duration::from_nanos(WAITED.load(Ordering::SeqCst)))
}

struct Asking(Instant);

impl Drop for Asking {
    fn drop(&mut self) {
        let Ok(whole) = fitted(self.0.elapsed().as_nanos());
        WAITED.store(whole, Ordering::SeqCst);
    }
}

pub fn showing(pid: i32) -> Result<(), Never> {
    SHOWING.store(pid, Ordering::SeqCst);
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

    Ok(())
}

pub fn showing_nothing() -> Result<(), Never> {
    SHOWING.store(0, Ordering::SeqCst);

    Ok(())
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

pub fn where_() -> Result<PathBuf, Never> {
    let runtime = match std::env::var("XDG_RUNTIME_DIR") {
        Ok(runtime) => Some(runtime),
        Err(_) => None,
    };

    let screen = match std::env::var("WAYLAND_DISPLAY") {
        Ok(screen) => Some(screen),
        Err(_) => None,
    };

    under(runtime.as_deref(), screen.as_deref())
}

fn under(runtime: Option<&str>, screen: Option<&str>) -> Result<PathBuf, Never> {
    let runtime = match runtime {
        Some(runtime) if !runtime.is_empty() => runtime,
        Some(_) | None => "/tmp",
    };

    let screen = match screen {
        Some(screen) if !screen.is_empty() => screen,
        Some(_) | None => "no-screen-named",
    };

    Ok(Path::new(runtime).join("console").join(format!("chooser-{screen}.lock")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Took {
    It,
    Not,
}

pub fn take(handle: &File) -> Result<Took, Never> {
    // SAFETY: the descriptor is this file's, and open for as long as the call.
    Ok(match unsafe { libc::flock(handle.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 } {
        true => Took::It,
        false => Took::Not,
    })
}

pub fn holder(said: &str) -> Result<(i32, &str), Never> {
    let (pid, name) = said.trim().split_once(' ').unwrap_or((said.trim(), ""));

    let pid = match pid.parse::<i32>() {
        Ok(pid) => pid,
        Err(_fault) => return Ok((0, name)),
    };

    Ok((pid, name))
}

fn door(name: &str) -> Result<&str, Never> {
    Ok(name.trim())
}

enum Meanwhile {
    Drawn,
    Free,
    Stuck,
}

fn meanwhile(handle: &mut File) -> Result<Meanwhile, Never> {
    let Ok(patience) = Patience::asking_every(COMING, BREATH);
    let mut answer = Meanwhile::Stuck;
    let Ok(_settled) = until(patience, || {
        let Ok(took) = take(handle);

        match took == Took::It {
            true => {
                answer = Meanwhile::Free;

                return Ok(Seen::Yes);
            }
            false => {},
        }

        let Ok(said) = read(handle);
        let Ok((_pid, drawn)) = holder(&said);

        Ok(match !drawn.is_empty() {
            true => {
                answer = Meanwhile::Drawn;

                Seen::Yes
            }
            false => Seen::NotYet,
        })
    });

    Ok(answer)
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

pub fn put_away() -> Result<Away, Never> {
    let Ok(where_) = where_();

    let mut handle = match OpenOptions::new().read(true).write(true).open(where_) {
        Ok(handle) => handle,
        Err(_fault) => return Ok(Away::Nothing),
    };

    let Ok(took) = take(&handle);

    match took == Took::It {
        true => return Ok(Away::Nothing),
        false => {},
    }

    let Ok(said) = read(&mut handle);
    let Ok((pid, _door)) = holder(&said);

    match pid <= 0 {
        true => return Ok(Away::Nothing),
        false => {},
    }

    // SAFETY: a signal to a pid, which is what the file said was there.
    unsafe { libc::kill(pid, libc::SIGTERM) };

    Ok(Away::Told)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Away {
    Told,
    Nothing,
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
    let _asking = Asking(Instant::now());
    let Ok(name) = door(name);
    let Ok(mut held) = holding();

    match held.is_some() {
        true => return Ok(Alone::Yes),
        false => {},
    }

    let Ok(path) = where_();
    let opened = path
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path));

    let mut handle = match opened {
        Ok(handle) => handle,
        Err(_fault) => return Ok(Alone::Yes),
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
                        Meanwhile::Drawn => return Ok(Alone::No),
                        Meanwhile::Free => return kept(&mut held, handle, name),
                        Meanwhile::Stuck => {}
                    }
                }
                false => {},
            }

            // SAFETY: a signal to a pid, which is what the file said was there.
            unsafe { libc::kill(pid, libc::SIGTERM) };

            let Ok(asked) = given_up(&handle, PATIENCE);

            match asked {
                Waited::Happened => {},
                Waited::RanOut => {
                    eprintln!("{name}: {pid} would not give the screen up, and is taken off it");

                    // SAFETY: the same pid the file named, asked once already.
                    unsafe { libc::kill(pid, libc::SIGKILL) };

                    let Ok(taken) = given_up(&handle, COMING);

                    match taken {
                        Waited::Happened => {},
                        Waited::RanOut => {
                            eprintln!("{name}: {pid} was taken off the screen and it is not free");

                            return Ok(Alone::No);
                        }
                    }
                }
            }

            match holding == name {
                true => return Ok(Alone::No),
                false => {},
            }
        }
        false => {},
    }

    kept(&mut held, handle, name)
}

fn given_up(handle: &File, patience: Duration) -> Result<Waited, Never> {
    let Ok(patience) = Patience::asking_every(patience, BREATH);

    until(patience, || {
        let Ok(got) = take(handle);

        Ok(match got == Took::It {
            true => Seen::Yes,
            false => Seen::NotYet,
        })
    })
}

fn kept(held: &mut Option<Holding>, mut handle: File, name: &str) -> Result<Alone, Never> {
    let Ok(()) = written(&mut handle, "");

    *held = Some(Holding { handle, name: name.to_string() });

    Ok(Alone::Yes)
}

pub fn drawn() -> Result<(), Never> {
    let Ok(mut held) = holding();

    let holding = match held.as_mut() {
        Some(holding) => holding,
        None => return Ok(()),
    };

    let name = holding.name.clone();
    let Ok(()) = written(&mut holding.handle, &name);

    Ok(())
}

pub fn gone() -> Result<(), Never> {
    let Ok(mut held) = holding();

    let holding = match held.as_mut() {
        Some(holding) => holding,
        None => return Ok(()),
    };

    let Ok(()) = written(&mut holding.handle, "");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_lives_under_the_sessions_own_runtime() {
        assert_eq!(
            under(Some("/run/user/1000"), Some("wayland-1")),
            Ok(PathBuf::from("/run/user/1000/console/chooser-wayland-1.lock"))
        );
        assert_eq!(
            under(None, Some("wayland-1")),
            Ok(PathBuf::from("/tmp/console/chooser-wayland-1.lock"))
        );
        assert_eq!(under(None, None), Ok(PathBuf::from("/tmp/console/chooser-no-screen-named.lock")));
    }

    #[test]
    fn a_variable_set_to_nothing_has_not_named_a_screen() {
        assert_eq!(
            under(Some(""), Some("")),
            Ok(PathBuf::from("/tmp/console/chooser-no-screen-named.lock"))
        );
    }

    #[test]
    fn two_screens_under_one_runtime_are_two_locks() {
        let Ok(login) = under(Some("/run/user/1000"), Some("wayland-1"));
        let Ok(nested) = under(Some("/run/user/1000"), Some("wayland-7"));

        assert_ne!(login, nested);
    }

    #[test]
    fn the_file_says_who_is_holding_it_and_which_door_they_came_out_of() {
        assert_eq!(holder("1234 settings sound"), Ok((1234, "settings sound")));
        assert_eq!(holder("1234 "), Ok((1234, "")));
    }

    #[test]
    fn a_chooser_on_its_way_has_a_pid_and_no_door() {
        assert_eq!(holder("1234"), Ok((1234, "")));
    }

    #[test]
    fn a_door_named_for_a_tab_it_was_not_given_is_the_name_on_its_own() {
        let Ok(name) = door("notices ");

        assert_eq!(name, "notices");
        assert_eq!(holder(&format!("1234 {name}")), Ok((1234, name)));
    }

    #[test]
    fn a_file_saying_nothing_names_nobody() {
        assert_eq!(holder(""), Ok((0, "")));
        assert_eq!(holder("what"), Ok((0, "")));
    }
}

//! The one picture on a card that changes while nobody presses anything.
//!
//! A row's square comes out of the picture store, at the size a row draws it,
//! made by another program for the next opening. A photograph being looked at
//! is drawn across most of the card, and a store of every one of those at that
//! size would be a store the size of the pictures folder -- so it is not kept
//! there. What is held here is the one picture on the screen: decoded on a
//! thread of its own the first time a card asks for it, and let go when a card
//! asks for another.
//!
//! A film is the same picture changing. Whoever plays it hands each frame to
//! [`put`], and the frame replaces the last one. The decode that was started
//! for the still is what a film shows before it starts, and it cannot land on
//! top of a frame the player has already put.
//!
//! **The draw loop is told rather than asked.** The loop sleeps in `poll` until
//! the compositor says something, and a picture that finished decoding while it
//! slept would stay undrawn until the next press. So there is a socket here,
//! and every change says which of three things it is: a frame, which repaints
//! the picture's own rectangle and nothing else; the card, which draws the
//! whole card again from the rows it already has; and the rows, which asks the
//! page for them again, because a film's clock under the picture has moved on.
//! A frame thirty times a second is thirty rectangles, and the words, the bar
//! and the buttons are painted when they change and not when the film does.
//!
//! The same socket is how a card is shut. The host sets a flag and waits for
//! the loop to end, and a flag is only read when the loop next wakes -- which,
//! with nothing loading, was up to a whole resting poll later. Measured, a
//! panel took as long as a tenth of a second to close, all of it spent asleep,
//! and the host could open nothing else meanwhile. So shutting says so here
//! too, and the loop wakes to read the flag at once.
//!
//! The loop has one other sleep, and it is here for the same reason. A tab with
//! nothing on it yet holds its first frame back a moment for its rows, and that
//! was a wait on the rows alone, which a shutting could not end: the music card
//! asks the player several things before it has a row, and a close that landed
//! meanwhile waited out the rest of the moment. So the first look listens on
//! this socket too, beside a [`deadline`] the kernel holds for the moment. A
//! byte is only *look again*: a picture that finished decoding writes one as
//! well, and a look that ended on it drew the viewer empty under a press that
//! then landed on nothing. What ends the look is the rows, the shut flag or the
//! deadline, each asked after every byte.

use std::io::{Read, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_shapes::Pixels;
use rustix::event::{Nsecs, PollFd, PollFlags, Secs, poll};
use rustix::time::{Itimerspec, Timespec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, timerfd_create, timerfd_settime};

#[derive(Debug, Clone, PartialEq, Eq)]
enum FrameState {
    Decoding,
    Decoded(Pixels),
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Looked {
    of: PathBuf,
    room: Size<u32>,
    held: FrameState,
    asked: Size<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    Frame,
    Card,
    Rows,
    Closed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Woken {
    pub frame: FrameReceived,
    pub card: FrameReceived,
    pub rows: FrameReceived,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FrameReceived {
    Yes,
    #[default]
    No,
}

impl FrameReceived {
    fn or(self, later: FrameReceived) -> Result<FrameReceived, Never> {
        Ok(match (self, later) {
            (FrameReceived::No, FrameReceived::No) => FrameReceived::No,
            (FrameReceived::Yes, _) | (FrameReceived::No, FrameReceived::Yes) => FrameReceived::Yes,
        })
    }
}

impl Woken {
    pub fn and(self, later: Woken) -> Result<Woken, Never> {
        let Ok(frame) = self.frame.or(later.frame);
        let Ok(card) = self.card.or(later.card);
        let Ok(rows) = self.rows.or(later.rows);

        Ok(Woken { frame, card, rows })
    }
}

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the one picture on the screen, handed from the thread that decoded or played it to the loop that draws it; the loop builds its rows from a page it did not write, so there is no value of its own this could be carried in"
    )
)]
static LOOKING_AT: Mutex<Option<Looked>> = Mutex::new(None);

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the two ends of the socket the draw loop sleeps on: whoever changes the picture has to reach the loop that is sleeping, and neither was handed the other"
    )
)]
static WAKE: OnceLock<Option<(UnixStream, UnixStream)>> = OnceLock::new();

fn wake() -> Result<Option<&'static (UnixStream, UnixStream)>, Never> {
    Ok(WAKE
        .get_or_init(|| match UnixStream::pair() {
            Ok((told, telling)) => {
                let quiet = told.set_nonblocking(true).and(telling.set_nonblocking(true));

                match quiet {
                    Ok(()) => Some((told, telling)),
                    Err(fault) => {
                        eprintln!("console-panel: a picture cannot wake the card: {fault}");

                        None
                    },
                }
            },
            Err(fault) => {
                eprintln!("console-panel: a picture cannot wake the card: {fault}");

                None
            },
        })
        .as_ref())
}

pub fn waking() -> Result<Option<BorrowedFd<'static>>, Never> {
    let Ok(wake) = wake();

    Ok(wake.map(|(told, _)| told.as_fd()))
}

pub fn tell(what: Notice) -> Result<(), Never> {
    let Ok(wake) = wake();

    let mut telling = match wake {
        Some((_, telling)) => telling,
        None => return Ok(()),
    };

    let byte = match what {
        Notice::Frame => b'f',
        Notice::Card => b'c',
        Notice::Rows => b'r',
        Notice::Closed => b's',
    };

    let _a_full_socket_has_already_woken_the_loop = telling.write(&[byte]);

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listened {
    Notified,
    RanOut,
}

pub fn deadline(until: Duration) -> Result<Option<OwnedFd>, Never> {
    let Ok(seconds) = fitted::<u64, Secs>(until.as_secs());
    let Ok(nanoseconds) = fitted::<u32, Nsecs>(until.subsec_nanos());

    let timer = match timerfd_create(TimerfdClockId::Monotonic, TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK) {
        Ok(timer) => timer,
        Err(fault) => {
            eprintln!("console-panel: a first look has no deadline, so it is not taken: {fault}");

            return Ok(None);
        },
    };

    let once = Itimerspec {
        it_interval: Timespec { tv_sec: 0, tv_nsec: 0 },
        it_value: Timespec { tv_sec: seconds, tv_nsec: nanoseconds },
    };

    Ok(match timerfd_settime(&timer, TimerfdTimerFlags::empty(), &once) {
        Ok(_before) => Some(timer),
        Err(fault) => {
            eprintln!("console-panel: a first look has no deadline, so it is not taken: {fault}");

            None
        },
    })
}

pub fn listened(deadline: Option<&OwnedFd>) -> Result<Listened, Never> {
    let Ok(waking) = waking();

    let deadline = match deadline {
        Some(deadline) => deadline,
        None => return Ok(Listened::RanOut),
    };

    let mut watch = vec![PollFd::new(deadline, PollFlags::IN)];

    watch.extend(waking.map(|waking| PollFd::from_borrowed_fd(waking, PollFlags::IN)));

    let _told_or_not_what_happened_is_read_from_the_watch = poll(&mut watch, None);

    Ok(match watch.first().map(|timer| timer.revents().contains(PollFlags::IN)) {
        Some(true) => Listened::RanOut,
        Some(false) | None => Listened::Notified,
    })
}

pub fn woken() -> Result<Woken, Never> {
    let Ok(wake) = wake();

    let mut told = match wake {
        Some((told, _)) => told,
        None => return Ok(Woken::default()),
    };

    let mut heard = Woken::default();
    let mut read = [0u8; 64];

    loop {
        let bytes = match told.read(&mut read) {
            Ok(0) => return Ok(heard),
            Err(_the_read_failed) => return Ok(heard),
            Ok(many) => read.iter().take(many),
        };

        for byte in bytes {
            match byte {
                b'f' => heard.frame = FrameReceived::Yes,
                b'c' => heard.card = FrameReceived::Yes,
                b'r' => heard.rows = FrameReceived::Yes,
                _somebody_else => {},
            }
        }
    }
}

pub fn current(of: &Path, room: Size<u32>) -> Result<Option<Pixels>, Never> {
    let mut holding = match LOOKING_AT.lock() {
        Ok(holding) => holding,
        Err(_a_decode_gave_up_holding_it) => return Ok(None),
    };

    match holding.as_mut() {
        Some(looked) => match looked.of.as_path() == of {
            true => {
                looked.room = room;

                return Ok(match &looked.held {
                    FrameState::Decoded(pixels) => Some(pixels.clone()),
                    FrameState::Decoding | FrameState::Failed => None,
                });
            },
            false => {},
        },
        None => {},
    }

    *holding = Some(Looked { of: of.to_path_buf(), room, held: FrameState::Decoding, asked: room });

    let at = of.to_path_buf();
    let decoding = std::thread::spawn(move || {
        let read = match console_pictures::decoded(&at, room) {
            Ok(Some(pixels)) => FrameState::Decoded(pixels),
            Ok(None) => FrameState::Failed,
            Err(why) => {
                eprintln!("console-panel: no picture: {why}");

                FrameState::Failed
            },
        };

        let Ok(()) = landed(&at, read);
    });

    let Ok(()) = console_program_lifetime::threads::let_go(decoding);

    Ok(None)
}

fn landed(at: &Path, read: FrameState) -> Result<(), Never> {
    let mut holding = match LOOKING_AT.lock() {
        Ok(holding) => holding,
        Err(_a_reader_gave_up_holding_it) => return Ok(()),
    };

    let still_wanted = holding
        .as_ref()
        .is_some_and(|looked| looked.of.as_path() == at && looked.held == FrameState::Decoding);

    match still_wanted {
        true => {
            match holding.as_mut() {
                Some(looked) => looked.held = read,
                None => {},
            }

            drop(holding);

            tell(Notice::Card)
        },
        false => Ok(()),
    }
}

pub fn sharpened(of: &Path, within: Size<u32>) -> Result<(), Never> {
    let mut holding = match LOOKING_AT.lock() {
        Ok(holding) => holding,
        Err(_a_decode_gave_up_holding_it) => return Ok(()),
    };

    let looked = match holding.as_mut() {
        Some(looked) => looked,
        None => return Ok(()),
    };

    let larger = within.width > looked.asked.width || within.height > looked.asked.height;

    match (looked.of.as_path() == of, &looked.held, larger) {
        (true, FrameState::Decoded(_), true) => {},
        (false, _, _) | (true, FrameState::Decoding | FrameState::Failed, _) | (true, FrameState::Decoded(_), false) => return Ok(()),
    }

    looked.asked = within;

    drop(holding);

    let at = of.to_path_buf();
    let decoding = std::thread::spawn(move || {
        let read = match console_pictures::decoded(&at, within) {
            Ok(Some(pixels)) => pixels,
            Ok(None) => return,
            Err(why) => {
                eprintln!("console-panel: no sharper picture: {why}");

                return;
            },
        };

        let Ok(()) = sharper(&at, within, read);
    });

    console_program_lifetime::threads::let_go(decoding)
}

fn sharper(at: &Path, within: Size<u32>, read: Pixels) -> Result<(), Never> {
    let mut holding = match LOOKING_AT.lock() {
        Ok(holding) => holding,
        Err(_a_reader_gave_up_holding_it) => return Ok(()),
    };

    let looked = match holding.as_mut() {
        Some(looked) => looked,
        None => return Ok(()),
    };

    match looked.of.as_path() == at && looked.asked == within {
        true => looked.held = FrameState::Decoded(read),
        false => return Ok(()),
    }

    drop(holding);

    tell(Notice::Card)
}

pub fn room(of: &Path) -> Result<Option<Size<u32>>, Never> {
    let holding = match LOOKING_AT.lock() {
        Ok(holding) => holding,
        Err(_a_decode_gave_up_holding_it) => return Ok(None),
    };

    Ok(holding.as_ref().filter(|looked| looked.of.as_path() == of).map(|looked| looked.room))
}

pub fn put(of: &Path, pixels: Pixels) -> Result<(), Never> {
    let mut holding = match LOOKING_AT.lock() {
        Ok(holding) => holding,
        Err(_a_reader_gave_up_holding_it) => return Ok(()),
    };

    let room = match holding.as_ref() {
        Some(looked) => match looked.of.as_path() == of {
            true => looked.room,
            false => return Ok(()),
        },
        None => return Ok(()),
    };

    *holding = Some(Looked { of: of.to_path_buf(), room, held: FrameState::Decoded(pixels), asked: room });

    drop(holding);

    tell(Notice::Frame)
}

#[cfg(test)]
#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the picture on the screen is one per process and the tests share the process, so the tests that look at it take turns"
    )
)]
pub(crate) static ONE_TEST_AT_A_TIME: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    fn a_frame(red: u8) -> Pixels {
        Pixels { width: 2, height: 1, stride: 8, bytes: Arc::new(vec![red, 0, 0, 255, red, 0, 0, 255]) }
    }

    fn patience() -> console_waiting::Schedule {
        console_waiting::Schedule::of(Duration::from_secs(20)).expect("a patience")
    }

    #[test]
    fn a_picture_asked_for_is_decoded_away_from_the_loop_and_the_loop_is_told() {
        let _turn = ONE_TEST_AT_A_TIME.lock();
        let at = std::env::temp_dir().join(format!("console-panel-frames-{}.png", std::process::id()));
        let Ok(mut making) = console_core_external_programs::Program::Ffmpeg.command();
        let made = making
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", "color=c=red:s=40x30", "-frames:v", "1"])
            .arg(&at)
            .status();

        assert!(made.is_ok_and(|how| how.success()), "ffmpeg made no picture to look at");

        let room = Size { width: 80, height: 80 };

        assert_eq!(current(&at, room), Ok(None), "the first ask is answered at once, with nothing yet");

        let mut told = Woken::default();
        let Ok(drawn) = console_waiting::found(patience(), || {
            let Ok(heard) = woken();

            match heard.card {
                FrameReceived::Yes => told.card = FrameReceived::Yes,
                FrameReceived::No => {},
            }

            current(&at, room)
        });
        let _ = std::fs::remove_file(&at);

        let drawn = drawn.expect("the picture was never decoded");

        assert_eq!((drawn.width, drawn.height), (80, 60));

        let Ok(heard) = woken();

        match heard.card {
            FrameReceived::Yes => told.card = FrameReceived::Yes,
            FrameReceived::No => {},
        }

        assert_eq!(told.card, FrameReceived::Yes, "the loop was never told the card had changed");

        let film = Path::new("/nowhere/a-film.mkv");

        assert_eq!(current(film, room), Ok(None));
        assert_eq!(room_of(film), Some(room));

        let Ok(()) = put(film, a_frame(200));
        let Ok(heard) = woken();

        assert_eq!(heard.frame, FrameReceived::Yes, "a frame put is a frame the loop hears about");
        assert_eq!(current(film, room), Ok(Some(a_frame(200))));

        let Ok(()) = landed(film, FrameState::Failed);

        assert_eq!(current(film, room), Ok(Some(a_frame(200))), "a late still does not cover a frame");

        let Ok(()) = put(Path::new("/nowhere/another.mkv"), a_frame(9));

        assert_eq!(current(film, room), Ok(Some(a_frame(200))), "a frame of a film nobody is looking at is dropped");
    }

    fn room_of(of: &Path) -> Option<Size<u32>> {
        let Ok(room) = room(of);

        room
    }
}

//! Somewhere a check can be run, and what can be seen from there.
//!
//! A check says what somebody did and what should have happened. Where it is
//! run decides how the doing is done and how much of the happening can be seen
//! at all.
//!
//! ```text
//! here      emulated devices, the daemon in this process, no machine
//!           involved. What can be seen is what the daemon decided to run.
//!
//! desktop   the device's own desktop, nested on this machine, and looked at.
//!           What this can answer that nothing else can is what colour the
//!           screen is, and what it presses with is the pointer -- a finger
//!           needs uinput, and uinput belongs to the machine this is nested on
//!           rather than to the picture.
//!
//! panels    one panel, opened alone in the nested desktop and asked what it
//!           drew. The tier a change to a panel is tried in while it is being
//!           written: one program, a few seconds, no device. What it can answer
//!           is not whether a thing is drawn but whether a hand could use it.
//!
//! device    the Legion Go itself, over ssh. The pressing goes through
//!           InputPlumber's own SendEvent, so a button, a stick and a trigger
//!           all arrive exactly as the hardware's would, through the loaded
//!           profile; a finger arrives through the touchscreen `console-tap`
//!           makes, and the pointer through `console-point`. What can be seen
//!           is the machine: which workspace, which windows, how bright,
//!           whether the keyboard is up, which profile is loaded, and what
//!           colour any place on the screen is.
//! ```
//!
//! The same check runs in more than one of them. It cannot assert the same
//! things in each, so it says what it needs to be able to see by which stage it
//! is written for, and a stage nothing is written for skips it and says so
//! rather than passing quietly.
//!
//! The last of them is somebody's machine and is lent rather than given, so a
//! run there is bracketed by `putting_back`: what was true before anything was
//! pressed is read once, and put back once the last check has had its turn.
//! `stopping` is the same promise kept when a run is interrupted, which is the
//! moment it matters most.

use console_core_geometry::{Point, Size};
use std::fmt;
use std::path::PathBuf;

pub mod baseline;
pub mod checking;
pub mod desktop;
pub mod device;
pub mod here;
pub mod lasting;
pub mod palette;
pub mod panels;
pub mod picture;
pub mod plug;
pub mod putting_back;
pub mod stopping;
pub mod watching;

#[derive(Debug)]
pub enum Awry {
    Machine(std::io::Error),
    Unwritten(console_core_atomic_writes::Unwritten),
    NoPointer(PathBuf),
    PointerNotBeside,
    AlreadyTaken(&'static str),
    NoBus(PathBuf),
    TookNoPicture(String),
    NestedPictureGone,
    NoScreenSaid(PathBuf, std::io::Error),
    NoScreenAtAll(PathBuf),
    NoSize(PathBuf, String),
    NoWindowsSaid(PathBuf, std::io::Error),
    Asking(console_compositor::Unanswered),
    Pressing(console_input_gamepad::Unpressed),
    Layers(serde_json::Error),
    Unreadable(PathBuf, std::io::Error),
    NotAPicture(PathBuf, cairo::IoError),
    NothingToRead(cairo::BorrowError),
    OffTheEdge(Point<i64>, Size<u32>),
    Hostless,
    Unnamed(console_device::naming::Unnamed),
    DeviceWroteNoPicture,
    DevicePictureGone,
    Undeclared(console_screen::Undeclared),
    NotInside(Point<i32>, String),
    NamelessKey,
    NotBuilt(PathBuf),
    Stale(PathBuf),
    SaidNothingDrawn(String, std::io::Error, String),
    Untold(console_panel::telling::Untold),
    DrewNothing(String),
    OfferUnanswered(String, Vec<String>),
    MarkWithoutOffer(String),
    TooManyMarks(String, usize),
    OutOfReach(String, (i32, i32), Vec<String>),
    NoWayOut(String, String),
}

impl fmt::Display for Awry {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Awry::Machine(fault) => write!(to, "{fault}"),
            Awry::Unwritten(fault) => write!(to, "{fault}"),
            Awry::NoPointer(at) => write!(
                to,
                "{} is not there, so nothing would be pressed: cargo build -p console-input-pointer",
                at.display()
            ),
            Awry::PointerNotBeside => {
                write!(to, "console-point is not beside console-desktop")
            }
            Awry::AlreadyTaken(what) => {
                write!(to, "the picture has already been taken; {what} before looking")
            }
            Awry::NoBus(at) => write!(
                to,
                "a bus of this check's own never listened on {}",
                at.display()
            ),
            Awry::TookNoPicture(last) => {
                write!(to, "the nested desktop took no picture: {last}")
            }
            Awry::NestedPictureGone => {
                write!(to, "the nested desktop took a picture and then had none")
            }
            Awry::NoScreenSaid(at, fault) => write!(
                to,
                "{}: the nested desktop said nothing about its screen: {fault}",
                at.display()
            ),
            Awry::NoScreenAtAll(at) => write!(
                to,
                "{}: the nested desktop had no screen at all",
                at.display()
            ),
            Awry::NoSize(at, named) => write!(
                to,
                "{} says nothing about how big {named} is",
                at.display()
            ),
            Awry::NoWindowsSaid(at, fault) => write!(
                to,
                "{}: the nested desktop said no windows: {fault}",
                at.display()
            ),
            Awry::Asking(fault) => write!(to, "{fault}"),
            Awry::Pressing(fault) => write!(to, "{fault}"),
            Awry::Layers(fault) => write!(to, "layers: {fault}"),
            Awry::Unreadable(at, fault) => write!(to, "{}: {fault}", at.display()),
            Awry::NotAPicture(at, fault) => {
                write!(to, "{} is not a picture: {fault}", at.display())
            }
            Awry::NothingToRead(fault) => write!(to, "nothing to read: {fault}"),
            Awry::OffTheEdge(spot, room) => write!(
                to,
                "{},{} is off the edge of a {}x{} picture",
                spot.across, spot.down, room.wide, room.tall
            ),
            Awry::Hostless => write!(
                to,
                "CONSOLE_HOST is not set, so there is no device to talk to. \
                  Set it to the device, as in CONSOLE_HOST=root@handheld."
            ),
            Awry::Unnamed(fault) => write!(to, "{fault}"),
            Awry::DeviceWroteNoPicture => {
                write!(to, "the device never wrote a picture to /tmp")
            }
            Awry::DevicePictureGone => {
                write!(to, "the device took a picture and then had none")
            }
            Awry::Undeclared(fault) => write!(to, "{fault}"),
            Awry::NotInside(spot, panel) => write!(
                to,
                "({}, {}) is not a place inside {panel}",
                spot.across, spot.down
            ),
            Awry::NamelessKey => write!(to, "a key with no name"),
            Awry::NotBuilt(at) => write!(
                to,
                "{} is not in target/debug, so there would be nothing to open: \
                 cargo build --workspace",
                at.display()
            ),
            Awry::Stale(at) => write!(
                to,
                "{} was built before console-panel was last edited, so this would hold \
                 today's rules against yesterday's panel: cargo build --workspace",
                at.display()
            ),
            Awry::SaidNothingDrawn(program, fault, why) => write!(
                to,
                "{program} said nothing about what it drew ({fault}):\n{why}"
            ),
            Awry::Untold(fault) => write!(to, "{fault}"),
            Awry::DrewNothing(program) => write!(to, "{program} drew nothing at all"),
            Awry::OfferUnanswered(panel, missing) => write!(
                to,
                "{panel} offers something behind Y that no finger can reach: {}",
                missing.join(", ")
            ),
            Awry::MarkWithoutOffer(panel) => {
                write!(to, "{panel} draws a mark for an offer it does not make")
            }
            Awry::TooManyMarks(panel, marks) => write!(
                to,
                "{panel} draws {marks} marks for Y where a card with one subject wants one"
            ),
            Awry::OutOfReach(panel, room, off) => write!(
                to,
                "{panel} draws these where a hand cannot land, in a room of {room:?}: {}",
                off.join(", ")
            ),
            Awry::NoWayOut(panel, tab) => {
                write!(to, "{panel} draws no way out, on the {tab} tab")
            }
        }
    }
}

impl std::error::Error for Awry {}

impl From<console_compositor::Unanswered> for Awry {
    fn from(fault: console_compositor::Unanswered) -> Self {
        Awry::Asking(fault)
    }
}

impl From<console_input_gamepad::Unpressed> for Awry {
    fn from(fault: console_input_gamepad::Unpressed) -> Self {
        Awry::Pressing(fault)
    }
}

impl From<console_device::naming::Unnamed> for Awry {
    fn from(fault: console_device::naming::Unnamed) -> Self {
        Awry::Unnamed(fault)
    }
}

impl From<console_screen::Undeclared> for Awry {
    fn from(fault: console_screen::Undeclared) -> Self {
        Awry::Undeclared(fault)
    }
}

impl From<console_panel::telling::Untold> for Awry {
    fn from(fault: console_panel::telling::Untold) -> Self {
        Awry::Untold(fault)
    }
}

pub fn beside(program: &str) -> Result<std::path::PathBuf, console_core_never::Never> {
    let running = match std::env::current_exe() {
        Ok(running) => running,
        Err(_nothing_says_where_this_is) => return Ok(std::path::PathBuf::from(program)),
    };

    let beside_it = running.parent().map(std::path::Path::to_path_buf);
    let above_that = running.parent().and_then(std::path::Path::parent).map(std::path::Path::to_path_buf);

    let built = [beside_it, above_that]
        .into_iter()
        .flatten()
        .map(|at| at.join(program))
        .find(|at| at.is_file());

    Ok(match built {
        Some(built) => built,
        None => std::path::PathBuf::from(program),
    })
}

pub fn screen() -> Result<console_screen::Screen, Awry> {
    let Ok(root) = root();
    let written =
        std::fs::read_to_string(root.join(console_screen::CONFIG)).map_err(Awry::Machine)?;

    console_screen::Screen::read(&written).map_err(Awry::Undeclared)
}

pub fn root() -> Result<std::path::PathBuf, console_core_never::Never> {
    let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    Ok(match from.canonicalize() {
        Ok(tidied) => tidied,
        Err(_sandboxed) => from,
    })
}

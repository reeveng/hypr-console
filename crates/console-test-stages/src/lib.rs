//! Somewhere a check can be run, and what can be seen from there.
//!
//! A check says what someone did and what should have happened. Where it is
//! run decides how the doing is done and how much of the happening can be seen
//! at all.
//!
//! ```text
//! here      emulated devices, the daemon in this process, no machine
//!           involved. What can be seen is what the daemon decided to run.
//!
//! desktop   the device's own desktop, nested on this machine, and looked at.
//!           What this can answer that nothing else can is what color the
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
//!           color any place on the screen is.
//! ```
//!
//! The same check runs in more than one of them. It cannot assert the same
//! things in each, so it says what it needs to be able to see by which stage it
//! is written for, and a stage nothing is written for skips it and says so
//! rather than passing quietly.
//!
//! The last of them is someone's machine and is lent rather than given, so a
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
pub enum Error {
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
    Query(console_compositor::HyprctlError),
    Pressing(console_input_gamepad::GamepadError),
    Layers(serde_json::Error),
    Read(PathBuf, std::io::Error),
    NotAPicture(PathBuf, cairo::IoError),
    NothingToRead(cairo::BorrowError),
    OffTheEdge(Point<i64>, Size<u32>),
    Hostless,
    Unnamed(console_device_name::Unnamed),
    DeviceWroteNoPicture,
    DevicePictureGone,
    Undeclared(console_screen::Undeclared),
    NotInside(Point<i32>, String),
    UnnamedKey,
    NotBuilt(PathBuf),
    Stale(PathBuf),
    SaidNothingDrawn(String, std::io::Error, String),
    NotNested(String, String),
    RenderError(console_panel::description::RenderError),
    DrewNothing(String),
    OfferUnanswered(String, Vec<String>),
    MarkWithoutOffer(String),
    TooManyMarks(String, u32),
    OutOfReach(String, (i32, i32), Vec<String>),
    NoWayOut(String, String),
    Hidden(String, Vec<String>),
}

impl fmt::Display for Error {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Machine(fault) => write!(to, "{fault}"),
            Error::Unwritten(fault) => write!(to, "{fault}"),
            Error::NoPointer(at) => write!(
                to,
                "{} is not there, so nothing would be pressed: cargo build -p console-input-pointer",
                at.display()
            ),
            Error::PointerNotBeside => {
                write!(to, "console-point is not beside console-desktop")
            }
            Error::AlreadyTaken(what) => {
                write!(to, "the picture has already been taken; {what} before looking")
            }
            Error::NoBus(at) => write!(
                to,
                "a bus of this check's own never listened on {}",
                at.display()
            ),
            Error::TookNoPicture(last) => {
                write!(to, "the nested desktop took no picture: {last}")
            }
            Error::NestedPictureGone => {
                write!(to, "the nested desktop took a picture and then had none")
            }
            Error::NoScreenSaid(at, fault) => write!(
                to,
                "{}: the nested desktop said nothing about its screen: {fault}",
                at.display()
            ),
            Error::NoScreenAtAll(at) => write!(
                to,
                "{}: the nested desktop had no screen at all",
                at.display()
            ),
            Error::NoSize(at, named) => write!(
                to,
                "{} says nothing about how big {named} is",
                at.display()
            ),
            Error::NoWindowsSaid(at, fault) => write!(
                to,
                "{}: the nested desktop said no windows: {fault}",
                at.display()
            ),
            Error::Query(fault) => write!(to, "{fault}"),
            Error::Pressing(fault) => write!(to, "{fault}"),
            Error::Layers(fault) => write!(to, "layers: {fault}"),
            Error::Read(at, fault) => write!(to, "{}: {fault}", at.display()),
            Error::NotAPicture(at, fault) => {
                write!(to, "{} is not a picture: {fault}", at.display())
            }
            Error::NothingToRead(fault) => write!(to, "nothing to read: {fault}"),
            Error::OffTheEdge(spot, room) => write!(
                to,
                "{},{} is off the edge of a {}x{} picture",
                spot.x, spot.y, room.width, room.height
            ),
            Error::Hostless => write!(
                to,
                "CONSOLE_HOST is not set, so there is no device to talk to. \
                  Set it to the device, as in CONSOLE_HOST=root@handheld."
            ),
            Error::Unnamed(fault) => write!(to, "{fault}"),
            Error::DeviceWroteNoPicture => {
                write!(to, "the device never wrote a picture to /tmp")
            }
            Error::DevicePictureGone => {
                write!(to, "the device took a picture and then had none")
            }
            Error::Undeclared(fault) => write!(to, "{fault}"),
            Error::NotInside(spot, panel) => write!(
                to,
                "({}, {}) is not a place inside {panel}",
                spot.x, spot.y
            ),
            Error::UnnamedKey => write!(to, "a key with no name"),
            Error::NotBuilt(at) => write!(
                to,
                "{} is not in target/debug, so there would be nothing to open: \
                 cargo build --workspace",
                at.display()
            ),
            Error::Stale(at) => write!(
                to,
                "{} was built before console-panel was last edited, so this would hold \
                 today's rules against yesterday's panel: cargo build --workspace",
                at.display()
            ),
            Error::SaidNothingDrawn(program, fault, why) => write!(
                to,
                "{program} said nothing about what it drew ({fault}):\n{why}"
            ),
            Error::NotNested(program, why) => write!(
                to,
                "the desktop {program} was opened in did not come up as it was asked to:\n{why}"
            ),
            Error::RenderError(fault) => write!(to, "{fault}"),
            Error::DrewNothing(program) => write!(to, "{program} drew nothing at all"),
            Error::OfferUnanswered(panel, missing) => write!(
                to,
                "{panel} offers something behind Y that no finger can reach: {}",
                missing.join(", ")
            ),
            Error::MarkWithoutOffer(panel) => {
                write!(to, "{panel} draws a mark for an offer it does not make")
            }
            Error::TooManyMarks(panel, marks) => write!(
                to,
                "{panel} draws {marks} marks for Y where a card with one subject wants one"
            ),
            Error::OutOfReach(panel, room, off) => write!(
                to,
                "{panel} draws these where a hand cannot land, in a room of {room:?}: {}",
                off.join(", ")
            ),
            Error::NoWayOut(panel, tab) => {
                write!(to, "{panel} draws no way out, on the {tab} tab")
            }
            Error::Hidden(panel, missing) => {
                write!(to, "{panel} carries what it never drew: {}", missing.join("; "))
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<console_compositor::HyprctlError> for Error {
    fn from(fault: console_compositor::HyprctlError) -> Self {
        Error::Query(fault)
    }
}

impl From<console_input_gamepad::GamepadError> for Error {
    fn from(fault: console_input_gamepad::GamepadError) -> Self {
        Error::Pressing(fault)
    }
}

impl From<console_device_name::Unnamed> for Error {
    fn from(fault: console_device_name::Unnamed) -> Self {
        Error::Unnamed(fault)
    }
}

impl From<console_screen::Undeclared> for Error {
    fn from(fault: console_screen::Undeclared) -> Self {
        Error::Undeclared(fault)
    }
}

impl From<console_panel::description::RenderError> for Error {
    fn from(fault: console_panel::description::RenderError) -> Self {
        Error::RenderError(fault)
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

pub fn screen() -> Result<console_screen::Screen, Error> {
    let Ok(root) = root();
    let written =
        std::fs::read_to_string(root.join(console_screen::CONFIGURATION)).map_err(Error::Machine)?;

    console_screen::Screen::read(&written).map_err(Error::Undeclared)
}

pub fn root() -> Result<std::path::PathBuf, console_core_never::Never> {
    let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    Ok(match from.canonicalize() {
        Ok(tidied) => tidied,
        Err(_sandboxed) => from,
    })
}

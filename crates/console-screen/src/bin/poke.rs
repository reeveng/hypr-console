//! Put a finger down on the screen, at a place on the picture.
//!
//! For the checks. Everything else this desktop can be asked to do can be
//! asked with a button -- InputPlumber publishes the pad and a check presses it
//! over D-Bus -- and a touch is the one thing nothing could send, so it was the
//! one thing no check ever asked. That is not a small gap: a surface can be on
//! the screen, at the right size, on the right layer, drawing the right thing,
//! and answer no finger at all, and every question that is not a finger
//! answers yes.
//!
//! Which is what happened to the bar. The home screen asked the compositor for
//! the keyboard exclusively, an exclusive layer is handed every touch on the
//! screen wherever it lands, and so every tap on the bar went to the home
//! screen. Nothing was broken about the bar and nothing about the bar could be
//! asked to find that out.
//!
//! It makes a touchscreen of its own rather than driving the real one, because
//! the real one is glass and there is nobody holding the machine. The ranges
//! are the panel's own, which is what the real digitizer reports in, so the
//! compositor turns what this says exactly as it turns what that says --
//! `console_screen::Screen::poked` is the arithmetic, and it is the same
//! arithmetic either way round.
//!
//!     console-poke ACROSS DOWN
//!
//! Where ACROSS and DOWN are a place on the picture, in the size the desktop
//! is laid out in -- the numbers `hyprctl layers` answers in, and the numbers
//! anything drawing a surface thinks in.

use std::io::Write;
use std::os::fd::AsRawFd;

use console_number::fitted;
use console_screen::declared;

/// How long the finger is down.
///
/// Long enough that a toolkit sees a press and a release rather than one
/// frame with both in it, and far short of anything that reads as a hold.
const DOWN_FOR: std::time::Duration = std::time::Duration::from_millis(120);

/// How long the compositor is given to notice a device that has just appeared.
///
/// A touchscreen created and used in the same instant is a touchscreen sending
/// into nothing: libinput has to see it, add it, and apply the configuration
/// the compositor has for touch devices before anything it says lands anywhere.
const NOTICED: std::time::Duration = std::time::Duration::from_millis(1200);

fn main() {
    if let Err(fault) = poke() {
        eprintln!("console-poke: {fault}");
        std::process::exit(1);
    }
}

fn poke() -> Result<(), String> {
    let said: Vec<String> = std::env::args().skip(1).collect();

    let [across, down] = said.as_slice() else {
        return Err("say where: console-poke ACROSS DOWN".to_string());
    };

    let at = (number(across)?, number(down)?);
    let screen = declared()?;
    let (wide, tall) = screen.logical();

    if at.0 > wide || at.1 > tall {
        return Err(format!("({}, {}) is not on a {wide}x{tall} screen", at.0, at.1));
    }

    let (x, y) = screen.poked(at);
    let finger = Finger::new(screen.mode)?;

    std::thread::sleep(NOTICED);
    finger.at(x, y)?;

    Ok(())
}

fn number(said: &str) -> Result<u32, String> {
    said.parse().map_err(|_| format!("{said} is not a place on the screen"))
}

/// A touchscreen of our own, for as long as this program runs.
struct Finger {
    to: std::fs::File,
}

impl Finger {
    fn new(mode: (u32, u32)) -> Result<Finger, String> {
        let to = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .map_err(|fault| format!("/dev/uinput: {fault}"))?;

        // SAFETY: every one of these is an ioctl on a file this function just
        // opened, with arguments the kernel's uinput interface defines. The
        // struct handed to the last of them lives until the call returns.
        unsafe {
            let fd = to.as_raw_fd();

            set(fd, UI_SET_EVBIT, EV_KEY)?;
            set(fd, UI_SET_EVBIT, EV_ABS)?;
            set(fd, UI_SET_EVBIT, EV_SYN)?;
            set(fd, UI_SET_KEYBIT, BTN_TOUCH)?;
            set(fd, UI_SET_PROPBIT, INPUT_PROP_DIRECT)?;

            for (code, most) in [
                (ABS_X, mode.0),
                (ABS_Y, mode.1),
                (ABS_MT_POSITION_X, mode.0),
                (ABS_MT_POSITION_Y, mode.1),
                (ABS_MT_SLOT, 9),
                (ABS_MT_TRACKING_ID, 65535),
            ] {
                absolute(fd, code, most)?;
            }

            made(fd)?;
        }

        Ok(Finger { to })
    }

    /// Down there, and up again.
    fn at(&self, x: u32, y: u32) -> Result<(), String> {
        let down: i32 = 1;

        self.say(&[
            (EV_ABS, ABS_MT_SLOT, 0),
            (EV_ABS, ABS_MT_TRACKING_ID, down),
            (EV_ABS, ABS_MT_POSITION_X, fitted(x)),
            (EV_ABS, ABS_MT_POSITION_Y, fitted(y)),
            (EV_ABS, ABS_X, fitted(x)),
            (EV_ABS, ABS_Y, fitted(y)),
            (EV_KEY, BTN_TOUCH, 1),
        ])?;

        std::thread::sleep(DOWN_FOR);

        self.say(&[(EV_ABS, ABS_MT_TRACKING_ID, -1), (EV_KEY, BTN_TOUCH, 0)])?;

        // The device goes when this returns, and a device that goes in the same
        // instant its last event was written is a release the compositor may
        // never get around to reading.
        std::thread::sleep(DOWN_FOR);

        Ok(())
    }

    /// One report: these, and then the sync that says that is all of them.
    fn say(&self, events: &[(u16, u16, i32)]) -> Result<(), String> {
        let mut written = Vec::new();

        for (kind, code, value) in events.iter().chain(&[(EV_SYN, SYN_REPORT, 0)]) {
            written.extend_from_slice(&packed(*kind, *code, *value));
        }

        (&self.to).write_all(&written).map_err(|fault| format!("writing the touch: {fault}"))
    }
}

/// One `input_event`, as the kernel wants it on a 64-bit machine.
///
/// Written by hand rather than through a crate because this is the whole of
/// what is written: a timestamp the kernel fills in for us, a kind, a code and
/// a value.
fn packed(kind: u16, code: u16, value: i32) -> [u8; 24] {
    let mut out = [0u8; 24];
    out[16..18].copy_from_slice(&kind.to_ne_bytes());
    out[18..20].copy_from_slice(&code.to_ne_bytes());
    out[20..24].copy_from_slice(&value.to_ne_bytes());

    out
}

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;
const SYN_REPORT: u16 = 0;
const BTN_TOUCH: u16 = 0x14a;
const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const ABS_MT_SLOT: u16 = 0x2f;
const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;
const ABS_MT_TRACKING_ID: u16 = 0x39;
const INPUT_PROP_DIRECT: u16 = 0x01;

/// `_IOW('U', n, T)`, worked out rather than written down.
///
/// The size of the struct is part of the request number, so a request written
/// out as a hexadecimal constant is a constant that is wrong the moment the
/// struct it names is a different size -- and the kernel says so with
/// `Invalid argument` and nothing about which of the two is at fault, which is
/// exactly as helpful as it sounds.
const fn iow(number: libc::c_ulong, size: usize) -> libc::c_ulong {
    (1 << 30) | (wide(size) << 16) | (UINPUT << 8) | number
}

/// The letter uinput numbers its requests under: `U`.
///
/// Written as the number it is because the number is what goes into a request
/// worked out at compile time, and `b'U'` cannot be widened there --
/// `From` is not something a `const fn` may call. `the_letter_is_the_one_uinput_uses`
/// is what keeps this one and that one the same.
const UINPUT: libc::c_ulong = 0x55;

/// A size, widened to what a request number is made of.
///
/// The long way round, because the short way is an `as` cast and an `as` cast
/// is allowed to be quietly wrong. Byte for byte instead, so a machine whose
/// `usize` is not the width of a `c_ulong` stops compiling here rather than
/// sending a request with the size cut off the front of it.
const fn wide(size: usize) -> libc::c_ulong {
    libc::c_ulong::from_ne_bytes(size.to_ne_bytes())
}

const UI_SET_EVBIT: libc::c_ulong = iow(100, size_of::<libc::c_int>());
const UI_SET_KEYBIT: libc::c_ulong = iow(101, size_of::<libc::c_int>());
const UI_SET_PROPBIT: libc::c_ulong = iow(110, size_of::<libc::c_int>());
const UI_DEV_SETUP: libc::c_ulong = iow(3, size_of::<Setup>());
const UI_ABS_SETUP: libc::c_ulong = iow(4, size_of::<AbsSetup>());
const UI_DEV_CREATE: libc::c_ulong = 0x0000_5501;

/// SAFETY: `fd` is an open `/dev/uinput`, and every request here takes an
/// integer by value.
unsafe fn set(fd: i32, request: libc::c_ulong, bit: u16) -> Result<(), String> {
    // SAFETY: `fd` is an open `/dev/uinput`, and every request that reaches
    // here takes its argument as an integer by value rather than a pointer.
    match unsafe { libc::ioctl(fd, request, libc::c_int::from(bit)) } {
        -1 => Err(format!("uinput would not take {bit:#x}: {}", last())),
        _ => Ok(()),
    }
}

#[repr(C)]
struct AbsInfo {
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

#[repr(C)]
struct AbsSetup {
    code: u16,
    padding: u16,
    absinfo: AbsInfo,
}

#[repr(C)]
struct Id {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
struct Setup {
    id: Id,
    name: [u8; 80],
    effects: u32,
}

/// SAFETY: `fd` is an open `/dev/uinput` and the struct outlives the call.
unsafe fn absolute(fd: i32, code: u16, most: u32) -> Result<(), String> {
    let setup = AbsSetup {
        code,
        padding: 0,
        absinfo: AbsInfo {
            value: 0,
            minimum: 0,
            maximum: fitted(most),
            fuzz: 0,
            flat: 0,
            resolution: 0,
        },
    };

    // SAFETY: `fd` is an open `/dev/uinput`, and `setup` is a live
    // `uinput_abs_setup` that outlives the call the pointer is handed to.
    match unsafe { libc::ioctl(fd, UI_ABS_SETUP, &raw const setup) } {
        -1 => Err(format!("uinput would not take the range of {code:#x}: {}", last())),
        _ => Ok(()),
    }
}

/// SAFETY: `fd` is an open `/dev/uinput` and the struct outlives the call.
unsafe fn made(fd: i32) -> Result<(), String> {
    let mut name = [0u8; 80];
    let called = b"console-poke";
    name[..called.len()].copy_from_slice(called);

    let setup = Setup {
        id: Id { bustype: 0x03, vendor: 0x1, product: 0x1, version: 1 },
        name,
        effects: 0,
    };

    // SAFETY: `fd` is an open `/dev/uinput`, and `setup` is a live
    // `uinput_setup` that outlives the call the pointer is handed to.
    if unsafe { libc::ioctl(fd, UI_DEV_SETUP, &raw const setup) } == -1 {
        return Err(format!("uinput would not be set up: {}", last()));
    }

    // SAFETY: `fd` is an open `/dev/uinput`, and this request takes nothing
    // beyond it.
    match unsafe { libc::ioctl(fd, UI_DEV_CREATE) } {
        -1 => Err(format!("uinput would not make the device: {}", last())),
        _ => Ok(()),
    }
}

fn last() -> String {
    std::io::Error::last_os_error().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The letter is written out as a number because a `const fn` cannot widen
    /// a byte; this is the place that can, and so this is where the two are
    /// held together.
    #[test]
    fn the_letter_is_the_one_uinput_uses() {
        assert_eq!(UINPUT, libc::c_ulong::from(b'U'));
    }

    /// What the kernel actually wanted, written down once.
    ///
    /// The first of these was guessed at and carried a struct four bytes
    /// short of `uinput_setup`, which uinput answers with `Invalid argument`
    /// and nothing about which of the two it disagreed with. Deriving the
    /// number from the struct is the fix; this is what says the derivation
    /// still lands where the kernel is listening.
    #[test]
    fn a_request_carries_the_size_of_what_it_takes() {
        assert_eq!(UI_DEV_SETUP, 0x405c_5503);
        assert_eq!(size_of::<Setup>(), 92);
    }
}

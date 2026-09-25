//! The display itself, with no compositor in front of it.
//!
//! Before anybody has logged in there is no Wayland to ask for a surface, so
//! the greeter does what a compositor does first: finds a connector with a
//! screen on it, takes the mode the screen prefers, asks the kernel for a
//! plain buffer of that size, maps it, and points the screen at it. That is
//! the legacy mode-setting path -- one buffer, set once, drawn into in place --
//! and it is the whole of what a picture that changes when a button is
//! pressed needs. Atomic mode setting, page flips and planes are for a screen
//! that moves every frame, and this one does not.
//!
//! The requests and their structs are the kernel's, from `drm.h` and
//! `drm_mode.h`, and the numbers are what those headers expand to on x86-64.
//! The session this runs in is the greeter's own, which logind has made the
//! active one, so the card opens for it and the first to open it holds it.

use std::ffi::{c_int, c_ulong, c_void};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const CARDS: &str = "/dev/dri";

const CARD: &str = "card";

const GET_RESOURCES: c_ulong = 0xc040_64a0;
const GET_CONNECTOR: c_ulong = 0xc050_64a7;
const GET_ENCODER: c_ulong = 0xc014_64a6;
const SET_CONTROLLER: c_ulong = 0xc068_64a2;
const CREATE_DUMB: c_ulong = 0xc020_64b2;
const MAP_DUMB: c_ulong = 0xc010_64b3;
const ADD_FRAMEBUFFER: c_ulong = 0xc01c_64ae;
const SET_MASTER: c_ulong = 0x641e;

const CONNECTED: u32 = 1;
const PREFERRED: u32 = 8;
const BITS: u32 = 32;
const DEPTH: u32 = 24;

const READ_WRITE: c_int = 3;
const SHARED: c_int = 1;

unsafe extern "C" {
    fn ioctl(descriptor: c_int, request: c_ulong, ...) -> c_int;

    #[cfg_attr(dylint_lib = "explicit051_no_machine_width", allow(explicit051_no_machine_width, reason = "the mapping's length is C's `size_t`, which is the machine's width by the C ABI and not by choice here"))]
    fn mmap(at: *mut c_void, long: usize, protection: c_int, flags: c_int, descriptor: c_int, offset: i64) -> *mut c_void;

    #[cfg_attr(dylint_lib = "explicit051_no_machine_width", allow(explicit051_no_machine_width, reason = "the mapping's length is C's `size_t`, which is the machine's width by the C ABI and not by choice here"))]
    fn munmap(at: *mut c_void, long: usize) -> c_int;
}

#[repr(C)]
#[derive(Default)]
struct Resources {
    framebuffers: u64,
    controllers: u64,
    connectors: u64,
    encoders: u64,
    count_framebuffers: u32,
    count_controllers: u32,
    count_connectors: u32,
    count_encoders: u32,
    least_wide: u32,
    most_wide: u32,
    least_tall: u32,
    most_tall: u32,
}

#[repr(C)]
#[derive(Default)]
struct Connector {
    encoders: u64,
    modes: u64,
    properties: u64,
    property_values: u64,
    count_modes: u32,
    count_properties: u32,
    count_encoders: u32,
    encoder: u32,
    connector: u32,
    kind: u32,
    kind_number: u32,
    connection: u32,
    wide_millimetres: u32,
    tall_millimetres: u32,
    subpixel: u32,
    pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Mode {
    clock: u32,
    width: u16,
    sync_start_across: u16,
    sync_end_across: u16,
    total_across: u16,
    skew: u16,
    height: u16,
    sync_start_down: u16,
    sync_end_down: u16,
    total_down: u16,
    scan: u16,
    refresh: u32,
    flags: u32,
    kind: u32,
    name: [u8; 32],
}

#[repr(C)]
#[derive(Default)]
struct Encoder {
    encoder: u32,
    kind: u32,
    controller: u32,
    possible_controllers: u32,
    possible_clones: u32,
}

#[repr(C)]
#[derive(Default)]
struct Controller {
    connectors: u64,
    count_connectors: u32,
    controller: u32,
    framebuffer: u32,
    x: u32,
    y: u32,
    gamma_size: u32,
    mode_valid: u32,
    mode: Mode,
}

#[repr(C)]
#[derive(Default)]
struct Dumb {
    height: u32,
    width: u32,
    bits: u32,
    flags: u32,
    handle: u32,
    pitch: u32,
    long: u64,
}

#[repr(C)]
#[derive(Default)]
struct Mapped {
    handle: u32,
    pad: u32,
    offset: u64,
}

#[repr(C)]
#[derive(Default)]
struct Framebuffer {
    framebuffer: u32,
    width: u32,
    height: u32,
    pitch: u32,
    bits: u32,
    depth: u32,
    handle: u32,
}

#[derive(Debug)]
pub enum Unshown {
    NoCard,
    NothingConnected,
    NoCrtc,
    Query(&'static str, io::Error),
}

impl std::fmt::Display for Unshown {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unshown::NoCard => write!(to, "there is no display card under {CARDS}"),
            Unshown::NothingConnected => write!(to, "no display card has a screen connected to it"),
            Unshown::NoCrtc => write!(to, "the connected screen has nothing that can drive it"),
            Unshown::Query(what, why) => write!(to, "the display would not {what}: {why}"),
        }
    }
}

impl std::error::Error for Unshown {}

pub struct Display {
    card: File,
    size: Size<u32>,
    pitch: u32,
    at: *mut c_void,
    long: u64,
}

fn asked<T>(card: &File, request: c_ulong, value: &mut T, what: &'static str) -> Result<(), Unshown> {
    // SAFETY: every request here is paired with the struct the kernel's
    // header declares for it, and `value` is that struct, alive and writable
    // for the length of the call.
    let code = unsafe { ioctl(card.as_raw_fd(), request, std::ptr::from_mut(value)) };

    match code {
        -1 => Err(Unshown::Query(what, io::Error::last_os_error())),
        _ => Ok(()),
    }
}

fn address<T>(list: &mut [T]) -> Result<u64, Never> {
    fitted::<_, u64>(list.as_mut_ptr().addr())
}

impl Display {
    pub fn opened() -> Result<Display, Unshown> {
        let entries = match fs::read_dir(CARDS) {
            Ok(entries) => entries,
            Err(why) => return Err(Unshown::Query("be listed", why)),
        };
        let mut cards: Vec<_> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.file_name().is_some_and(|name| name.to_string_lossy().starts_with(CARD)))
            .collect();
        let mut last = Unshown::NoCard;

        cards.sort();

        for path in cards {
            #[cfg_attr(
                dylint_lib = "explicit040_no_torn_write",
                allow(
                    explicit040_no_torn_write,
                    reason = "a display card is a device, not a file: what is written to it is requests, and there is nothing beside it to commit"
                )
            )]
            let opened = OpenOptions::new().read(true).write(true).open(&path);

            match opened {
                Ok(card) => match Display::on(card) {
                    Ok(display) => return Ok(display),
                    Err(why) => last = why,
                },
                Err(why) => last = Unshown::Query("open", why),
            }
        }

        Err(last)
    }

    fn on(card: File) -> Result<Display, Unshown> {
        let mut nothing = 0_u32;
        let _ = asked(&card, SET_MASTER, &mut nothing, "be held");
        let mut resources = Resources::default();

        asked(&card, GET_RESOURCES, &mut resources, "list its connectors")?;

        let Ok(many_connectors) = index(resources.count_connectors);
        let Ok(many_controllers) = index(resources.count_controllers);
        let mut connectors = vec![0_u32; many_connectors];
        let mut controllers = vec![0_u32; many_controllers];
        let Ok(connectors_at) = address(&mut connectors);
        let Ok(controllers_at) = address(&mut controllers);
        let mut listed = Resources {
            connectors: connectors_at,
            controllers: controllers_at,
            count_connectors: resources.count_connectors,
            count_controllers: resources.count_controllers,
            ..Resources::default()
        };

        asked(&card, GET_RESOURCES, &mut listed, "list its connectors")?;

        let Ok(found) = connected(&card, &connectors);
        let (connector, mode, encoder) = match found {
            Some(found) => found,
            None => return Err(Unshown::NothingConnected),
        };
        let mut asked_encoder = Encoder { encoder, ..Encoder::default() };
        let controller = match asked(&card, GET_ENCODER, &mut asked_encoder, "say which encoder drives the screen") {
            Ok(()) => match asked_encoder.controller {
                0 => {
                    let Ok(first) = first_possible(&controllers, asked_encoder.possible_controllers);

                    first
                }
                controller => Some(controller),
            },
            Err(_) => controllers.first().copied(),
        };
        let controller = match controller {
            Some(controller) => controller,
            None => return Err(Unshown::NoCrtc),
        };
        let size = Size { width: u32::from(mode.width), height: u32::from(mode.height) };
        let mut dumb = Dumb { height: size.height, width: size.width, bits: BITS, ..Dumb::default() };

        asked(&card, CREATE_DUMB, &mut dumb, "make a buffer")?;

        let mut framebuffer = Framebuffer {
            width: size.width,
            height: size.height,
            pitch: dumb.pitch,
            bits: BITS,
            depth: DEPTH,
            handle: dumb.handle,
            ..Framebuffer::default()
        };

        asked(&card, ADD_FRAMEBUFFER, &mut framebuffer, "take the buffer as a framebuffer")?;

        let mut mapped = Mapped { handle: dumb.handle, ..Mapped::default() };

        asked(&card, MAP_DUMB, &mut mapped, "map the buffer")?;

        let Ok(long) = index(dumb.long);
        let Ok(offset) = fitted::<u64, i64>(mapped.offset);

        // SAFETY: the offset the kernel just gave for this buffer, mapped for
        // exactly its length, and unmapped once by `Drop`.
        let at = unsafe { mmap(std::ptr::null_mut(), long, READ_WRITE, SHARED, card.as_raw_fd(), offset) };

        match at.addr().checked_add(1) {
            None => return Err(Unshown::Query("map the buffer into memory", io::Error::last_os_error())),
            Some(_) => {}
        }

        let mut connectors_set = [connector];
        let Ok(connectors_set_at) = address(&mut connectors_set);
        let mut set = Controller {
            connectors: connectors_set_at,
            count_connectors: 1,
            controller,
            framebuffer: framebuffer.framebuffer,
            mode_valid: 1,
            mode,
            ..Controller::default()
        };
        let display = Display { card, size, pitch: dumb.pitch, at, long: dumb.long };

        asked(&display.card, SET_CONTROLLER, &mut set, "show the buffer")?;

        Ok(display)
    }

    pub fn size(&self) -> Result<Size<u32>, Never> {
        Ok(self.size)
    }

    pub fn shown(&mut self, frame: &[u8]) -> Result<(), Never> {
        let Ok(long) = index(self.long);

        // SAFETY: the mapping `on` made, `long` bytes, alive until `Drop`.
        let target = unsafe { std::slice::from_raw_parts_mut(self.at.cast::<u8>(), long) };
        let Ok(row) = index(self.size.width.saturating_mul(4));
        let Ok(pitch) = index(self.pitch);

        for (from, to) in frame.chunks_exact(row.max(1)).zip(target.chunks_exact_mut(pitch.max(1))) {
            match to.get_mut(..row) {
                Some(to) => to.copy_from_slice(from),
                None => {}
            }
        }

        Ok(())
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        let Ok(long) = index(self.long);

        // SAFETY: the mapping `on` made, unmapped exactly once.
        let _ = unsafe { munmap(self.at, long) };
    }
}

fn connected(card: &File, connectors: &[u32]) -> Result<Option<(u32, Mode, u32)>, Never> {
    for connector in connectors {
        let Ok(found) = described(card, *connector);

        match found {
            Some(found) => return Ok(Some(found)),
            None => {}
        }
    }

    Ok(None)
}

fn preferred(modes: &[Mode]) -> Result<Option<Mode>, Never> {
    Ok(modes.iter().find(|mode| mode.kind & PREFERRED == PREFERRED).or(modes.first()).copied())
}

fn described(card: &File, connector: u32) -> Result<Option<(u32, Mode, u32)>, Never> {
    let mut asked_once = Connector { connector, ..Connector::default() };

    match asked(card, GET_CONNECTOR, &mut asked_once, "describe a connector") {
        Ok(()) => {}
        Err(_) => return Ok(None),
    }

    match (asked_once.connection, asked_once.count_modes) {
        (CONNECTED, 1..) => {}
        _ => return Ok(None),
    }

    let Ok(many_modes) = index(asked_once.count_modes);
    let Ok(many_encoders) = index(asked_once.count_encoders);
    let mut modes = vec![Mode::default(); many_modes];
    let mut encoders = vec![0_u32; many_encoders];
    let Ok(modes_at) = address(&mut modes);
    let Ok(encoders_at) = address(&mut encoders);
    let mut described = Connector {
        connector,
        modes: modes_at,
        encoders: encoders_at,
        count_modes: asked_once.count_modes,
        count_encoders: asked_once.count_encoders,
        ..Connector::default()
    };

    match asked(card, GET_CONNECTOR, &mut described, "describe a connector") {
        Ok(()) => {}
        Err(_) => return Ok(None),
    }

    let Ok(preferred) = preferred(&modes);
    let encoder = match described.encoder {
        0 => encoders.first().copied(),
        encoder => Some(encoder),
    };

    Ok(match (preferred, encoder) {
        (Some(mode), Some(encoder)) => Some((connector, mode, encoder)),
        (Some(mode), None) => Some((connector, mode, 0)),
        (None, _) => None,
    })
}

fn first_possible(controllers: &[u32], possible: u32) -> Result<Option<u32>, Never> {
    Ok(controllers
        .iter()
        .enumerate()
        .find(|(at, _)| {
            let Ok(at) = fitted::<_, u32>(*at);

            at < u32::BITS && possible.checked_shr(at).is_some_and(|shifted| shifted & 1 == 1)
        })
        .map(|(_, controller)| *controller))
}

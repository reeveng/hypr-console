//! A picture of every open window, asked of the compositor.
//!
//! A card that says "foot" twice says nothing about which foot is which, and
//! the answer is on the screen already: the compositor has drawn every window
//! it holds, the ones on other places included. `ext-foreign-toplevel-list`
//! names each of them and `ext-image-copy-capture` copies one into memory this
//! process shares, which is the standard pair rather than Hyprland's own export
//! protocol, so the day the compositor is another one this file does not change.
//!
//! A window is copied at the size it is drawn, which at 2.5 is fifteen
//! megabytes for a window the size of the screen, and a card shows it a
//! quarter as wide. So every row is walked once and only every so many pixels
//! kept, until the picture is no wider than it is asked to be. That is the
//! crudest way to make a picture smaller, and at a card's size nobody can tell.

use std::fmt;
use std::io;
use std::os::fd::AsFd;
use std::sync::Arc;

use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_shapes::Pixels;
use console_draw_surface::memory::Shared;

use crate::mapping::{
    hyprland_toplevel_mapping_manager_v1 as mapper, hyprland_toplevel_window_mapping_handle_v1 as mapped,
};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_buffer, wl_registry, wl_shm, wl_shm_pool};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum, event_created_child};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1 as handle, ext_foreign_toplevel_list_v1 as list,
};
use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_foreign_toplevel_image_capture_source_manager_v1 as sources, ext_image_capture_source_v1 as source,
};
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_frame_v1 as frame, ext_image_copy_capture_manager_v1 as copier,
    ext_image_copy_capture_session_v1 as session,
};

const BYTES_PER_PIXEL: u32 = 4;

const OPAQUE: u8 = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub address: String,
    pub pixels: Pixels,
}

#[derive(Debug)]
pub enum Unpictured {
    Compositor(wayland_client::ConnectError),
    Global(&'static str),
    Protocol(wayland_client::DispatchError),
    Memory(io::Error),
}

impl fmt::Display for Unpictured {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unpictured::Compositor(why) => write!(to, "no compositor to ask for pictures of windows: {why}"),
            Unpictured::Global(what) => write!(to, "the compositor offers no {what}, so windows are shown by name"),
            Unpictured::Protocol(why) => write!(to, "the compositor stopped answering while windows were pictured: {why}"),
            Unpictured::Memory(why) => write!(to, "no memory to copy a window into: {why}"),
        }
    }
}

impl std::error::Error for Unpictured {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Waiting,
    Succeeded,
    Failed,
}

struct Listed {
    handle: handle::ExtForeignToplevelHandleV1,
    address: Option<String>,
}

struct Taking {
    listed: Vec<Listed>,
    size: Option<(u32, u32)>,
    formats: Vec<wl_shm::Format>,
    session: Outcome,
    frame: Outcome,
}

struct Globals {
    shm: wl_shm::WlShm,
    sources: sources::ExtForeignToplevelImageCaptureSourceManagerV1,
    copier: copier::ExtImageCopyCaptureManagerV1,
}

const HALF: u32 = 32;

pub fn take(widest: u32) -> Result<Vec<Capture>, Unpictured> {
    let connection = Connection::connect_to_env().map_err(Unpictured::Compositor)?;
    let (globals, mut queue) =
        registry_queue_init::<Taking>(&connection).map_err(|_| Unpictured::Global("list of globals"))?;
    let hand = queue.handle();

    let shm = globals.bind::<wl_shm::WlShm, _, _>(&hand, 1..=1, ()).map_err(|_| Unpictured::Global("wl_shm"))?;
    let sources = globals
        .bind::<sources::ExtForeignToplevelImageCaptureSourceManagerV1, _, _>(&hand, 1..=1, ())
        .map_err(|_| Unpictured::Global("ext_foreign_toplevel_image_capture_source_manager_v1"))?;
    let copier = globals
        .bind::<copier::ExtImageCopyCaptureManagerV1, _, _>(&hand, 1..=1, ())
        .map_err(|_| Unpictured::Global("ext_image_copy_capture_manager_v1"))?;
    let bound = Globals { shm, sources, copier };
    let listing = globals
        .bind::<list::ExtForeignToplevelListV1, _, _>(&hand, 1..=1, ())
        .map_err(|_| Unpictured::Global("ext_foreign_toplevel_list_v1"))?;
    let addresses = globals
        .bind::<mapper::HyprlandToplevelMappingManagerV1, _, _>(&hand, 1..=1, ())
        .map_err(|_| Unpictured::Global("hyprland_toplevel_mapping_manager_v1"))?;

    let mut taking = Taking { listed: Vec::new(), size: None, formats: Vec::new(), session: Outcome::Waiting, frame: Outcome::Waiting };

    queue.roundtrip(&mut taking).map_err(Unpictured::Protocol)?;

    for listed in &taking.listed {
        let _asked = addresses.get_window_for_toplevel(&listed.handle, &hand, listed.handle.clone());
    }

    queue.roundtrip(&mut taking).map_err(Unpictured::Protocol)?;

    let wanted: Vec<(handle::ExtForeignToplevelHandleV1, String)> = taking
        .listed
        .iter()
        .filter_map(|listed| listed.address.clone().map(|address| (listed.handle.clone(), address)))
        .collect();

    let mut taken = Vec::new();

    for (window, address) in wanted {
        let pictured = picture(&mut queue, &mut taking, &bound, &window)?;

        match pictured {
            Some(mut captured) => {
                let Ok(pixels) = smaller(&mut captured, widest);

                taken.push(Capture { address, pixels });
            }
            None => {},
        }
    }

    listing.stop();
    addresses.destroy();

    Ok(taken)
}

struct Captured {
    shared: Shared,
    width: u32,
    height: u32,
    stride: u32,
}

fn picture(
    queue: &mut EventQueue<Taking>,
    taking: &mut Taking,
    bound: &Globals,
    window: &handle::ExtForeignToplevelHandleV1,
) -> Result<Option<Captured>, Unpictured> {
    let hand = queue.handle();
    let source = bound.sources.create_source(window, &hand, ());
    let session = bound.copier.create_session(&source, copier::Options::empty(), &hand, ());

    taking.size = None;
    taking.formats.clear();
    taking.session = Outcome::Waiting;

    let said = until_told(queue, taking, Awaited::Session)?;

    let format = taking
        .formats
        .iter()
        .copied()
        .find(|format| matches!(format, wl_shm::Format::Xrgb8888 | wl_shm::Format::Argb8888));

    let captured = match (said, taking.size, format) {
        (Outcome::Succeeded, Some((width, height)), Some(format)) => copied(queue, taking, bound, &session, Copying { width, height, format })?,
        (Outcome::Succeeded | Outcome::Failed | Outcome::Waiting, _, _) => None,
    };

    session.destroy();
    source.destroy();

    Ok(captured)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Copying {
    width: u32,
    height: u32,
    format: wl_shm::Format,
}

fn copied(
    queue: &mut EventQueue<Taking>,
    taking: &mut Taking,
    bound: &Globals,
    session: &session::ExtImageCopyCaptureSessionV1,
    copying: Copying,
) -> Result<Option<Captured>, Unpictured> {
    let hand = queue.handle();
    let stride = copying.width.saturating_mul(BYTES_PER_PIXEL);
    let long = u64::from(stride).saturating_mul(u64::from(copying.height));
    let shared = Shared::of(long).map_err(Unpictured::Memory)?;
    let Ok(held) = shared.descriptor();
    let Ok(many) = fitted::<u64, i32>(long);
    let Ok(wide) = fitted::<u32, i32>(copying.width);
    let Ok(tall) = fitted::<u32, i32>(copying.height);
    let Ok(across) = fitted::<u32, i32>(stride);
    let pool = bound.shm.create_pool(held.as_fd(), many, &hand, ());
    let buffer = pool.create_buffer(0, wide, tall, across, copying.format, &hand, ());
    let frame = session.create_frame(&hand, ());

    frame.attach_buffer(&buffer);
    frame.damage_buffer(0, 0, wide, tall);
    frame.capture();
    taking.frame = Outcome::Waiting;

    let said = until_told(queue, taking, Awaited::Frame)?;

    frame.destroy();
    buffer.destroy();
    pool.destroy();

    Ok(match said {
        Outcome::Succeeded => Some(Captured { shared, width: copying.width, height: copying.height, stride }),
        Outcome::Failed | Outcome::Waiting => None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Awaited {
    Session,
    Frame,
}

fn until_told(queue: &mut EventQueue<Taking>, taking: &mut Taking, awaited: Awaited) -> Result<Outcome, Unpictured> {
    let told = console_core_iteration::iterate((queue, taking), |(queue, taking)| {
        let told = match awaited {
            Awaited::Session => taking.session,
            Awaited::Frame => taking.frame,
        };

        Ok(match told {
            Outcome::Succeeded | Outcome::Failed => Step::Halt(Ok(told)),
            Outcome::Waiting => match queue.blocking_dispatch(taking) {
                Ok(_dispatched) => Step::Again((queue, taking)),
                Err(fault) => Step::Halt(Err(Unpictured::Protocol(fault))),
            },
        })
    });

    match told {
        Ok(told) => told,
        Err(_endless) => Ok(Outcome::Failed),
    }
}

fn smaller(captured: &mut Captured, widest: u32) -> Result<Pixels, Never> {
    let step = captured.width.div_ceil(widest.max(1)).max(1);
    let Ok(skip) = index(step);
    let Ok(row) = index(captured.stride);
    let width = captured.width.div_ceil(step);
    let height = captured.height.div_ceil(step);
    let Ok(across) = index(width);
    let Ok(copied) = captured.shared.pixels();

    let bytes: Vec<u8> = copied
        .chunks(row)
        .step_by(skip)
        .flat_map(|line| {
            line.as_chunks::<4>().0.iter().step_by(skip).take(across).flat_map(|[blue, green, red, _alpha]| [*red, *green, *blue, OPAQUE])
        })
        .collect();

    Ok(Pixels { width, height, stride: width.saturating_mul(BYTES_PER_PIXEL), bytes: Arc::new(bytes) })
}

macro_rules! nothing_to_hear {
    ($($object:ty),* $(,)?) => {
        $(
            impl Dispatch<$object, ()> for Taking {
                fn event(
                    _taking: &mut Self,
                    _object: &$object,
                    _event: <$object as wayland_client::Proxy>::Event,
                    _held: &(),
                    _connection: &Connection,
                    _hand: &QueueHandle<Self>,
                ) {
                }
            }
        )*
    };
}

nothing_to_hear!(
    wl_shm::WlShm,
    wl_shm_pool::WlShmPool,
    wl_buffer::WlBuffer,
    sources::ExtForeignToplevelImageCaptureSourceManagerV1,
    source::ExtImageCaptureSourceV1,
    copier::ExtImageCopyCaptureManagerV1,
    mapper::HyprlandToplevelMappingManagerV1,
);

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Taking {
    fn event(
        _taking: &mut Self,
        _registry: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _held: &GlobalListContents,
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<list::ExtForeignToplevelListV1, ()> for Taking {
    #[cfg_attr(
        dylint_lib = "explicit016_no_wildcard_arm",
        allow(
            explicit016_no_wildcard_arm,
            reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
        )
    )]
    fn event(
        taking: &mut Self,
        _list: &list::ExtForeignToplevelListV1,
        event: list::Event,
        _held: &(),
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
        match event {
            list::Event::Toplevel { toplevel } => taking.listed.push(Listed { handle: toplevel, address: None }),
            _ => {}
        }
    }

    event_created_child!(Taking, list::ExtForeignToplevelListV1, [
        list::EVT_TOPLEVEL_OPCODE => (handle::ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<handle::ExtForeignToplevelHandleV1, ()> for Taking {
    #[cfg_attr(
        dylint_lib = "explicit016_no_wildcard_arm",
        allow(
            explicit016_no_wildcard_arm,
            reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
        )
    )]
    fn event(
        taking: &mut Self,
        window: &handle::ExtForeignToplevelHandleV1,
        event: handle::Event,
        _held: &(),
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
        match event {
            handle::Event::Closed => taking.listed.retain(|listed| listed.handle != *window),
            _ => {}
        }
    }
}

impl Dispatch<mapped::HyprlandToplevelWindowMappingHandleV1, handle::ExtForeignToplevelHandleV1> for Taking {
    fn event(
        taking: &mut Self,
        mapping: &mapped::HyprlandToplevelWindowMappingHandleV1,
        event: mapped::Event,
        window: &handle::ExtForeignToplevelHandleV1,
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
        let address = match event {
            mapped::Event::WindowAddress { address_hi, address } => {
                Some(format!("0x{:x}", u64::from(address_hi).wrapping_shl(HALF) | u64::from(address)))
            }
            mapped::Event::Failed => None,
        };

        for listed in taking.listed.iter_mut().filter(|listed| listed.handle == *window) {
            listed.address.clone_from(&address);
        }

        mapping.destroy();
    }
}

impl Dispatch<session::ExtImageCopyCaptureSessionV1, ()> for Taking {
    #[cfg_attr(
        dylint_lib = "explicit016_no_wildcard_arm",
        allow(
            explicit016_no_wildcard_arm,
            reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
        )
    )]
    fn event(
        taking: &mut Self,
        _session: &session::ExtImageCopyCaptureSessionV1,
        event: session::Event,
        _held: &(),
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
        match event {
            session::Event::BufferSize { width, height } => taking.size = Some((width, height)),
            session::Event::ShmFormat { format: WEnum::Value(format) } => taking.formats.push(format),
            session::Event::Done => taking.session = Outcome::Succeeded,
            session::Event::Stopped => taking.session = Outcome::Failed,
            _ => {}
        }
    }
}

impl Dispatch<frame::ExtImageCopyCaptureFrameV1, ()> for Taking {
    #[cfg_attr(
        dylint_lib = "explicit016_no_wildcard_arm",
        allow(
            explicit016_no_wildcard_arm,
            reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
        )
    )]
    fn event(
        taking: &mut Self,
        _frame: &frame::ExtImageCopyCaptureFrameV1,
        event: frame::Event,
        _held: &(),
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
        match event {
            frame::Event::Ready => taking.frame = Outcome::Succeeded,
            frame::Event::Failed { .. } => taking.frame = Outcome::Failed,
            _ => {}
        }
    }
}

//! A picture of every open window, asked of the compositor.
//!
//! A card that says "foot" twice says nothing about which foot is which, and
//! the answer is on the screen already: the compositor has drawn every window
//! it holds, the ones on other places included. `ext-foreign-toplevel-list`
//! names each of them and `ext-image-copy-capture` copies one into memory this
//! process shares. Hyprland's mapping protocol matches the listed toplevel with
//! the window address used by the control socket.
//!
//! A window is copied at the size it is drawn, which at 2.5 is fifteen
//! megabytes for a window the size of the screen, and a card shows it a
//! quarter as wide. Only the sampled rows are read from the shared buffer,
//! and every few pixels from each are kept until the picture fits.

use std::fmt;
use std::io;

use console_core_geometry::Size;
use console_core_iteration::Step;
use console_core_number_conversion::{fitted, index};
use console_core_shapes::Pixels;
use console_wayland::buffer::ShmBuffer;
use console_wayland::memory::Shared;
use console_wayland::{client, protocols, registry};

use crate::toplevel_mapping::{
  hyprland_toplevel_mapping_manager_v1::HyprlandToplevelMappingManagerV1,
  hyprland_toplevel_window_mapping_handle_v1::{self, HyprlandToplevelWindowMappingHandleV1},
};
use client::globals::GlobalListContents;
use client::protocol::{wl_buffer, wl_registry, wl_shm, wl_shm_pool};
use client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum, event_created_child};
use protocols::ext::foreign_toplevel_list::v1::client::{
  ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
  ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1},
};
use protocols::ext::image_capture_source::v1::client::{
  ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
  ext_image_capture_source_v1::ExtImageCaptureSourceV1,
};
use protocols::ext::image_copy_capture::v1::client::{
  ext_image_copy_capture_frame_v1::{self, ExtImageCopyCaptureFrameV1},
  ext_image_copy_capture_manager_v1::{self, ExtImageCopyCaptureManagerV1},
  ext_image_copy_capture_session_v1::{self, ExtImageCopyCaptureSessionV1},
};

const OPAQUE: u8 = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
  pub address: String,
  pub pixels: Pixels,
}

#[derive(Debug)]
pub enum Unpictured {
  Compositor(client::ConnectError),
  Global(&'static str),
  Protocol(client::DispatchError),
  Memory(io::Error),
}

impl fmt::Display for Unpictured {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Unpictured::Compositor(why) => {
        write!(to, "no compositor to ask for pictures of windows: {why}")
      }
      Unpictured::Global(what) => write!(
        to,
        "the compositor offers no {what}, so windows are shown by name"
      ),
      Unpictured::Protocol(why) => write!(
        to,
        "the compositor stopped answering while windows were pictured: {why}"
      ),
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
  handle: ExtForeignToplevelHandleV1,
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
  sources: ExtForeignToplevelImageCaptureSourceManagerV1,
  copier: ExtImageCopyCaptureManagerV1,
}

const HALF: u32 = 32;

pub fn take(widest: u32) -> Result<Vec<Capture>, Unpictured> {
  let connection = console_wayland::connect().map_err(Unpictured::Compositor)?;
  let (globals, mut queue) =
    registry::queue::<Taking>(&connection).map_err(|_| Unpictured::Global("list of globals"))?;
  let hand = queue.handle();

  let shm = registry::shared_memory(&globals, &hand).map_err(|_| Unpictured::Global("wl_shm"))?;
  let sources = globals
    .bind::<ExtForeignToplevelImageCaptureSourceManagerV1, _, _>(&hand, 1..=1, ())
    .map_err(|_| Unpictured::Global("ext_foreign_toplevel_image_capture_source_manager_v1"))?;
  let copier = globals
    .bind::<ExtImageCopyCaptureManagerV1, _, _>(&hand, 1..=1, ())
    .map_err(|_| Unpictured::Global("ext_image_copy_capture_manager_v1"))?;
  let bound = Globals {
    shm,
    sources,
    copier,
  };
  let listing = globals
    .bind::<ExtForeignToplevelListV1, _, _>(&hand, 1..=1, ())
    .map_err(|_| Unpictured::Global("ext_foreign_toplevel_list_v1"))?;
  let addresses = globals
    .bind::<HyprlandToplevelMappingManagerV1, _, _>(&hand, 1..=1, ())
    .map_err(|_| Unpictured::Global("hyprland_toplevel_mapping_manager_v1"))?;

  let mut taking = Taking {
    listed: Vec::new(),
    size: None,
    formats: Vec::new(),
    session: Outcome::Waiting,
    frame: Outcome::Waiting,
  };

  queue.roundtrip(&mut taking).map_err(Unpictured::Protocol)?;

  for listed in &taking.listed {
    let _asked = addresses.get_window_for_toplevel(&listed.handle, &hand, listed.handle.clone());
  }

  queue.roundtrip(&mut taking).map_err(Unpictured::Protocol)?;

  let wanted: Vec<(ExtForeignToplevelHandleV1, String)> = taking
    .listed
    .iter()
    .filter_map(|listed| {
      listed
        .address
        .clone()
        .map(|address| (listed.handle.clone(), address))
    })
    .collect();

  let mut taken = Vec::new();

  for (window, address) in wanted {
    let pictured = picture(&mut queue, &mut taking, &bound, &window)?;

    match pictured {
      Some(captured) => {
        let pixels = smaller(&captured, widest).map_err(Unpictured::Memory)?;

        taken.push(Capture { address, pixels });
      }
      None => {}
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
  window: &ExtForeignToplevelHandleV1,
) -> Result<Option<Captured>, Unpictured> {
  let hand = queue.handle();
  let source = bound.sources.create_source(window, &hand, ());
  let session = bound.copier.create_session(
    &source,
    ext_image_copy_capture_manager_v1::Options::empty(),
    &hand,
    (),
  );

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
    (Outcome::Succeeded, Some((width, height)), Some(format_2)) => copied(
      queue,
      taking,
      bound,
      &session,
      Copying {
        width,
        height,
        format: format_2,
      },
    )?,
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
  session: &ExtImageCopyCaptureSessionV1,
  copying: Copying,
) -> Result<Option<Captured>, Unpictured> {
  let hand = queue.handle();
  let size = Size {
    width: copying.width,
    height: copying.height,
  };
  let pixels =
    ShmBuffer::new(&bound.shm, &hand, size, copying.format).map_err(Unpictured::Memory)?;
  let Ok(wide) = fitted::<u32, i32>(copying.width);
  let Ok(tall) = fitted::<u32, i32>(copying.height);
  let frame = session.create_frame(&hand, ());

  let Ok(buffer) = pixels.buffer();
  frame.attach_buffer(buffer);
  frame.damage_buffer(0, 0, wide, tall);
  frame.capture();
  taking.frame = Outcome::Waiting;

  let said = until_told(queue, taking, Awaited::Frame);

  frame.destroy();
  let Ok((shared, stride)) = pixels.into_memory();
  let said_2 = said?;

  Ok(match said_2 {
    Outcome::Succeeded => Some(Captured {
      shared,
      width: copying.width,
      height: copying.height,
      stride,
    }),
    Outcome::Failed | Outcome::Waiting => None,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Awaited {
  Session,
  Frame,
}

fn until_told(
  queue: &mut EventQueue<Taking>,
  taking: &mut Taking,
  awaited: Awaited,
) -> Result<Outcome, Unpictured> {
  let told = console_core_iteration::iterate((queue, taking), |(queue_2, taking_2)| {
    let told = match awaited {
      Awaited::Session => taking_2.session,
      Awaited::Frame => taking_2.frame,
    };

    Ok(match told {
      Outcome::Succeeded | Outcome::Failed => Step::Halt(Ok(told)),
      Outcome::Waiting => match queue_2.blocking_dispatch(taking_2) {
        Ok(_dispatched) => Step::Again((queue_2, taking_2)),
        Err(fault) => Step::Halt(Err(Unpictured::Protocol(fault))),
      },
    })
  });

  match told {
    Ok(told_2) => told_2,
    Err(_endless) => Ok(Outcome::Failed),
  }
}

fn smaller(captured: &Captured, widest: u32) -> io::Result<Pixels> {
  let step = captured.width.div_ceil(widest.max(1)).max(1);
  const MAX_POOL_BYTES: u32 = 2_147_483_647;
  let row_bytes = captured.stride.min(MAX_POOL_BYTES);

  match (row_bytes == captured.stride).then_some(()) {
    Some(()) => {}
    None => {
      return Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "thumbnail row exceeds the Wayland pool limit",
      ));
    }
  }

  let width = captured.width.div_ceil(step);
  let height = captured.height.div_ceil(step);
  let Ok(stride) = index(row_bytes);
  let mut row = Vec::new();
  row
    .try_reserve_exact(stride.min(2_147_483_647))
    .map_err(io::Error::other)?;
  row.resize(row.capacity().min(stride), 0);
  let mut bytes = Vec::new();

  for rendered_row in 0..height {
    let line = rendered_row.saturating_mul(step);
    let offset = u64::from(line).saturating_mul(u64::from(row_bytes));
    captured.shared.read_pixels_at(&mut row, offset)?;
    let (pixels, _padding) = row.as_chunks::<4>();

    for rendered_column in 0..width {
      let Ok(column) = index(rendered_column.saturating_mul(step));
      let [blue, green, red, _alpha] = pixels.get(column).ok_or(io::ErrorKind::InvalidData)?;
      bytes.extend([*red, *green, *blue, OPAQUE]);
    }
  }

  let Ok(pixels) = Pixels::packed(Size { width, height }, bytes);

  Ok(pixels)
}

#[cfg(test)]
mod tests {
  use std::fs::File;
  use std::io::Write;

  use super::*;

  #[test]
  fn sampling_keeps_odd_edges_and_converts_bgra_to_opaque_rgba() -> io::Result<()> {
    let shared = Shared::of(36)?;
    let Ok(descriptor) = shared.descriptor();
    let owned = descriptor.try_clone_to_owned()?;
    let source: Vec<_> = (0_u8..9).flat_map(|blue| [blue, 20, 30, 40]).collect();
    File::from(owned).write_all(&source)?;

    let pixels = smaller(
      &Captured {
        shared,
        width: 3,
        height: 3,
        stride: 12,
      },
      2,
    )?;
    assert_eq!((pixels.width, pixels.height, pixels.stride), (2, 2, 8));
    assert_eq!(
      pixels.bytes,
      [
        30, 20, 0, OPAQUE, 30, 20, 2, OPAQUE, 30, 20, 6, OPAQUE, 30, 20, 8, OPAQUE
      ]
    );
    Ok(())
  }
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "Wayland requires a separate Dispatch implementation for each proxy type even when it has no events this client handles"
  )
)]
macro_rules! nothing_to_hear {
    ($($object:ty),* $(,)?) => {
        $(
            impl Dispatch<$object, ()> for Taking {
                fn event(
                    _taking: &mut Self,
                    _object: &$object,
                    _event: <$object as client::Proxy>::Event,
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
  ExtForeignToplevelImageCaptureSourceManagerV1,
  ExtImageCaptureSourceV1,
  ExtImageCopyCaptureManagerV1,
  HyprlandToplevelMappingManagerV1,
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

impl Dispatch<ExtForeignToplevelListV1, ()> for Taking {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    taking: &mut Self,
    _list: &ExtForeignToplevelListV1,
    event: ext_foreign_toplevel_list_v1::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } => taking.listed.push(Listed {
        handle: toplevel,
        address: None,
      }),
      _ => {}
    }
  }

  event_created_child!(Taking, ExtForeignToplevelListV1, [
      ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
  ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for Taking {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    taking: &mut Self,
    window: &ExtForeignToplevelHandleV1,
    event: ext_foreign_toplevel_handle_v1::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      ext_foreign_toplevel_handle_v1::Event::Closed => {
        taking.listed.retain(|listed| listed.handle != *window)
      }
      _ => {}
    }
  }
}

impl Dispatch<HyprlandToplevelWindowMappingHandleV1, ExtForeignToplevelHandleV1> for Taking {
  fn event(
    taking: &mut Self,
    mapping: &HyprlandToplevelWindowMappingHandleV1,
    event: hyprland_toplevel_window_mapping_handle_v1::Event,
    window: &ExtForeignToplevelHandleV1,
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    let address = match event {
      hyprland_toplevel_window_mapping_handle_v1::Event::WindowAddress {
        address_hi,
        address,
      } => Some(format!(
        "0x{:x}",
        u64::from(address_hi).wrapping_shl(HALF) | u64::from(address)
      )),
      hyprland_toplevel_window_mapping_handle_v1::Event::Failed => None,
    };

    for listed in taking
      .listed
      .iter_mut()
      .filter(|listed| listed.handle == *window)
    {
      listed.address.clone_from(&address);
    }

    mapping.destroy();
  }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, ()> for Taking {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    taking: &mut Self,
    _session: &ExtImageCopyCaptureSessionV1,
    event: ext_image_copy_capture_session_v1::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      ext_image_copy_capture_session_v1::Event::BufferSize { width, height } => {
        taking.size = Some((width, height))
      }
      ext_image_copy_capture_session_v1::Event::ShmFormat {
        format: WEnum::Value(format),
      } => taking.formats.push(format),
      ext_image_copy_capture_session_v1::Event::Done => taking.session = Outcome::Succeeded,
      ext_image_copy_capture_session_v1::Event::Stopped => taking.session = Outcome::Failed,
      _ => {}
    }
  }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, ()> for Taking {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    taking: &mut Self,
    _frame: &ExtImageCopyCaptureFrameV1,
    event: ext_image_copy_capture_frame_v1::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      ext_image_copy_capture_frame_v1::Event::Ready => taking.frame = Outcome::Succeeded,
      ext_image_copy_capture_frame_v1::Event::Failed { .. } => taking.frame = Outcome::Failed,
      _ => {}
    }
  }
}

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
use std::fs::File;
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
const MAX_QUERY_ENTRIES: u32 = 65_536;

unsafe extern "C" {
  fn ioctl(descriptor: c_int, request: c_ulong, ...) -> c_int;

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

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the live DRM descriptor and each request's corresponding writable kernel ABI struct are borrowed for the ioctl"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit090_no_raw_descriptor",
  expect(
    explicit090_no_raw_descriptor,
    reason = "the C DRM ioctl ABI borrows the raw number only while the caller keeps the File open"
  )
)]
fn ioctl_query<T>(
  card: &File,
  request: c_ulong,
  value: &mut T,
  what: &'static str,
) -> Result<(), Unshown> {
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

#[cfg_attr(
  dylint_lib = "explicit088_no_arriving_length",
  expect(
    explicit088_no_arriving_length,
    reason = "counts above MAX_QUERY_ENTRIES are refused before converting to a native allocation length"
  )
)]
fn query_entries<T: Clone>(count: u32, default: T) -> Result<Vec<T>, Unshown> {
  match count {
    0..=MAX_QUERY_ENTRIES => {}
    _ => {
      return Err(Unshown::Query(
        "fit its resource list in memory",
        io::Error::from(rustix::io::Errno::OVERFLOW),
      ));
    }
  }

  let Ok(length) = index(count);

  Ok(vec![default; length])
}

impl Display {
  pub fn open() -> Result<Display, Unshown> {
    use rustix::fs::{Dir, FileType, Mode, OFlags, fstat, open, openat};

    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let directory = open(CARDS, flags, Mode::empty())
      .map_err(|why| Unshown::Query("open the device directory", io::Error::from(why)))?;
    let entries = Dir::read_from(&directory)
      .map_err(|why| Unshown::Query("list the devices", io::Error::from(why)))?;
    let mut cards: Vec<_> = entries
      .flatten()
      .map(|entry| entry.file_name().to_owned())
      .filter(|name| name.to_bytes().starts_with(CARD.as_bytes()))
      .collect();
    let mut last = Unshown::NoCard;

    cards.sort();

    for name in cards {
      let flags_2 = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC;
      let opened = openat(&directory, &name, flags_2, Mode::empty()).and_then(|card| {
        let checked_value = fstat(&card)?;

        let kind = FileType::from_raw_mode(checked_value.st_mode);

        match bool::then_some(kind == FileType::CharacterDevice, ()) {
          Some(()) => Ok(File::from(card)),
          None => Err(rustix::io::Errno::NODEV),
        }
      });

      match opened {
        Ok(card) => match Display::on(card) {
          Ok(display) => return Ok(display),
          Err(why) => last = why,
        },
        Err(why) => last = Unshown::Query("open", io::Error::from(why)),
      }
    }

    Err(last)
  }

  fn on(card: File) -> Result<Display, Unshown> {
    let (connector, mode, controller) = select_output(&card)?;
    let size = Size {
      width: u32::from(mode.width),
      height: u32::from(mode.height),
    };
    let mut dumb = Dumb {
      height: size.height,
      width: size.width,
      bits: BITS,
      ..Dumb::default()
    };

    ioctl_query(&card, CREATE_DUMB, &mut dumb, "make a buffer")?;

    let mut framebuffer = Framebuffer {
      width: size.width,
      height: size.height,
      pitch: dumb.pitch,
      bits: BITS,
      depth: DEPTH,
      handle: dumb.handle,
      ..Framebuffer::default()
    };

    ioctl_query(
      &card,
      ADD_FRAMEBUFFER,
      &mut framebuffer,
      "take the buffer as a framebuffer",
    )?;

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
    let at = map_buffer(&card, &dumb)?;
    let display = Display {
      card,
      size,
      pitch: dumb.pitch,
      at,
      long: dumb.long,
    };

    ioctl_query(&display.card, SET_CONTROLLER, &mut set, "show the buffer")?;

    Ok(display)
  }

  pub fn size(&self) -> Result<Size<u32>, Never> {
    Ok(self.size)
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the mapped framebuffer remains valid for the exclusive borrow and only Drop unmaps it"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit096_no_unchecked_memory",
    expect(
      explicit096_no_unchecked_memory,
      reason = "the successful DRM mapping sized by the kernel stays live for this exclusive framebuffer borrow"
    )
  )]
  pub fn mapped(&mut self) -> Result<Mapping<'_>, Never> {
    let Ok(long) = index(self.long);

    // SAFETY: the mapping `on` made, `long` bytes, alive until `Drop`, and borrowed from `self` so it cannot outlive it.
    let bytes = unsafe { std::slice::from_raw_parts_mut(self.at.cast::<u8>(), long) };

    Ok(Mapping {
      bytes,
      pitch: self.pitch,
    })
  }
}

pub struct Mapping<'a> {
  pub bytes: &'a mut [u8],
  pub pitch: u32,
}

impl Drop for Display {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "Display owns the kernel's framebuffer mapping and supplies its original address and length"
    )
  )]
  fn drop(&mut self) {
    let Ok(long) = index(self.long);

    #[cfg_attr(
      dylint_lib = "explicit060_no_reasonless_discard",
      expect(
        explicit060_no_reasonless_discard,
        reason = "the mapping's exact address and length are supplied; the destructor cannot recover from a kernel error"
      )
    )]
    // SAFETY: the mapping `on` made, unmapped exactly once.
    let _ = unsafe { rustix::mm::munmap(self.at, long) };
  }
}

fn select_output(card: &File) -> Result<(u32, Mode, u32), Unshown> {
  let mut nothing = 0_u32;
  #[cfg_attr(
    dylint_lib = "explicit060_no_reasonless_discard",
    expect(
      explicit060_no_reasonless_discard,
      reason = "some cards already hold master; the later mode-set request reports whether this descriptor can drive the display"
    )
  )]
  let _ = ioctl_query(card, SET_MASTER, &mut nothing, "be held");
  let mut resources = Resources::default();

  ioctl_query(card, GET_RESOURCES, &mut resources, "list its connectors")?;

  let mut connectors = query_entries(resources.count_connectors, 0_u32)?;
  let mut controllers = query_entries(resources.count_controllers, 0_u32)?;
  let Ok(connectors_at) = address(&mut connectors);
  let Ok(controllers_at) = address(&mut controllers);
  let mut listed = Resources {
    connectors: connectors_at,
    controllers: controllers_at,
    count_connectors: resources.count_connectors,
    count_controllers: resources.count_controllers,
    ..Resources::default()
  };

  ioctl_query(card, GET_RESOURCES, &mut listed, "list its connectors")?;

  let Ok(found) = connected(card, &connectors);
  let (connector, mode, encoder) = found.ok_or(Unshown::NothingConnected)?;
  let mut asked_encoder = Encoder {
    encoder,
    ..Encoder::default()
  };
  let controller = match ioctl_query(
    card,
    GET_ENCODER,
    &mut asked_encoder,
    "say which encoder drives the screen",
  ) {
    Ok(()) => match asked_encoder.controller {
      0 => {
        let Ok(first) = first_possible(&controllers, asked_encoder.possible_controllers);

        first
      }
      current => Some(current),
    },
    Err(_the_card_would_not_say) => controllers.first().copied(),
  };
  let selected = controller.ok_or(Unshown::NoCrtc)?;

  Ok((connector, mode, selected))
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the live DRM descriptor and successful map ioctl identify the exact buffer offset and length"
  )
)]
fn map_buffer(card: &File, dumb: &Dumb) -> Result<*mut c_void, Unshown> {
  let mut mapped = Mapped {
    handle: dumb.handle,
    ..Mapped::default()
  };

  ioctl_query(card, MAP_DUMB, &mut mapped, "map the buffer")?;

  let Ok(long) = index(dumb.long);
  // SAFETY: DRM supplied the offset and size; the returned mapping is owned by Display.
  unsafe {
    rustix::mm::mmap(
      std::ptr::null_mut(),
      long,
      rustix::mm::ProtFlags::READ | rustix::mm::ProtFlags::WRITE,
      rustix::mm::MapFlags::SHARED,
      card,
      mapped.offset,
    )
  }
  .map_err(|why| Unshown::Query("map the buffer into memory", io::Error::from(why)))
}

fn connected(card: &File, connectors: &[u32]) -> Result<Option<(u32, Mode, u32)>, Never> {
  for connector in connectors {
    let Ok(found) = describe_connector(card, *connector);

    match found {
      Some(found_2) => return Ok(Some(found_2)),
      None => {}
    }
  }

  Ok(None)
}

fn preferred(modes: &[Mode]) -> Result<Option<Mode>, Never> {
  Ok(
    modes
      .iter()
      .find(|mode| mode.kind & PREFERRED == PREFERRED)
      .or(modes.first())
      .copied(),
  )
}

fn describe_connector(card: &File, connector: u32) -> Result<Option<(u32, Mode, u32)>, Never> {
  let mut asked_once = Connector {
    connector,
    ..Connector::default()
  };

  match ioctl_query(card, GET_CONNECTOR, &mut asked_once, "describe a connector") {
    Ok(()) => {}
    Err(_the_card_would_not_say) => return Ok(None),
  }

  match (asked_once.connection, asked_once.count_modes) {
    (CONNECTED, 1..) => {}
    _ => return Ok(None),
  }

  let mut modes = match query_entries(asked_once.count_modes, Mode::default()) {
    Ok(modes) => modes,
    Err(_unusable_connector) => return Ok(None),
  };
  let mut encoders = match query_entries(asked_once.count_encoders, 0_u32) {
    Ok(encoders) => encoders,
    Err(_unusable_connector) => return Ok(None),
  };
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

  match ioctl_query(card, GET_CONNECTOR, &mut described, "describe a connector") {
    Ok(()) => {}
    Err(_the_card_would_not_say) => return Ok(None),
  }

  let Ok(preferred) = preferred(&modes);
  let encoder = match described.encoder {
    0 => encoders.first().copied(),
    encoder => Some(encoder),
  };

  Ok(match (preferred, encoder) {
    (Some(mode), Some(encoder_2)) => Some((connector, mode, encoder_2)),
    (Some(mode), None) => Some((connector, mode, 0)),
    (None, _) => None,
  })
}

fn first_possible(controllers: &[u32], possible: u32) -> Result<Option<u32>, Never> {
  Ok(
    controllers
      .iter()
      .enumerate()
      .find(|(at, _)| {
        let Ok(at_2) = fitted::<_, u32>(*at);

        at_2 < u32::BITS
          && possible
            .checked_shr(at_2)
            .is_some_and(|shifted| shifted & 1 == 1)
      })
      .map(|(_, controller)| *controller),
  )
}

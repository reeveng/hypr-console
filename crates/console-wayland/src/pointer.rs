//! Connection and wire operations for a Wayland virtual pointer.

use std::fmt;

use console_core_geometry::{Point, Size};
use console_core_number_conversion::fitted;

use wayland_client::backend::WaylandError;
use wayland_client::protocol::wl_pointer::{Axis, AxisSource, ButtonState};
use wayland_client::protocol::{wl_output, wl_registry, wl_seat};
use wayland_client::{ConnectError, Connection, Dispatch, DispatchError, QueueHandle};
use wayland_protocols::xdg::xdg_output::zv1::client::{
  zxdg_output_manager_v1::ZxdgOutputManagerV1, zxdg_output_v1, zxdg_output_v1::ZxdgOutputV1,
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
  zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
  zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

const BTN_LEFT: u32 = 0x110;

#[derive(Debug)]
pub enum Error {
  Connect(ConnectError),
  Measure(DispatchError),
  NoManager,
  Create(WaylandError),
  Write(WaylandError),
}

impl fmt::Display for Error {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Connect(fault) => write!(to, "no compositor to point at: {fault}"),
      Self::Measure(fault) => write!(to, "the compositor said nothing about itself: {fault}"),
      Self::NoManager => write!(
        to,
        "this compositor publishes no zwlr_virtual_pointer_manager_v1, \
         so nothing but a hand can move its pointer"
      ),
      Self::Create(fault) => write!(to, "the pointer could not be asked for: {fault}"),
      Self::Write(fault) => write!(to, "writing to the compositor: {fault}"),
    }
  }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
  Pressed,
  Released,
}

#[derive(Default)]
struct Found {
  manager: Option<ZwlrVirtualPointerManagerV1>,
  sizes: Option<ZxdgOutputManagerV1>,
  seat: Option<wl_seat::WlSeat>,
  screen: Option<wl_output::WlOutput>,
  room: Option<Size<u32>>,
}

pub struct Pointer {
  connection: Connection,
  pointer: ZwlrVirtualPointerV1,
  logical_size: Option<Size<u32>>,
}

impl Pointer {
  pub fn connect() -> Result<Self, Error> {
    let connection = crate::connect().map_err(Error::Connect)?;
    let mut queue = connection.new_event_queue();
    let hand = queue.handle();
    let _registry = connection.display().get_registry(&hand, ());

    let mut found = Found::default();
    queue.roundtrip(&mut found).map_err(Error::Measure)?;

    match (found.sizes.as_ref(), found.screen.as_ref()) {
      (Some(sizes), Some(screen)) => {
        let _requested = sizes.get_xdg_output(screen, &hand, ());
      }
      (Some(_), None) | (None, _) => {}
    }

    match queue.roundtrip(&mut found) {
      Ok(_answered) => {}
      Err(fault) => console_core_logging::warn!("the screen said nothing of its size: {fault}"),
    }

    let manager = found.manager.ok_or(Error::NoManager)?;
    let pointer = manager.create_virtual_pointer_with_output(
      found.seat.as_ref(),
      found.screen.as_ref(),
      &hand,
      (),
    );
    connection.flush().map_err(Error::Create)?;

    Ok(Self {
      connection,
      pointer,
      logical_size: found.room,
    })
  }

  #[must_use]
  pub fn logical_size(&self) -> Result<Option<Size<u32>>, console_core_never::Never> {
    Ok(self.logical_size)
  }

  pub fn move_to(&self, time: u32, at: Point<u32>, size: Size<u32>) -> Result<(), Error> {
    self
      .pointer
      .motion_absolute(time, at.x, at.y, size.width, size.height);
    self.frame()
  }

  pub fn button(&self, time: u32, state: Button) -> Result<(), Error> {
    let state_2 = match state {
      Button::Pressed => ButtonState::Pressed,
      Button::Released => ButtonState::Released,
    };
    self.pointer.button(time, BTN_LEFT, state_2);
    self.frame()
  }

  pub fn scroll(&self, time: u32, motion: ScrollMotion) -> Result<(), Error> {
    let distance = f64::from(motion.steps) * f64::from(motion.logical_pixels_per_step);
    self.pointer.axis_source(AxisSource::Wheel);
    self
      .pointer
      .axis_discrete(time, Axis::VerticalScroll, distance, motion.steps);
    self.frame()
  }

  fn frame(&self) -> Result<(), Error> {
    self.pointer.frame();
    self.connection.flush().map_err(Error::Write)
  }
}

pub struct ScrollMotion {
  pub steps: i32,
  pub logical_pixels_per_step: u32,
}

impl Dispatch<wl_registry::WlRegistry, ()> for Found {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    found: &mut Self,
    registry: &wl_registry::WlRegistry,
    event: wl_registry::Event,
    _: &(),
    _: &Connection,
    handle: &QueueHandle<Self>,
  ) {
    let (name, interface, version) = match event {
      wl_registry::Event::Global {
        name,
        interface,
        version,
      } => (name, interface, version),
      _the_registry_said_something_else => return,
    };

    match interface.as_str() {
      "zwlr_virtual_pointer_manager_v1" => {
        found.manager = Some(registry.bind(name, version.min(2), handle, ()));
      }
      "zxdg_output_manager_v1" => {
        found.sizes = Some(registry.bind(name, version.min(3), handle, ()));
      }
      "wl_seat" => found.seat = Some(registry.bind(name, version.min(5), handle, ())),
      "wl_output" => match &found.screen {
        None => {
          found.screen = Some(registry.bind(name, version.min(3), handle, ()));
        }
        Some(_) => {}
      },
      _ => {}
    }
  }
}

impl Dispatch<ZxdgOutputV1, ()> for Found {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    found: &mut Self,
    _: &ZxdgOutputV1,
    event: zxdg_output_v1::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    let (width, height) = match event {
      zxdg_output_v1::Event::LogicalSize { width, height } => (width, height),
      _the_output_said_something_else => return,
    };

    let Ok(wide) = fitted(width);
    let Ok(tall) = fitted(height);

    found.room = Some(Size {
      width: wide,
      height: tall,
    });
  }
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "Wayland requires an event dispatch implementation for each bound proxy; these proxies have identical empty handlers and need only supply their types"
  )
)]
macro_rules! says_nothing {
    ($($what:ty),*) => {
        $(impl Dispatch<$what, ()> for Found {
            fn event(
                _: &mut Self,
                _: &$what,
                _: <$what as wayland_client::Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        })*
    };
}

says_nothing!(
  ZwlrVirtualPointerManagerV1,
  ZwlrVirtualPointerV1,
  ZxdgOutputManagerV1,
  wl_seat::WlSeat,
  wl_output::WlOutput
);

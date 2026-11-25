//! Prepare the first keyboard frame while the resident service is hidden.
//! Only an opening with the same layout, input state, size, and scale can use
//! the worker's pixels. A slow worker or a changed display uses live drawing.

use std::io;
use std::sync::mpsc::{Receiver, TryRecvError, TrySendError, sync_channel};

use console_concurrency::HostWorkers;
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_i32};
use console_core_parallel_execution::Workers;
use console_draw_painting::text::FontCatalog;
use console_input_keyboard::configuration::Configuration;
use console_wayland::buffer::PreparedFrame;

use super::KeyboardState;

const MAX_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn geometry(
  screen: Option<console_screen::Screen>,
  height: u32,
) -> Result<Option<((u32, u32), i32)>, Never> {
  let held = match screen {
    Some(output) => output,
    None => return Ok(None),
  };
  let Ok(logical) = held.logical();
  let Ok(scale) = toward_zero_i32(held.scale.ceil());

  Ok(Some(((logical.width, height), scale.max(1))))
}

pub(super) enum Opening {
  None,
  Preparing(Receiver<Ready>),
}

pub(super) struct Ready {
  view: KeyboardState,
  pub(super) pixels: PreparedFrame,
  pub(super) fonts: FontCatalog,
}

struct PaintOpening {
  configuration: Configuration,
  view: KeyboardState,
  device: Size<u32>,
}

impl Opening {
  pub(super) fn start(configuration: &Configuration, view: KeyboardState) -> Result<Self, Never> {
    let Ok(scale) = fitted::<_, u32>(view.scale);
    let device = Size {
      width: view.size.0.saturating_mul(scale),
      height: view.size.1.saturating_mul(scale),
    };
    let bytes = u64::from(device.width)
      .saturating_mul(u64::from(device.height))
      .saturating_mul(4);

    match (1..=MAX_BYTES).contains(&bytes).then_some(()) {
      Some(()) => {}
      None => return Ok(Self::None),
    }

    let effect = PaintOpening {
      configuration: configuration.clone(),
      view,
      device,
    };
    let (returning, ready) = sync_channel(1);
    let started = HostWorkers.start("keyboard opening frame".to_owned(), move || {
      match effect.paint() {
        Ok(prepared) => match returning.try_send(prepared) {
          Ok(()) => {}
          Err(TrySendError::Disconnected(stale) | TrySendError::Full(stale)) => drop(stale),
        },
        Err(fault) => console_core_logging::debug!("preparing the keyboard opening: {fault}"),
      }
    });

    Ok(match started {
      Ok(_one_shot) => Self::Preparing(ready),
      Err(fault) => {
        console_core_logging::debug!("starting keyboard preparation: {fault}");

        Self::None
      }
    })
  }

  pub(super) fn take(&mut self, view: KeyboardState) -> Result<Option<Ready>, Never> {
    let receiver = match std::mem::replace(self, Self::None) {
      Self::None => return Ok(None),
      Self::Preparing(ready) => ready,
    };

    Ok(match receiver.try_recv() {
      Ok(prepared) => (prepared.view == view).then_some(prepared),
      Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
    })
  }
}

impl PaintOpening {
  fn paint(self) -> io::Result<Ready> {
    let Ok((fonts, painted)) = FontCatalog::discover_with(|| {
      PreparedFrame::paint(self.device, |pixels| {
        self.view.paint(&self.configuration, pixels, self.device)
      })
    });
    let pixels = painted?;

    Ok(Ready {
      view: self.view,
      pixels,
      fonts,
    })
  }
}

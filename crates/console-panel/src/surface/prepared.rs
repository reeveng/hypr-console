//! Retain an opening's pixels only when a later opening can validate the same
//! shapes and geometry. The compositor can attach the retained storage directly.

use std::sync::mpsc::{SyncSender, TrySendError};

use console_core_color::palette::Wearing;
use console_core_geometry::Size;
use console_core_never::Never;
use console_draw_surface::LayerSupport;
use console_draw_surface::standing::{DrawingSender, PreparedFrame, SurfaceError};

use super::{Connected, Panelled, Part, Shape, Surface, Visible, paint_region};

const MAX_FRAME_BYTES: u64 = 16 * 1024 * 1024;

const MAX_SHAPES: u32 = 256;

pub struct PreparedCard {
  part: Part,
  shapes: Vec<Shape>,
  pixels: PreparedFrame,
  fixed: Option<Fixed>,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Appearance {
  pub identity: &'static str,
  pub logical: Size<u32>,
  pub wearing: Wearing,
}

#[derive(Clone, Copy)]
pub(super) struct Fixed {
  pub appearance: Appearance,
  pub rows: u64,
}

pub(super) struct Preview {
  pub panel: Panelled,
  pub rows: u64,
  pub painted: Painted,
}

pub struct Preparing {
  pub previous: Option<PreparedCard>,
  pub remember: SyncSender<PreparedCard>,
  pub connected: Option<Connected>,
}

impl Preparing {
  pub(super) fn preview(
    &mut self,
    surface: &mut Surface,
    appearance: Appearance,
    finished: &mut Option<DrawingSender>,
  ) -> Result<Option<Preview>, SurfaceError> {
    let Ok(scale) = surface.scale();
    let cached = match (&self.previous, surface.layer_support()) {
      (Some(frame), Ok(LayerSupport::Available)) => frame,
      (None, _) | (_, Ok(LayerSupport::Unavailable)) => return Ok(None),
    };
    let Ok(device) = scale.device(cached.part.size);
    let fixed = match cached.fixed.filter(|known| known.appearance == appearance) {
      Some(fixed) => fixed,
      None => return Ok(None),
    };

    match (cached.pixels.size() == Ok(device)).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    let frame = match self.previous.take() {
      Some(frame) => frame,
      None => return Ok(None),
    };
    let PreparedCard {
      part,
      shapes,
      pixels: stored,
      fixed: _,
    } = frame;
    let mut prepared = Some(stored);
    let mut painted = Painted::No;
    let visible = surface.draw_prepared_layer(
      part,
      finished,
      (&mut prepared, &mut painted),
      |used, pixels, raster_size, _scale| {
        *used = Painted::Yes;

        paint_region(pixels, raster_size, (part, &shapes))
      },
    )?;

    match visible {
      Visible::Yes => {
        let panel = Panelled {
          shapes: shapes.clone(),
          touching: Vec::new(),
          moving: None,
        };

        match prepared {
          Some(pixels) => {
            let Ok(()) = published(
              &self.remember,
              PreparedCard {
                part,
                shapes,
                pixels,
                fixed: Some(fixed),
              },
            );
          }
          None => {}
        }

        Ok(Some(Preview {
          panel,
          rows: fixed.rows,
          painted,
        }))
      }
      Visible::NotYet => {
        self.previous = prepared.map(|pixels| PreparedCard {
          part,
          shapes,
          pixels,
          fixed: Some(fixed),
        });

        Ok(None)
      }
    }
  }

  pub(super) fn draw(
    self,
    surface: &mut Surface,
    (part, panel, fixed): (Part, &Panelled, Option<Fixed>),
    finished: &mut Option<DrawingSender>,
  ) -> Result<Visible, SurfaceError> {
    let mut painted = Painted::No;
    let paint =
      |pixels: &mut [u8], device, _scale| paint_region(pixels, device, (part, &panel.shapes));
    let Ok(shape_limit) = console_core_number_conversion::index(MAX_SHAPES);
    let Ok(scale) = surface.scale();
    let Ok(device) = scale.device(part.size);
    let bytes = u64::from(device.width)
      .saturating_mul(u64::from(device.height))
      .saturating_mul(4);

    match (bytes <= MAX_FRAME_BYTES
      && panel.shapes.len() <= shape_limit
      && panel.moving.is_none()
      && !panel
        .shapes
        .iter()
        .any(|shape| matches!(shape, Shape::Picture(_) | Shape::Cropped(_))))
    .then_some(())
    {
      None => return surface.draw_layer_on_backdrop(part, finished, paint),
      Some(()) => {}
    }

    let matching = self.previous.filter(|held| {
      held.part == part && held.shapes == panel.shapes && held.pixels.size() == Ok(device)
    });
    let Ok(mut saved) = Reusable::from_previous(matching);
    let visible = surface.draw_prepared_layer(
      part,
      finished,
      (&mut saved.pixels, &mut painted),
      |state, pixels, raster_size, raster_scale| {
        *state = Painted::Yes;

        paint(pixels, raster_size, raster_scale)
      },
    )?;

    match (visible, painted) {
      (Visible::Yes, Painted::No) => {
        let Ok(()) = crate::opening::counted("prepared_frame", 1);
      }
      (Visible::Yes, Painted::Yes) | (Visible::NotYet, _) => {}
    }

    let Ok(()) = saved.publish((part, panel, fixed), visible, self.remember);

    Ok(visible)
  }
}

pub(super) enum Painted {
  Yes,
  No,
}

struct Reusable {
  previous_shapes: Option<Vec<Shape>>,
  pixels: Option<PreparedFrame>,
}

impl Reusable {
  fn from_previous(previous: Option<PreparedCard>) -> Result<Self, Never> {
    Ok(match previous {
      Some(held) => Self {
        previous_shapes: Some(held.shapes),
        pixels: Some(held.pixels),
      },
      None => Self {
        previous_shapes: None,
        pixels: None,
      },
    })
  }

  fn publish(
    self,
    (part, panel, fixed): (Part, &Panelled, Option<Fixed>),
    visible: Visible,
    remember: SyncSender<PreparedCard>,
  ) -> Result<(), Never> {
    let pixels = match (visible, self.pixels) {
      (Visible::Yes, Some(pixels)) => pixels,
      (Visible::NotYet, _) | (Visible::Yes, None) => return Ok(()),
    };
    let shapes = match self.previous_shapes {
      Some(shapes) => shapes,
      None => panel.shapes.clone(),
    };

    published(
      &remember,
      PreparedCard {
        part,
        shapes,
        pixels,
        fixed,
      },
    )
  }
}

fn published(remember: &SyncSender<PreparedCard>, frame: PreparedCard) -> Result<(), Never> {
  match remember.try_send(frame) {
    Ok(()) => {}
    Err(TrySendError::Disconnected(returned) | TrySendError::Full(returned)) => drop(returned),
  }

  Ok(())
}

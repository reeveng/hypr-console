//! Whole-frame pictures can use the shared opaque resize directly.

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_picture_scaling::{ChannelOrder, resize_opaque};
use console_core_shapes::{Picture, Pixels};

use crate::{Frame, raster::Canvas};

pub(super) enum OpaquePaint {
  Painted,
  Fallback,
}

pub(super) fn paint(
  canvas: &mut Canvas<'_>,
  frame: Frame,
  picture: &Picture<&Pixels>,
) -> Result<OpaquePaint, Never> {
  let source = picture.pixels;
  let fits = picture.at == (Point { x: 0, y: 0 })
    && picture.size == frame.points
    && frame.points.width != 0
    && frame.points.height != 0
    && source.width.checked_mul(4) == Some(source.stride);

  match fits.then_some(()) {
    Some(()) => {}
    None => return Ok(OpaquePaint::Fallback),
  }

  #[cfg(target_endian = "little")]
  let order = ChannelOrder::Bgra;
  #[cfg(target_endian = "big")]
  let order = ChannelOrder::Rgba;
  let Ok(size) = source.size();
  let Ok(destination) = canvas.pixels.data_mut();
  let resized = resize_opaque(
    source.bytes.as_chunks::<4>().0,
    size,
    destination.as_chunks_mut::<4>().0,
    frame.device,
    order,
  );

  Ok(match resized {
    Ok(()) => OpaquePaint::Painted,
    Err(_incompatible_pixels) => OpaquePaint::Fallback,
  })
}

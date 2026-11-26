//! Whole-frame pictures can use the shared opaque resize directly.

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_picture_scaling::{ChannelOrder, resize_opaque};
use console_core_shapes::{Picture, PictureGeometry, Pixels};

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
  let rectangle = match picture.geometry {
    PictureGeometry::Rectangle(rectangle) => rectangle,
    PictureGeometry::Circle(_) | PictureGeometry::Triangle(_) => {
      return Ok(OpaquePaint::Fallback);
    }
  };
  let source = picture.pixels;
  let Ok(points) = frame.points.map(f64::from);
  let fits = rectangle.origin == Point::default()
    && rectangle.size == points
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

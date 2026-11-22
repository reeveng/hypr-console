//! A borrowed Wayland buffer while paths are drawn into it.
//!
//! The rasterizer blends each color channel independently. On little-endian
//! machines, feeding it blue in place of red draws directly into Wayland's
//! BGRA bytes. Only incoming colors and images change order, so a video overlay
//! touches its rectangle rather than converting the whole frame twice.
//!
//! On big-endian machines native ARGB also moves alpha; those buffers change
//! order while borrowed and are restored when the borrow ends. Both paths
//! keep the caller's bytes borrowed safely without a second frame buffer.

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, narrow_f32};
use console_core_path_rasterizer::{
  Color, ColorU8, FillRule, Mask, Paint, Path, PathBuilder, PixmapMut, Rect, Transform,
};
use console_core_paths::Point;
use console_core_shapes::Clip;
use kurbo::{PathEl, RoundedRect, Shape};

use crate::Cannot;

pub struct Canvas<'a> {
  pub(crate) pixels: PixmapMut<'a>,
  pub(crate) transform: Transform,
  pub(crate) clip: Option<Mask>,
}

impl<'a> Canvas<'a> {
  pub fn new(
    bytes: &'a mut [u8],
    device: Size<u32>,
    transform: Transform,
  ) -> Result<Self, Cannot> {
    match ::core::primitive::bool::then_some(({
      let Ok(explicit_value) = transform.is_finite();
      explicit_value == console_core_paths::IsFiniteAnswer::Yes
    }) && ({
      let Ok(explicit_value) = transform.is_valid();
      explicit_value == console_core_paths::IsValidAnswer::Yes
    }) && transform.sx > 0.0
      && transform.sy > 0.0, ())
    {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(Cannot::Transform),
    }

    let Ok(available) = fitted::<_, u64>(bytes.len());
    let required = u64::from(device.width)
      .checked_mul(u64::from(device.height))
      .and_then(|area| area.checked_mul(4))
      .ok_or(Cannot::Buffer {
        device,
        bytes: available,
      })?;
    let Ok(length) = index(required);
    let frame = bytes.get_mut(..length).ok_or(Cannot::Buffer {
      device,
      bytes: available,
    })?;
    let Ok(pixmap_mut) = PixmapMut::from_bytes(frame, device.width, device.height);
    let pixels = pixmap_mut.ok_or(Cannot::Buffer {
      device,
      bytes: available,
    })?;
    #[cfg(target_endian = "big")]
    let pixels = {
      let mut pixels = pixels;
      let Ok(data_mut) = pixels.data_mut();
      let Ok(()) = channels(data_mut);

      pixels
    };

    Ok(Self {
      pixels,
      transform,
      clip: None,
    })
  }

  pub fn fill_path(
    &mut self,
    path: &Path,
    paint: &Paint<'_>,
    rule: FillRule,
  ) -> Result<(), Never> {
    self
      .pixels
      .fill_path(path, paint, rule, self.transform, self.clip.as_ref())
  }

  pub fn set_clip(&mut self, clip: Clip) -> Result<(), Cannot> {
    self.clip = match clip {
      Clip::Lifted => None,
      Clip::To { at, size } => {
        let Ok(own_width) = self.pixels.width();
        let Ok(own_height) = self.pixels.height();
        let mut mask = ({
          let Ok(explicit_value) = Mask::new(own_width, own_height);
          explicit_value
        })
        .ok_or(Cannot::Mask)?;
        let Ok(x) = narrow_f32(f64::from(at.x));
        let Ok(y) = narrow_f32(f64::from(at.y));
        let Ok(width) = narrow_f32(f64::from(size.width));
        let Ok(height) = narrow_f32(f64::from(size.height));

        match {
          let Ok(explicit_value) = Rect::from_xywh(x, y, width, height);
          explicit_value
        } {
          Some(rect) => {
            let Ok(path_builder) = PathBuilder::from_rect(rect);
            let Ok(()) = mask.fill_path(
              &path_builder,
              FillRule::Winding,
              console_core_path_rasterizer::CoverageMode::Antialiased,
              self.transform,
            );
          }
          None => {}
        }

        Some(mask)
      }
    };

    Ok(())
  }
}

impl Drop for Canvas<'_> {
  fn drop(&mut self) {
    #[cfg(target_endian = "big")]
    let Ok(data_mut) = self.pixels.data_mut();
    #[cfg(target_endian = "big")]
    let Ok(()) = native_channels(data_mut);
  }
}

#[cfg(target_endian = "big")]
fn channels(bytes: &mut [u8]) -> Result<(), Never> {
  for pixel in bytes.as_chunks_mut::<4>().0 {
    let [alpha, red, green, blue] = *pixel;

    *pixel = [red, green, blue, alpha];
  }

  Ok(())
}

#[cfg(target_endian = "big")]
fn native_channels(bytes: &mut [u8]) -> Result<(), Never> {
  for pixel in bytes.as_chunks_mut::<4>().0 {
    let [red, green, blue, alpha] = *pixel;

    *pixel = [alpha, red, green, blue];
  }

  Ok(())
}

pub fn buffer_color(color: Color) -> Result<Color, Never> {
  #[cfg(target_endian = "little")]
  let adjusted = {
    let Ok(blue) = color.blue();
    let Ok(green) = color.green();
    let Ok(red) = color.red();
    let Ok(alpha) = color.alpha();
    let Ok(channels) = Color::from_rgba(blue, green, red, alpha);

    match channels {
      Some(ready) => ready,
      None => Color::TRANSPARENT,
    }
  };
  #[cfg(target_endian = "big")]
  let adjusted = color;

  Ok(adjusted)
}

pub fn buffer_pixel(color: ColorU8) -> Result<[u8; 4], Never> {
  let Ok(pixel) = color.premultiply();
  let Ok(red) = pixel.red();
  let Ok(green) = pixel.green();
  let Ok(blue) = pixel.blue();
  let Ok(alpha) = pixel.alpha();

  #[cfg(target_endian = "little")]
  let bytes = [blue, green, red, alpha];
  #[cfg(target_endian = "big")]
  let bytes = [red, green, blue, alpha];

  Ok(bytes)
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the existing rasterizer entry point accepts a radius in the same logical pixels as Rect"
  )
)]
pub fn rounded(rect: Rect, radius: f32) -> Result<Option<Path>, Never> {
  let radius_2 = radius.max(0.0).min(
    ({
      let Ok(explicit_value) = rect.width();
      explicit_value
    })
    .min({
      let Ok(value) = rect.height();
      value
    }) / 2.0,
  );

  match ::core::primitive::bool::then_some(radius_2 <= 0.0, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(path_builder) = PathBuilder::from_rect(rect);

      return Ok(Some(path_builder));
    }
    ::core::option::Option::None => {}
  }

  let Ok(left) = rect.left();
  let Ok(top) = rect.top();
  let Ok(right) = rect.right();
  let Ok(bottom) = rect.bottom();
  let radius_pixels = f64::from(radius_2);
  let rounded = RoundedRect::new(
    f64::from(left),
    f64::from(top),
    f64::from(right),
    f64::from(bottom),
    radius_pixels,
  );
  let Ok(mut path) = PathBuilder::new();

  const SINGLE_CUBIC_CORNER_TOLERANCE: f64 = 0.001;

  for element in rounded.path_elements(radius_pixels * SINGLE_CUBIC_CORNER_TOLERANCE) {
    let Ok(()) = match element {
      PathEl::MoveTo(end) => {
        let Ok(point) = path_point(end);
        path.move_to_pt(point)
      }
      PathEl::LineTo(end) => {
        let Ok(point) = path_point(end);
        path.line_to_pt(point)
      }
      PathEl::QuadTo(control, end) => {
        let Ok(handle) = path_point(control);
        let Ok(destination) = path_point(end);
        path.quad_to_pt(handle, destination)
      }
      PathEl::CurveTo(first, second, end) => {
        let Ok(handle_in) = path_point(first);
        let Ok(handle_out) = path_point(second);
        let Ok(destination) = path_point(end);
        path.cubic_to_pt(handle_in, handle_out, destination)
      }
      PathEl::ClosePath => path.close(),
    };
  }

  let Ok(finish) = path.finish();

  Ok(finish)
}

fn path_point(point: kurbo::Point) -> Result<Point, Never> {
  let Ok(x) = narrow_f32(point.x);
  let Ok(y) = narrow_f32(point.y);
  Ok(Point { x, y })
}

#[cfg(test)]
mod tests {
  use super::{Rect, rounded};
  use console_core_paths::PathSegment;

  #[test]
  fn rounded_panel_has_four_cubic_corners_and_the_original_bounds() -> Result<(), &'static str> {
    let Ok(rect) = Rect::from_ltrb(1.0, 2.0, 31.0, 22.0);
    let rectangle = rect.ok_or("positive panel dimensions")?;
    let Ok(rounded_panel) = rounded(rectangle, 8.0);
    let path = rounded_panel.ok_or("rounded panel path")?;

    assert_eq!(path.bounds(), Ok(rectangle));

    let Ok(segments) = path.segments();
    assert_eq!(
      segments.clone().filter(|segment| matches!(segment, PathSegment::CubicTo(..))).count(),
      4
    );
    assert!(matches!(segments.last(), Some(PathSegment::Close)));

    Ok(())
  }
}

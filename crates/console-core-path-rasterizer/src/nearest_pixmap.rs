//! Nearest-neighbor source-over spans for axis-aligned pixmaps, including glyphs.
//! Transformed rectangles use the same fixed-point edge grid as the binary path scanner.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_u32};
use console_core_paths::{HasSkewAnswer, IsIdentityAnswer};

use crate::fixed_point::{PixelCoordinate, fdot6};
use crate::geom::{IntSizeExt, PixelPosition};
use crate::{FloatRectangle, IntRect, PixmapMut, PixmapRef, Transform};

pub(super) enum Drawing {
  Drawn,
  Unsupported,
}

pub(super) fn draw(
  target: &mut PixmapMut<'_>,
  source: PixmapRef<'_>,
  rect: FloatRectangle,
  [pattern, transform]: [Transform; 2],
) -> Result<Drawing, Never> {
  let Ok(size) = target.size();
  let Ok(tiling) = crate::painter::DrawTiler::required(size);
  let Ok(skew) = transform.has_skew();

  match (tiling, skew) {
    (crate::RequiredAnswer::No, HasSkewAnswer::No) => {}
    (crate::RequiredAnswer::Yes, _) | (_, HasSkewAnswer::Yes) => return Ok(Drawing::Unsupported),
  }

  let Ok(placement) = pattern.post_concat(transform);
  let Ok(inverted) = placement.invert();
  let Ok(target_extent) = size.at_origin();
  let (inverse, extent) = match (inverted, target_extent) {
    (Some(inverse), Some(clip)) => (inverse, clip),
    (None, _) | (_, None) => return Ok(Drawing::Unsupported),
  };

  let Ok(clip) = extent.to_int_rect();
  let Ok(bounds) = device_rect(rect, transform);
  let intersection = match bounds {
    Some(rectangle) => rectangle.intersect(&clip)?,
    None => None,
  };
  let visible = match intersection {
    Some(visible) => visible,
    None => return Ok(Drawing::Drawn),
  };
  let Ok(target_width) = target.width();
  let Ok(source_width) = source.width();
  let Ok(source_height) = source.height();
  let Ok(pixels) = source.data();
  let Ok(destination) = target.data_mut();
  let mut blitter = Nearest {
    input: pixels.as_chunks::<4>().0,
    output: destination.as_chunks_mut::<4>().0,
    source_size: [source_width, source_height],
    device_columns: target_width,
    inverse,
  };
  let Ok(()) = blitter.rectangle(visible);

  Ok(Drawing::Drawn)
}

fn device_rect(rect: FloatRectangle, transform: Transform) -> Result<Option<IntRect>, Never> {
  let Ok(identity) = transform.is_identity();

  match identity {
    IsIdentityAnswer::Yes => rect.round(),
    IsIdentityAnswer::No => {
      let Ok(transformed) = rect.transform(transform);
      let bounds = match transformed {
        Some(bounds) => bounds,
        None => return Ok(None),
      };
      let Ok(left) = bounds.left();
      let Ok(top) = bounds.top();
      let Ok(right) = bounds.right();
      let Ok(bottom) = bounds.bottom();
      let [x0, y0, x1, y1] = [left, top, right, bottom].map(|edge| {
        let Ok(value) = fdot6::from_f32(PixelCoordinate(edge.clamp(-1.0, 32_767.0)));
        let Ok(pixel) = fdot6::round(value);
        pixel
      });

      IntRect::from_ltrb(x0, y0, x1, y1)
    }
  }
}

struct Nearest<'a> {
  input: &'a [[u8; 4]],
  output: &'a mut [[u8; 4]],
  source_size: [u32; 2],
  device_columns: u32,
  inverse: Transform,
}

impl Nearest<'_> {
  fn rectangle(&mut self, bounds: IntRect) -> Result<(), Never> {
    let Ok(left) = bounds.left();
    let Ok(top) = bounds.top();
    let Ok(bottom) = bounds.bottom();
    let Ok(x) = fitted::<i32, u32>(left);
    let Ok(first) = fitted::<i32, u32>(top);
    let Ok(stop) = fitted::<i32, u32>(bottom);
    let Ok(width) = bounds.width();

    for y in first..stop {
      let Ok(()) = self.span(PixelPosition { x, y }, width);
    }

    Ok(())
  }

  fn span(&mut self, at: PixelPosition, width: u32) -> Result<(), Never> {
    let [wide, tall] = self.source_size;
    let first = u64::from(at.y)
      .saturating_mul(u64::from(self.device_columns))
      .saturating_add(u64::from(at.x));
    let Ok(destination_start) = index(first);
    let Ok(destination_end) = index(first.saturating_add(u64::from(width)));
    let span = match self.output.get_mut(destination_start..destination_end) {
      Some(span) => span,
      None => return Ok(()),
    };
    let Ok(row) = sample(
      at.y,
      SampleAxis {
        size: tall,
        transform: [self.inverse.sy, self.inverse.ty],
      },
    );
    let start = u64::from(row).saturating_mul(u64::from(wide));
    let Ok(source_start) = index(start);
    let Ok(source_end) = index(start.saturating_add(u64::from(wide)));
    let source = match self.input.get(source_start..source_end) {
      Some(source) => source,
      None => return Ok(()),
    };

    let horizontal = SampleAxis {
      size: wide,
      transform: [self.inverse.sx, self.inverse.tx],
    };

    for (x, destination) in (at.x..).zip(span) {
      let Ok(column) = sample(x, horizontal);
      let Ok(column_index) = index(column);

      match source.get(column_index) {
        Some(pixel) => {
          let Ok(()) = over(destination, *pixel);
        }
        None => {}
      }
    }

    Ok(())
  }
}

#[derive(Clone, Copy)]
struct SampleAxis {
  size: u32,
  transform: [f32; 2],
}

fn sample(coordinate: u32, axis: SampleAxis) -> Result<u32, Never> {
  let Ok(pixel) = fitted::<u32, u16>(coordinate);
  let [scale, offset] = axis.transform;
  let sample = (f32::from(pixel) + 0.5) * scale + offset;
  let Ok(inside) = toward_zero_u32(f64::from(sample));

  Ok(inside.min(axis.size.saturating_sub(1)))
}

fn over(destination: &mut [u8; 4], source: [u8; 4]) -> Result<(), Never> {
  match source {
    [0, 0, 0, 0] => {}
    [_, _, _, 255] => *destination = source,
    [_, _, _, alpha] => {
      let uncovered = 255_u16.saturating_sub(u16::from(alpha));

      for (dst, src) in destination.iter_mut().zip(source) {
        let blended = u16::from(*dst)
          .wrapping_mul(uncovered)
          .wrapping_add(127)
          .wrapping_div(255);
        let Ok(channel) = fitted::<u16, u8>(u16::from(src).saturating_add(blended));
        *dst = channel;
      }
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    BlendMode, CoverageMode, FilterQuality, Paint, Pattern, Pixmap, PixmapPaint, SpreadMode,
  };

  #[test]
  fn nearest_spans_match_the_general_pipeline_at_fractional_scale() -> Result<(), &'static str> {
    let Ok(pixmap) = PixmapRef::from_bytes(
      &[
        0, 0, 0, 0, 35, 48, 90, 100, 80, 75, 65, 255, 5, 7, 8, 20, 2, 59, 120, 155, 77, 128, 21,
        222, 11, 12, 8, 20, 1, 2, 3, 255,
      ],
      4,
      2,
    );
    let source = pixmap.ok_or("source pixmap")?;
    let Ok(rectangle) = FloatRectangle::from_xywh(0.0, 0.0, 4.0, 2.0);
    let bounds = rectangle.ok_or("source rectangle")?;
    let Ok(identity) = Transform::identity();
    let Ok(shader) = Pattern::new(
      source,
      SpreadMode::Pad,
      FilterQuality::Nearest,
      1.0,
      identity,
    );
    let paint = Paint {
      shader,
      blend_mode: BlendMode::SourceOver,
      coverage: CoverageMode::Binary,
      ..Paint::default()
    };

    for scale in [0.5, 0.75, 1.0, 1.3, 1.5, 2.0] {
      for offset in [-1.5, 0.0, 0.25, 0.5, 1.75, 5.0] {
        let transform = Transform {
          sx: scale,
          sy: scale,
          tx: offset,
          ty: offset,
          ..identity
        };
        let Ok(target) = Pixmap::new(8, 8);
        let mut expected = target.ok_or("target pixmap")?;
        let Ok(background) = crate::Color::from_rgba8(241, 33, 202, 191);
        let Ok(()) = expected.fill(background);
        let mut actual = expected.clone();

        let Ok(mut reference) = expected.as_mut();
        let Ok(()) = reference.fill_rect(bounds, &paint, transform, None);
        let Ok(mut optimized) = actual.as_mut();
        let Ok(()) =
          optimized.draw_pixmap(0, 0, source, &PixmapPaint::default(), transform, None);

        assert_eq!(actual, expected, "scale {scale}, offset {offset}");
      }
    }

    Ok(())
  }
}

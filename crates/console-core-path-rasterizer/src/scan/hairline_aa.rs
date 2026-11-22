use core::num::NonZeroU16;

use console_core_fixed_capacity_vectors::FixedVec;
use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};
use console_core_paths::ContainsAnswer;
use console_core_size::Size;

use super::hairline::{HairPoint, intersect_segment};
use crate::alpha_runs::{AlphaRun, AlphaRuns};
use crate::blitter::{Blitter, CoverageBlitter};
use crate::color::AlphaU8;
use crate::fixed_point::{
  DeltaRatio, FDot6, FDot8, FDot16, PixelCoordinate, fdot6, fdot8, fdot16,
};
use crate::geom::{IntRectExt, PixelPosition, ScreenIntRect};
use crate::math::LENGTH_U32_ONE;
use crate::{IntRect, LengthU32, LineCap, Path, Point, Rect};

#[derive(Copy, Clone)]
struct SubpixelSpan {
  start: FDot8,
  end: FDot8,
}

struct SubpixelRect {
  columns: SubpixelSpan,
  rows: SubpixelSpan,
}

impl SubpixelRect {
  fn from_rect(rect: &Rect) -> Result<Self, Never> {
    let Ok(left) = rect.left();
    let Ok(top) = rect.top();
    let Ok(right) = rect.right();
    let Ok(bottom) = rect.bottom();
    let [x0, y0, x1, y1] = [left, top, right, bottom].map(|coordinate| {
      let Ok(fixed) = fdot16::from_f32(PixelCoordinate(coordinate));
      let Ok(subpixel) = fdot8::from_fdot16(fixed);

      subpixel
    });

    Ok(Self {
      columns: SubpixelSpan { start: x0, end: x1 },
      rows: SubpixelSpan { start: y0, end: y1 },
    })
  }
}

impl SubpixelSpan {
  fn bands(self) -> Result<SubpixelBands, Never> {
    match (self.start < self.end).then_some(()) {
      Some(()) => {}
      None => return Ok(SubpixelBands::Empty),
    }

    let start = self.start.wrapping_shr(8);

    match (start == self.end.wrapping_sub(1).wrapping_shr(8)).then_some(()) {
      Some(()) => {
        return Ok(SubpixelBands::Single {
          pixel: start,
          coverage: self.end.wrapping_sub(self.start),
        });
      }
      None => {}
    }

    let end = self.end.wrapping_shr(8);
    let head = match self.start & 255 {
      0 => None,
      start_fraction => Some(256i32.wrapping_sub(start_fraction)),
    };
    let tail = match self.end & 255 {
      0 => None,
      end_fraction => Some(end_fraction),
    };

    Ok(SubpixelBands::Multiple {
      start,
      head,
      end,
      tail,
    })
  }
}

enum SubpixelBands {
  Empty,
  Single {
    pixel: i32,
    coverage: i32,
  },
  Multiple {
    start: i32,
    head: Option<i32>,
    end: i32,
    tail: Option<i32>,
  },
}

pub fn fill_rect(
  rect: &Rect,
  clip: &ScreenIntRect,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  let Ok(clip_rect) = clip.to_rect();
  let Ok(intersection) = rect.intersect(&clip_rect);
  let clipped = match intersection {
    Some(value) => value,
    None => return Ok(()),
  };
  let Ok(area) = SubpixelRect::from_rect(&clipped);
  let Ok(columns) = area.columns.bands();
  let Ok(rows) = area.rows.bands();

  match rows {
    SubpixelBands::Empty => {}
    SubpixelBands::Single { pixel, coverage } => {
      let Ok(alpha) = wrapped::<_, u8>(coverage.wrapping_sub(1));
      let Ok(()) = do_scanline(&columns, pixel, alpha, blitter);
    }
    SubpixelBands::Multiple {
      start,
      head,
      end,
      tail,
    } => {
      let top = match head {
        Some(coverage) => {
          let Ok(alpha) = wrapped::<_, u8>(coverage);
          let Ok(()) = do_scanline(&columns, start, alpha, blitter);

          start.wrapping_add(1)
        }
        None => start,
      };
      let Ok(height) = nonzero_length(end.wrapping_sub(top));

      match height {
        Some(rows_2) => {
          let Ok(()) = fill_rows(&columns, top, rows_2, blitter);
        }
        None => {}
      }

      match tail {
        Some(coverage) => {
          let Ok(alpha) = wrapped::<_, u8>(coverage);
          let Ok(()) = do_scanline(&columns, end, alpha, blitter);
        }
        None => {}
      }
    }
  }

  Ok(())
}

fn fill_rows(
  columns: &SubpixelBands,
  top: i32,
  height: LengthU32,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  match *columns {
    SubpixelBands::Empty => {}
    SubpixelBands::Single { pixel, coverage } => {
      let Ok(alpha) = wrapped::<_, u8>(coverage.wrapping_sub(1));
      let Ok(()) = vertical_coverage([pixel, top], height, alpha, blitter);
    }
    SubpixelBands::Multiple {
      start,
      head,
      end,
      tail,
    } => {
      let left = match head {
        Some(coverage) => {
          let Ok(alpha) = wrapped::<_, u8>(coverage);
          let Ok(()) = vertical_coverage([start, top], height, alpha, blitter);

          start.wrapping_add(1)
        }
        None => start,
      };
      let Ok(width) = nonzero_length(end.wrapping_sub(left));
      let Ok(origin) = pixel_position([left, top]);

      match (origin, width) {
        (Some(at), Some(columns_2)) => {
          let Ok(()) = paint_full_rows(
            at,
            Size {
              width: columns_2.get(),
              height: height.get(),
            },
            blitter,
          );
        }
        _ => {}
      }

      match tail {
        Some(coverage) => {
          let Ok(alpha) = wrapped::<_, u8>(coverage);
          let Ok(()) = vertical_coverage([end, top], height, alpha, blitter);
        }
        None => {}
      }
    }
  }

  Ok(())
}

fn paint_full_rows(
  at: PixelPosition,
  size: Size<u32>,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  let Ok(bounds) = ScreenIntRect::new(at, size);

  match bounds {
    Some(rect) => blitter.blit_rect(&rect),
    None => Ok(()),
  }
}

fn vertical_coverage(
  position: [i32; 2],
  height: LengthU32,
  alpha: AlphaU8,
  blitter: &mut dyn CoverageBlitter,
) -> Result<(), Never> {
  match pixel_position(position) {
    Ok(Some(at)) => blitter.blit_v(at, height, alpha),
    Ok(None) => Ok(()),
  }
}

fn pixel_position([x, y]: [i32; 2]) -> Result<Option<PixelPosition>, Never> {
  match (u32::try_from(x), u32::try_from(y)) {
    (Ok(column), Ok(row)) => Ok(Some(PixelPosition { x: column, y: row })),
    _ => Ok(None),
  }
}

fn nonzero_length(count: i32) -> Result<Option<LengthU32>, Never> {
  Ok(match u32::try_from(count) {
    Ok(value) => LengthU32::new(value),
    Err(_negative_length) => None,
  })
}

fn alpha_mul(value: AlphaU8, scale: i32) -> Result<u8, Never> {
  wrapped(i32::from(value).wrapping_mul(scale).wrapping_shr(8))
}

fn do_scanline(
  columns: &SubpixelBands,
  row: i32,
  alpha: AlphaU8,
  blitter: &mut dyn CoverageBlitter,
) -> Result<(), Never> {
  match *columns {
    SubpixelBands::Empty => {}
    SubpixelBands::Single { pixel, coverage } => {
      let Ok(attenuated) = alpha_mul(alpha, coverage);
      let Ok(()) = vertical_coverage([pixel, row], LENGTH_U32_ONE, attenuated, blitter);
    }
    SubpixelBands::Multiple {
      start,
      head,
      end,
      tail,
    } => {
      let left = match head {
        Some(coverage) => {
          let Ok(attenuated) = alpha_mul(alpha, coverage);
          let Ok(()) = vertical_coverage([start, row], LENGTH_U32_ONE, attenuated, blitter);

          start.wrapping_add(1)
        }
        None => start,
      };
      let Ok(length) = nonzero_length(end.wrapping_sub(left));
      let Ok(origin) = pixel_position([left, row]);

      match (origin, length) {
        (Some(at), Some(width)) => {
          let Ok(()) = call_hline_blitter(at.x, Some(at.y), width, alpha, blitter);
        }
        _ => {}
      }

      match tail {
        Some(coverage) => {
          let Ok(attenuated) = alpha_mul(alpha, coverage);
          let Ok(()) = vertical_coverage([end, row], LENGTH_U32_ONE, attenuated, blitter);
        }
        None => {}
      }
    }
  }

  Ok(())
}

fn call_hline_blitter(
  x: u32,
  row: Option<u32>,
  count: LengthU32,
  alpha: AlphaU8,
  blitter: &mut dyn CoverageBlitter,
) -> Result<(), Never> {
  let y = match row {
    Some(value) => value,
    None => return Ok(()),
  };
  let mut runs = [None; 101];
  let mut coverage = [0u8; 100];

  for offset in (0..count.get()).step_by(100) {
    let remaining = count.get().wrapping_sub(offset).min(100);
    let Ok(amount) = wrapped::<_, u16>(remaining);
    let Ok(terminator) = index(amount);
    let [head, ..] = &mut runs;
    *head = NonZeroU16::new(amount);
    let [first, ..] = &mut coverage;
    *first = alpha;

    match runs.get_mut(terminator) {
      Some(end) => *end = None,
      None => return Ok(()),
    }

    let at = PixelPosition {
      x: x.wrapping_add(offset),
      y,
    };
    let Ok(()) = blitter.blit_anti_h(at, &mut coverage, &mut runs);
  }

  Ok(())
}

pub fn stroke_path(
  path: &Path,
  line_cap: LineCap,
  clip: &ScreenIntRect,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  super::hairline::stroke_path_impl(path, line_cap, clip, anti_hair_line_rgn, blitter)
}

fn anti_hair_line_rgn(
  points: &[Point],
  clip: Option<&ScreenIntRect>,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  let Ok(rect) = Rect::from_ltrb(-32767.0, -32767.0, 32767.0, 32767.0);
  let fixed_bounds = match rect {
    Some(bounds) => bounds,
    None => return Ok(()),
  };
  let padded = match clip {
    Some(region) => {
      let Ok(clip_rect) = region.to_rect();
      let Ok(outset) = clip_rect.outset(1.0, 1.0);

      outset
    }
    None => None,
  };

  for segment in points.array_windows::<2>() {
    let Ok(fixed) = intersect_segment(segment, &fixed_bounds);
    let candidate = match fixed {
      Some(line) => line,
      None => continue,
    };
    let Ok(clipped) = match padded {
      Some(outset) => intersect_segment(&candidate, &outset),
      None => Ok(Some(candidate)),
    };
    let [first, last] = match clipped {
      Some(line) => line,
      None => continue,
    };
    let Ok(start) = HairPoint::new(first);
    let Ok(end) = HairPoint::new(last);
    let line = match (start, end) {
      (Some(a), Some(b)) => [a, b],
      _ => continue,
    };
    let Ok(visibility) = match clip {
      Some(region) => segment_clip(line, region),
      None => Ok(SegmentClip::Inside),
    };

    match visibility {
      SegmentClip::Outside => {}
      SegmentClip::Inside => {
        let Ok(()) = rasterize_segments(line, None, blitter);
      }
      SegmentClip::Partial(region) => {
        let Ok(()) = rasterize_segments(line, Some(region), blitter);
      }
    }
  }

  Ok(())
}

enum SegmentClip {
  Outside,
  Inside,
  Partial(ScreenIntRect),
}

fn segment_clip(
  [start, end]: [HairPoint; 2],
  clip: &ScreenIntRect,
) -> Result<SegmentClip, Never> {
  let Ok(left) = fdot6::floor(start.x.min(end.x));
  let Ok(top) = fdot6::floor(start.y.min(end.y));
  let Ok(right) = fdot6::ceil(start.x.max(end.x));
  let Ok(bottom) = fdot6::ceil(start.y.max(end.y));
  let Ok(bounds) = IntRect::from_ltrb(
    left.wrapping_sub(1),
    top.wrapping_sub(1),
    right.wrapping_add(1),
    bottom.wrapping_add(1),
  );
  let extent = match bounds {
    Some(rect) => rect,
    None => return Ok(SegmentClip::Outside),
  };
  let Ok(clip_rect) = clip.to_int_rect();
  let Ok(intersection) = clip_rect.intersect(&extent);
  let overlap = match intersection {
    Some(rect) => rect,
    None => return Ok(SegmentClip::Outside),
  };
  let Ok(contained) = clip_rect.contains(&extent);

  Ok(match contained {
    ContainsAnswer::Yes => SegmentClip::Inside,
    ContainsAnswer::No => match overlap.to_screen_int_rect() {
      Some(rect) => SegmentClip::Partial(rect),
      None => SegmentClip::Outside,
    },
  })
}

#[doc = "The fixed grid covers at most 65,534 pixels. Eight bisections reduce every interval below 511 pixels."]
fn rasterize_segments(
  line: [HairPoint; 2],
  clip: Option<ScreenIntRect>,
  blitter: &mut dyn CoverageBlitter,
) -> Result<(), Never> {
  let Ok(limit) = fdot6::from_i32(511);
  let mut waiting = FixedVec::<[HairPoint; 2], 9>::new();
  waiting.push(line);

  for _ in 0..511 {
    let [start, end] = match waiting.pop() {
      Some(segment) => segment,
      None => break,
    };
    let x_distance = end.x.wrapping_sub(start.x).abs();
    let y_distance = end.y.wrapping_sub(start.y).abs();

    match (x_distance > limit || y_distance > limit).then_some(()) {
      Some(()) => {
        let mid = HairPoint {
          x: start.x.wrapping_shr(1).wrapping_add(end.x.wrapping_shr(1)),
          y: start.y.wrapping_shr(1).wrapping_add(end.y.wrapping_shr(1)),
        };
        waiting.push([mid, end]);
        waiting.push([start, mid]);
      }
      None => {
        let Ok(()) = rasterize_line([start, end], clip, blitter);
      }
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum SpanAxis {
  Horizontal,
  Vertical,
}

#[derive(Clone, Copy)]
enum SpanStyle {
  Orthogonal,
  Angled,
}

struct HairSpan {
  axis: SpanAxis,
  style: SpanStyle,
  from: i32,
  to: i32,
  cross_position: FDot16,
  slope: FDot16,
  leading_alpha: FDot6,
  trailing_alpha: FDot6,
  last_position: FDot6,
}

impl HairSpan {
  fn new([start, end]: [HairPoint; 2]) -> Result<Option<Self>, Never> {
    let dx = end.x.wrapping_sub(start.x).abs();
    let dy = end.y.wrapping_sub(start.y).abs();
    let (axis, mut initial, mut final_point) = match (dx > dy).then_some(()) {
      Some(()) => (SpanAxis::Horizontal, [start.x, start.y], [end.x, end.y]),
      None => (SpanAxis::Vertical, [start.y, start.x], [end.y, end.x]),
    };
    let [first, _] = initial;
    let [last, _] = final_point;

    match (first > last).then_some(()) {
      Some(()) => core::mem::swap(&mut initial, &mut final_point),
      None => {}
    }

    let [begin, near] = initial;
    let [end_2, far] = final_point;
    let length = end_2.wrapping_sub(begin);

    match length {
      0 => return Ok(None),
      _ => {}
    }

    let Ok(from) = fdot6::floor(begin);
    let Ok(to) = fdot6::ceil(end_2);
    let Ok(cross) = fdot6::to_fdot16(near);
    let (slope, style) = match (near == far).then_some(()) {
      Some(()) => (0, SpanStyle::Orthogonal),
      None => {
        let Ok(quotient) = fdot16::fast_div(DeltaRatio {
          numerator: far.wrapping_sub(near),
          denominator: length,
        });

        match quotient {
          Some(value) => (value, SpanStyle::Angled),
          None => return Ok(None),
        }
      }
    };
    let cross_position = cross.wrapping_add(
      slope
        .wrapping_mul(32i32.wrapping_sub(begin & 63))
        .wrapping_add(32)
        .wrapping_shr(6),
    );
    let (leading_alpha, trailing_alpha) = match to.wrapping_sub(from) {
      1 => (length, 0),
      _ => (64i32.wrapping_sub(begin & 63), end_2 & 63),
    };

    Ok(Some(Self {
      axis,
      style,
      from,
      to,
      cross_position,
      slope,
      leading_alpha,
      trailing_alpha,
      last_position: end_2,
    }))
  }

  fn restrict(&mut self, clip: ScreenIntRect) -> Result<SegmentClip, Never> {
    let Ok(bounds) = clip.to_int_rect();
    let Ok(left) = bounds.left();
    let Ok(right) = bounds.right();
    let Ok(top) = bounds.top();
    let Ok(bottom) = bounds.bottom();
    let ([low, high], [outer_low, outer_high]) = match self.axis {
      SpanAxis::Horizontal => ([left, right], [top, bottom]),
      SpanAxis::Vertical => ([top, bottom], [left, right]),
    };

    match (self.from >= high || self.to <= low).then_some(()) {
      Some(()) => return Ok(SegmentClip::Outside),
      None => {}
    }

    match (self.from < low).then_some(()) {
      Some(()) => {
        self.cross_position = self
          .cross_position
          .wrapping_add(self.slope.wrapping_mul(low.wrapping_sub(self.from)));
        self.from = low;
        self.leading_alpha = 64;

        match self.to.wrapping_sub(self.from) {
          1 => {
            self.leading_alpha = (self.last_position.wrapping_sub(1) & 63).wrapping_add(1);
            self.trailing_alpha = 0;
          }
          _ => {}
        }
      }
      None => {}
    }

    match (self.to > high).then_some(()) {
      Some(()) => {
        self.to = high;
        self.trailing_alpha = 0;
      }
      None => {}
    }

    match (self.from < self.to).then_some(()) {
      Some(()) => {}
      None => return Ok(SegmentClip::Outside),
    }

    let Ok([first, last]) = self.cross_bounds();

    Ok(
      match (
        first >= outer_high || last <= outer_low,
        outer_low <= first && outer_high >= last,
      ) {
        (true, _) => SegmentClip::Outside,
        (false, true) => SegmentClip::Inside,
        (false, false) => SegmentClip::Partial(clip),
      },
    )
  }

  fn cross_bounds(&self) -> Result<[i32; 2], Never> {
    let first = self.cross_position;
    let last = first.wrapping_add(
      self
        .to
        .wrapping_sub(self.from)
        .wrapping_sub(1)
        .wrapping_mul(self.slope),
    );
    let [near, far] = match (self.slope >= 0).then_some(()) {
      Some(()) => [first, last],
      None => [last, first],
    };
    let Ok(low) = fdot16::floor_to_i32(near.wrapping_sub(fdot16::HALF));
    let Ok(high) = fdot16::ceil_to_i32(far.wrapping_add(fdot16::HALF));

    Ok([low.wrapping_sub(1), high.wrapping_add(1)])
  }

  fn paint(&self, blitter: &mut dyn CoverageBlitter) -> Result<(), Never> {
    let (start, end) = match (u32::try_from(self.from), u32::try_from(self.to)) {
      (Ok(first), Ok(last)) => (first, last),
      _ => return Ok(()),
    };
    let Ok(mut sample) = self.paint_run(
      start..start.saturating_add(1),
      self.cross_position,
      CapCoverage::Partial(self.leading_alpha),
      blitter,
    );
    let first_full = start.saturating_add(1);
    let end_full = end.saturating_sub(u32::from(self.trailing_alpha > 0));

    match (first_full < end_full).then_some(()) {
      Some(()) => {
        let Ok(position) =
          self.paint_run(first_full..end_full, sample, CapCoverage::Full, blitter);
        sample = position;
      }
      None => {}
    }

    match (self.trailing_alpha > 0).then_some(()) {
      Some(()) => {
        let Ok(_) = self.paint_run(
          end_full..end,
          sample,
          CapCoverage::Partial(self.trailing_alpha),
          blitter,
        );
      }
      None => {}
    }

    Ok(())
  }

  fn paint_run(
    &self,
    pixels: core::ops::Range<u32>,
    sample: FDot16,
    contribution: CapCoverage,
    blitter: &mut dyn CoverageBlitter,
  ) -> Result<FDot16, Never> {
    let mut position = sample.wrapping_add(fdot16::HALF);

    match self.style {
      SpanStyle::Orthogonal => {
        let length = match LengthU32::new(pixels.end.wrapping_sub(pixels.start)) {
          Some(count) => count,
          None => return Ok(sample),
        };
        position = position.max(0);
        let Ok(cross) = wrapped::<_, u32>(position.wrapping_shr(16));
        let Ok(weights) = contribution.weights(position);
        let Ok(()) = paint_parallel(self.axis, [pixels.start, cross], length, weights, blitter);
      }
      SpanStyle::Angled => {
        for pixel in pixels {
          position = position.max(0);
          let Ok(cross) = wrapped::<_, u32>(position.wrapping_shr(16));
          let prior = cross.saturating_sub(1);
          let Ok(weights) = contribution.weights(position);
          let Ok(()) = match self.axis {
            SpanAxis::Horizontal => {
              blitter.blit_anti_v2(PixelPosition { x: pixel, y: prior }, weights)
            }
            SpanAxis::Vertical => {
              blitter.blit_anti_h2(PixelPosition { x: prior, y: pixel }, weights)
            }
          };
          position = position.wrapping_add(self.slope);
        }
      }
    }

    Ok(position.wrapping_sub(fdot16::HALF))
  }
}

enum CapCoverage {
  Full,
  Partial(FDot6),
}

impl CapCoverage {
  fn weights(&self, position: FDot16) -> Result<[AlphaU8; 2], Never> {
    let Ok(last) = wrapped::<_, u8>(position.wrapping_shr(8) & 255);
    let first = 255u8.wrapping_sub(last);

    Ok(match *self {
      CapCoverage::Full => [first, last],
      CapCoverage::Partial(scale) => {
        let Ok(before) = fdot6::small_scale(first, scale);
        let Ok(after) = fdot6::small_scale(last, scale);

        [before, after]
      }
    })
  }
}

fn paint_parallel(
  axis: SpanAxis,
  [pixel, cross]: [u32; 2],
  count: LengthU32,
  [before, after]: [AlphaU8; 2],
  blitter: &mut dyn CoverageBlitter,
) -> Result<(), Never> {
  match (axis, after) {
    (_, 0) => {}
    (SpanAxis::Horizontal, alpha) => {
      let Ok(()) = call_hline_blitter(pixel, Some(cross), count, alpha, blitter);
    }
    (SpanAxis::Vertical, alpha) => {
      let Ok(()) = blitter.blit_v(PixelPosition { x: cross, y: pixel }, count, alpha);
    }
  }

  match (axis, before) {
    (_, 0) => {}
    (SpanAxis::Horizontal, alpha) => {
      let Ok(()) = call_hline_blitter(pixel, cross.checked_sub(1), count, alpha, blitter);
    }
    (SpanAxis::Vertical, alpha) => {
      let Ok(()) = blitter.blit_v(
        PixelPosition {
          x: cross.saturating_sub(1),
          y: pixel,
        },
        count,
        alpha,
      );
    }
  }

  Ok(())
}

fn rasterize_line(
  points: [HairPoint; 2],
  clip: Option<ScreenIntRect>,
  blitter: &mut dyn CoverageBlitter,
) -> Result<(), Never> {
  let Ok(result) = HairSpan::new(points);
  let mut span = match result {
    Some(line) => line,
    None => return Ok(()),
  };
  let Ok(coverage) = match clip {
    Some(region) => span.restrict(region),
    None => Ok(SegmentClip::Inside),
  };

  match coverage {
    SegmentClip::Outside => Ok(()),
    SegmentClip::Inside => span.paint(blitter),
    SegmentClip::Partial(region) => span.paint(&mut RectClipBlitter {
      blitter,
      clip: region,
    }),
  }
}

struct RectClipBlitter<'a> {
  blitter: &'a mut dyn CoverageBlitter,
  clip: ScreenIntRect,
}

impl CoverageBlitter for RectClipBlitter<'_> {
  fn blit_anti_h(
    &mut self,
    at: PixelPosition,
    alpha: &mut [AlphaU8],
    runs: &mut [AlphaRun],
  ) -> Result<(), Never> {
    let Ok(top) = self.clip.top();
    let Ok(bottom) = self.clip.bottom();
    let Ok(left) = self.clip.left();
    let Ok(right) = self.clip.right();

    match (at.y < top || at.y >= bottom || at.x >= right).then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let Ok(width) = compute_anti_width(runs);
    let end = match width.and_then(|count| at.x.checked_add(count)) {
      Some(column) => column,
      None => return Ok(()),
    };
    let start = at.x.max(left);
    let clipped_end = end.min(right);

    match (start < clipped_end).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let Ok(()) = clip_row(
      RunClip {
        left: start.saturating_sub(at.x),
        width: clipped_end.saturating_sub(start),
        end,
        clipped_end,
      },
      PixelPosition { x: start, y: at.y },
      alpha,
      runs,
      self.blitter,
    );

    Ok(())
  }

  fn blit_v(
    &mut self,
    at: PixelPosition,
    height: LengthU32,
    alpha: AlphaU8,
  ) -> Result<(), Never> {
    let Ok(left) = self.clip.left();
    let Ok(right) = self.clip.right();

    match (left..right).contains(&at.x).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let Ok(top) = self.clip.top();
    let Ok(bottom) = self.clip.bottom();
    let y = at.y.max(top);
    let end = at.y.wrapping_add(height.get()).min(bottom);

    match LengthU32::new(end.saturating_sub(y)) {
      Some(count) => self
        .blitter
        .blit_v(PixelPosition { x: at.x, y }, count, alpha),
      None => Ok(()),
    }
  }

  fn blit_anti_h2(&mut self, at: PixelPosition, mut alpha: [AlphaU8; 2]) -> Result<(), Never> {
    self.blit_anti_h(
      at,
      &mut alpha,
      &mut [NonZeroU16::new(1), NonZeroU16::new(1), None],
    )
  }

  fn blit_anti_v2(
    &mut self,
    at: PixelPosition,
    [top, bottom]: [AlphaU8; 2],
  ) -> Result<(), Never> {
    let Ok(()) = self.blit_anti_h(at, &mut [top], &mut [NonZeroU16::new(1), None]);
    let next = PixelPosition {
      x: at.x,
      y: at.y.wrapping_add(1),
    };

    self.blit_anti_h(next, &mut [bottom], &mut [NonZeroU16::new(1), None])
  }
}

struct RunClip {
  left: u32,
  width: u32,
  end: u32,
  clipped_end: u32,
}

fn clip_row(
  clip: RunClip,
  at: PixelPosition,
  alpha: &mut [AlphaU8],
  runs: &mut [AlphaRun],
  blitter: &mut dyn CoverageBlitter,
) -> Result<(), Never> {
  match clip.left {
    0 => {}
    skip => {
      let Ok(()) = AlphaRuns::break_at(alpha, runs, skip);
    }
  }

  let Ok(offset) = index(clip.left);
  let (coverage, remaining) = match (alpha.get_mut(offset..), runs.get_mut(offset..)) {
    (Some(coverage), Some(remaining)) => (coverage, remaining),
    _ => return Ok(()),
  };

  match (clip.clipped_end < clip.end).then_some(()) {
    Some(()) => {
      let Ok(()) = AlphaRuns::break_at(coverage, remaining, clip.width);
      let Ok(terminator) = index(clip.width);

      match remaining.get_mut(terminator) {
        Some(end) => *end = None,
        None => return Ok(()),
      }
    }
    None => {}
  }

  let Ok(width) = compute_anti_width(remaining);

  match (width == Some(clip.width)).then_some(()) {
    Some(()) => blitter.blit_anti_h(at, coverage, remaining),
    None => Ok(()),
  }
}

fn compute_anti_width(runs: &[AlphaRun]) -> Result<Option<u32>, Never> {
  let mut width = 0u32;

  for _ in runs {
    let Ok(position) = index(width);

    match runs.get(position) {
      Some(Some(count)) => width = width.saturating_add(u32::from(count.get())),
      Some(None) => return Ok(Some(width)),
      None => return Ok(None),
    }
  }

  Ok(None)
}

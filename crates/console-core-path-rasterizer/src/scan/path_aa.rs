use core::cmp::Ordering;
use core::num::NonZeroU16;
use core::ops::Range;

use console_core_never::Never;
use console_core_number_conversion::fitted;

use super::path::Scan;
use crate::alpha_runs::{AlphaRuns, Coverage, RunOffset};
use crate::blitter::{Blitter, CoverageBlitter, SpanBlitter};
use crate::color::AlphaU8;
use crate::geom::{IntRectExt, PixelPosition, ScreenIntRect};
use crate::{Emptiness, FillRule, IntRect, LengthU32, Path, FloatRectangle};

const SHIFT: u32 = 2;
const SCALE: u32 = 1 << SHIFT;
const MASK: u32 = SCALE - 1;
const MAX_CLIP_COORD: u32 = 32_767;

pub fn fill_path(
  path: &Path,
  fill_rule: FillRule,
  clip: &ScreenIntRect,
  blitter: &mut dyn Blitter,
) -> Result<(), Never> {
  let Ok(bounds) = path.bounds();
  let Ok(left) = bounds.left();
  let Ok(top) = bounds.top();
  let Ok(right) = bounds.right();
  let Ok(bottom) = bounds.bottom();
  let Ok(rect) = FloatRectangle::from_ltrb(left.floor(), top.floor(), right.ceil(), bottom.ceil());
  let bounds_2 = match rect.and_then(|rect_2| {
    let Ok(rounded) = rect_2.round_out();
    rounded
  }) {
    Some(bounds_2) => bounds_2,
    None => return Ok(()),
  };
  let Ok(clip_bounds) = clip.to_int_rect();
  let Ok(intersection) = bounds_2.intersect(&clip_bounds);
  let clipped = match intersection {
    Some(clipped) => clipped,
    None => return Ok(()),
  };

  let Ok(row_coordinates) = clip_limits(clipped);
  let narrow =
    (i32::from(i16::MIN).wrapping_shr(SHIFT))..=(i32::from(i16::MAX).wrapping_shr(SHIFT));

  match row_coordinates
    .iter()
    .all(|coordinate| narrow.contains(coordinate))
    .then_some(())
  {
    Some(()) => {}
    None => return super::path::fill_path(path, fill_rule, clip, blitter),
  }

  let Ok(clip_right) = clip.right();
  let Ok(clip_bottom) = clip.bottom();

  match (clip_right > MAX_CLIP_COORD || clip_bottom > MAX_CLIP_COORD).then_some(()) {
    Some(()) => return Ok(()),
    None => {}
  }

  let Ok(super_blitter) = SuperBlitter::new(&bounds_2, clip, blitter);
  let mut sampling = match super_blitter {
    Some(sampling) => sampling,
    None => return Ok(()),
  };
  let Ok(shift) = fitted::<u32, i32>(SHIFT);

  super::path::fill_path_impl(
    path,
    fill_rule,
    clip,
    Scan {
      bounds: bounds_2,
      shift,
    },
    &mut sampling,
  )
}

fn clip_limits(rect: IntRect) -> Result<[i32; 4], Never> {
  let Ok(left) = rect.left();
  let Ok(top) = rect.top();
  let Ok(right) = rect.right();
  let Ok(bottom) = rect.bottom();

  Ok([left, top, right, bottom])
}

struct SuperBlitter<'a> {
  real: &'a mut dyn CoverageBlitter,
  row: Option<u32>,
  sample: Option<u32>,
  left: u32,
  sample_left: u32,
  runs: AlphaRuns,
  offset: RunOffset,
}

impl<'a> SuperBlitter<'a> {
  fn new(
    bounds: &IntRect,
    clip: &ScreenIntRect,
    real: &'a mut dyn CoverageBlitter,
  ) -> Result<Option<Self>, Never> {
    let Ok(clip_bounds) = clip.to_int_rect();
    let Ok(overlap) = bounds.intersect(&clip_bounds);
    let intersection = match overlap.and_then(|rect| rect.to_screen_int_rect()) {
      Some(intersection) => intersection,
      None => return Ok(None),
    };
    let Ok(width) = intersection.width();
    let width_2 = match match u16::try_from(width) {
      ::core::result::Result::Ok(value) => ::core::option::Option::Some(value),
      ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
    }
    .and_then(NonZeroU16::new)
    {
      Some(width_2) => width_2,
      None => return Ok(None),
    };
    let Ok(left) = intersection.left();
    let Ok(runs) = AlphaRuns::new(width_2);

    Ok(Some(Self {
      real,
      row: None,
      sample: None,
      left,
      sample_left: left.wrapping_shl(SHIFT),
      runs,
      offset: RunOffset::default(),
    }))
  }

  fn flush(&mut self) -> Result<(), Never> {
    let row = match self.row.take() {
      Some(row) => row,
      None => return Ok(()),
    };
    let Ok(empty) = self.runs.is_empty();

    match empty {
      Emptiness::Empty => {}
      Emptiness::NonEmpty => {
        let Ok(()) = self.real.blit_anti_h(
          PixelPosition {
            x: self.left,
            y: row,
          },
          &mut self.runs.alpha,
          &mut self.runs.runs,
        );
        let Ok(()) = self.runs.reset();

        self.offset = RunOffset::default();
      }
    }

    Ok(())
  }
}

impl Drop for SuperBlitter<'_> {
  fn drop(&mut self) {
    let Ok(()) = self.flush();
  }
}

impl SpanBlitter for SuperBlitter<'_> {
  fn blit_h(&mut self, at: PixelPosition, width: LengthU32) -> Result<(), Never> {
    let row = at.y.wrapping_shr(SHIFT);

    match self.sample.map(|sample| at.y.cmp(&sample)) {
      Some(Ordering::Less) => return Ok(()),
      Some(Ordering::Equal) => {}
      None | Some(Ordering::Greater) => {
        self.offset = RunOffset::default();
        self.sample = Some(at.y);
      }
    }

    match (self.row == Some(row)).then_some(()) {
      Some(()) => {}
      None => {
        let Ok(()) = self.flush();
        self.row = Some(row);
      }
    }

    let (start, extent) = match at.x.checked_sub(self.sample_left) {
      Some(start) => (start, width.get()),
      None => (0, at.x.wrapping_add(width.get())),
    };
    let stop = match start.checked_add(extent) {
      Some(stop) => stop,
      None => return Ok(()),
    };
    let Ok(coverage) = coverage_of(start..stop, at.y);
    let Ok(offset) = self
      .runs
      .add(start.wrapping_shr(SHIFT), self.offset, coverage);

    self.offset = offset;

    Ok(())
  }
}

fn coverage_of(samples: Range<u32>, sample_y: u32) -> Result<Coverage, Never> {
  let start = samples.start.wrapping_shr(SHIFT);
  let stop = samples.end.wrapping_shr(SHIFT);
  let first = samples.start & MASK;
  let last = samples.end & MASK;
  let (from, interior, to) = match (stop.cmp(&start), first) {
    (Ordering::Greater, 0) => (0, stop.saturating_sub(start), last),
    (Ordering::Greater, _) => (
      SCALE.saturating_sub(first),
      stop.saturating_sub(start).saturating_sub(1),
      last,
    ),
    (Ordering::Less | Ordering::Equal, _) => (last.saturating_sub(first), 0, 0),
  };
  let Ok(first_alpha) = partial_alpha(from);
  let Ok(last_alpha) = partial_alpha(to);
  let final_sample = (sample_y & MASK).saturating_add(1).wrapping_shr(SHIFT);
  let full = 1u32
    .wrapping_shl(8u32.saturating_sub(SHIFT))
    .saturating_sub(final_sample);
  let Ok(full_alpha) = fitted::<u32, AlphaU8>(full);

  Ok(Coverage {
    first: first_alpha,
    interior,
    full_alpha,
    last: last_alpha,
  })
}

fn partial_alpha(coverage: u32) -> Result<AlphaU8, Never> {
  let shift = 8u32.saturating_sub(2u32.saturating_mul(SHIFT));

  fitted::<u32, AlphaU8>(coverage.wrapping_shl(shift))
}

use console_core_never::Never;
use console_core_number_conversion::{nearest_f32_from_i64, toward_zero_i32, wrapped};
use core::num::Wrapping;
use kurbo::bezier;

use crate::fixed_point::{DeltaRatio, FDot6, FDot16, fdot6, fdot16};
use crate::{IsVerticalAnswer, Point, UpdateAnswer};

const MAX_COEFF_BITS: u32 = 6;

#[derive(Clone, Debug)]
pub enum Edge {
  Line(LineEdge),
  Quadratic(QuadraticEdge),
  Cubic(CubicEdge),
}

impl Edge {
  pub fn as_line(&self) -> Result<&LineEdge, Never> {
    Ok(match self {
      Self::Line(line) => line,
      Self::Quadratic(quad) => &quad.line,
      Self::Cubic(cubic) => &cubic.line,
    })
  }

  pub fn as_line_mut(&mut self) -> Result<&mut LineEdge, Never> {
    Ok(match self {
      Self::Line(line) => line,
      Self::Quadratic(quad) => &mut quad.line,
      Self::Cubic(cubic) => &mut cubic.line,
    })
  }
}

impl core::ops::Deref for Edge {
  type Target = LineEdge;

  fn deref(&self) -> &Self::Target {
    let Ok(line) = self.as_line();

    line
  }
}

impl core::ops::DerefMut for Edge {
  fn deref_mut(&mut self) -> &mut Self::Target {
    let Ok(line) = self.as_line_mut();

    line
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EdgeWinding {
  #[default]
  NoCrossing,
  Forward,
  Reverse,
}

impl EdgeWinding {
  pub fn delta(self) -> Result<i32, Never> {
    Ok(match self {
      Self::NoCrossing => 0,
      Self::Forward => 1,
      Self::Reverse => -1,
    })
  }
}

#[derive(Clone, Default, Debug)]
pub struct LineEdge {
  pub prev: Option<u32>,
  pub next: Option<u32>,
  pub x: FDot16,
  pub dx: FDot16,
  pub first_y: i32,
  pub last_y: i32,
  pub winding: EdgeWinding,
}

impl LineEdge {
  pub fn new(start: Point, end: Point, shift: i32) -> Result<Option<Self>, Never> {
    let Ok(bits) = wrapped::<i32, u32>(shift);
    let Ok(scale) = grid_scale(bits);
    let Ok(a) = scale.point(start);
    let Ok(b) = scale.point(end);
    let ([upper, lower], winding) = match (a.y > b.y).then_some(()) {
      Some(()) => ([b, a], EdgeWinding::Reverse),
      None => ([a, b], EdgeWinding::Forward),
    };
    let Ok(formed) = LineSpan::new([upper, lower]);

    Ok(formed.map(|span| Self {
      x: span.x,
      dx: span.dx,
      first_y: span.first_row,
      last_y: span.last_row,
      winding,
      ..Self::default()
    }))
  }

  pub fn is_vertical(&self) -> Result<IsVerticalAnswer, Never> {
    Ok(match self.dx {
      0 => IsVerticalAnswer::Yes,
      _ => IsVerticalAnswer::No,
    })
  }

  fn update(&mut self, points: [FixedPoint<16>; 2]) -> Result<UpdateAnswer, Never> {
    let initial = points.map(|point| FixedPoint {
      x: point.x.wrapping_shr(10),
      y: point.y.wrapping_shr(10),
    });
    let Ok(line) = LineSpan::new(initial);

    match line {
      Some(span) => {
        self.x = span.x;
        self.dx = span.dx;
        self.first_y = span.first_row;
        self.last_y = span.last_row;

        Ok(UpdateAnswer::Yes)
      }
      None => Ok(UpdateAnswer::No),
    }
  }
}

#[derive(Clone, Copy)]
struct FixedPoint<const FRACTION_BITS: u8> {
  x: i32,
  y: i32,
}

struct LineSpan {
  x: FDot16,
  dx: FDot16,
  first_row: i32,
  last_row: i32,
}

impl LineSpan {
  fn new([start, end]: [FixedPoint<6>; 2]) -> Result<Option<Self>, Never> {
    let Ok(top) = fdot6::round(start.y);
    let Ok(bottom) = fdot6::round(end.y);

    match (top < bottom).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    let Ok(coefficient) = fdot16::fast_div(DeltaRatio {
      numerator: end.x.wrapping_sub(start.x),
      denominator: end.y.wrapping_sub(start.y),
    });
    let dx = match coefficient {
      Some(dx) => dx,
      None => return Ok(None),
    };
    let into_first_row = top.wrapping_shl(6).wrapping_add(32).wrapping_sub(start.y);
    let Ok(place) = fdot16::mul([dx, into_first_row]);
    let Ok(x) = fdot6::to_fdot16(start.x.wrapping_add(place));

    Ok(Some(Self {
      x,
      dx,
      first_row: top,
      last_row: bottom.wrapping_sub(1),
    }))
  }
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the float stores the number of fixed-point sampling units per raster pixel"
  )
)]
struct GridScale(f32);

impl GridScale {
  fn point(&self, point: Point) -> Result<FixedPoint<6>, Never> {
    let Ok(x) = toward_zero_i32(f64::from(point.x * self.0));
    let Ok(y) = toward_zero_i32(f64::from(point.y * self.0));

    Ok(FixedPoint { x, y })
  }
}

fn grid_scale(bits: u32) -> Result<GridScale, Never> {
  let Ok(scale) = nearest_f32_from_i64(i64::from(1i32.wrapping_shl(bits.wrapping_add(6))));

  Ok(GridScale(scale))
}

#[derive(Clone, Debug)]
pub struct QuadraticEdge {
  pub line: LineEdge,
  pub remaining: u8,
  step_bits: u32,
  horizontal: QuadraticAxis,
  vertical: QuadraticAxis,
}

#[derive(Clone, Debug)]
struct QuadraticAxis {
  at: FDot16,
  delta: FDot16,
  change: FDot16,
  end: FDot16,
}

impl QuadraticAxis {
  fn new([start, control, end]: [FDot6; 3], bits: u32) -> Result<Self, Never> {
    let [bend, slope, _] = bezier::quadratic_coefficients([start, control, end].map(Wrapping));
    let slope = slope.0.wrapping_shl(9);
    let bend = bend.0.wrapping_shl(9);

    Ok(Self {
      at: start.wrapping_shl(10),
      delta: slope.wrapping_add(bend.wrapping_shr(bits)),
      change: bend.wrapping_shr(bits.saturating_sub(1)),
      end: end.wrapping_shl(10),
    })
  }

  fn advance(&mut self, remaining: u8, shift: u32) -> Result<(), Never> {
    match remaining {
      0 => self.at = self.end,
      _ => {
        self.at = self.at.wrapping_add(self.delta.wrapping_shr(shift));
        self.delta = self.delta.wrapping_add(self.change);
      }
    }

    Ok(())
  }
}

impl QuadraticEdge {
  pub fn new(points: &[Point; 3], shift: i32) -> Result<Option<Self>, Never> {
    let [first, control, last] = *points;
    let Ok(sampling) = wrapped::<i32, u32>(shift);
    let Ok(scale) = grid_scale(sampling);
    let Ok(a) = scale.point(first);
    let Ok(b) = scale.point(control);
    let Ok(c) = scale.point(last);
    let ([start, middle, end], winding) = match (a.y > c.y).then_some(()) {
      Some(()) => ([c, b, a], EdgeWinding::Reverse),
      None => ([a, b, c], EdgeWinding::Forward),
    };

    let Ok(top) = fdot6::round(start.y);
    let Ok(bottom) = fdot6::round(end.y);

    match (top < bottom, start.y <= middle.y && middle.y <= end.y) {
      (true, true) => {}
      (false, _) | (_, false) => return Ok(None),
    }

    let dx = middle
      .x
      .wrapping_shl(1)
      .wrapping_sub(start.x)
      .wrapping_sub(end.x)
      .wrapping_shr(2);
    let dy = middle
      .y
      .wrapping_shl(1)
      .wrapping_sub(start.y)
      .wrapping_sub(end.y)
      .wrapping_shr(2);
    let Ok(suggested) = subdivision_bits([dx, dy], sampling);
    let bits = suggested.clamp(1, MAX_COEFF_BITS);
    let Ok(horizontal) = QuadraticAxis::new([start.x, middle.x, end.x], bits);
    let Ok(vertical) = QuadraticAxis::new([start.y, middle.y, end.y], bits);
    let mut edge = Self {
      line: LineEdge {
        winding,
        ..LineEdge::default()
      },
      remaining: 1u8.wrapping_shl(bits),
      step_bits: bits.saturating_sub(1),
      horizontal,
      vertical,
    };
    let Ok(updated) = edge.update();

    Ok(match updated {
      UpdateAnswer::Yes => Some(edge),
      UpdateAnswer::No => None,
    })
  }

  pub fn update(&mut self) -> Result<UpdateAnswer, Never> {
    for remaining in (0..self.remaining).rev() {
      let start = FixedPoint {
        x: self.horizontal.at,
        y: self.vertical.at,
      };
      let Ok(()) = self.horizontal.advance(remaining, self.step_bits);
      let Ok(()) = self.vertical.advance(remaining, self.step_bits);
      let end = FixedPoint {
        x: self.horizontal.at,
        y: self.vertical.at,
      };
      self.remaining = remaining;

      let Ok(updated) = self.line.update([start, end]);

      match updated {
        UpdateAnswer::Yes => return Ok(UpdateAnswer::Yes),
        UpdateAnswer::No => {}
      }
    }

    Ok(UpdateAnswer::No)
  }
}

#[derive(Clone, Debug)]
pub struct CubicEdge {
  pub line: LineEdge,
  pub remaining: u8,
  step_bits: CubicBits,
  horizontal: CubicAxis,
  vertical: CubicAxis,
}

#[derive(Clone, Copy, Debug)]
struct CubicBits {
  subdivision: u32,
  coefficient_shift: u32,
  step_shift: u32,
}

#[derive(Clone, Debug)]
struct CubicAxis {
  at: FDot16,
  delta: FDot16,
  change: FDot16,
  third: FDot16,
  end: FDot16,
}

impl CubicAxis {
  fn new([start, first, second, end]: [FDot6; 4], bits: CubicBits) -> Result<Self, Never> {
    let [inflection, bend, slope, _] =
      bezier::cubic_coefficients([start, first, second, end].map(Wrapping), Wrapping(3));
    let slope = slope.0.wrapping_shl(bits.coefficient_shift);
    let bend = bend.0.wrapping_shl(bits.coefficient_shift);
    let inflection = inflection.0.wrapping_shl(bits.coefficient_shift);
    let third = inflection
      .wrapping_mul(3)
      .wrapping_shr(bits.subdivision.saturating_sub(1));

    Ok(Self {
      at: start.wrapping_shl(10),
      delta: slope
        .wrapping_add(bend.wrapping_shr(bits.subdivision))
        .wrapping_add(inflection.wrapping_shr(bits.subdivision.wrapping_mul(2))),
      change: bend.wrapping_mul(2).wrapping_add(third),
      third,
      end: end.wrapping_shl(10),
    })
  }

  fn advance(&mut self, remaining: u8, bits: CubicBits) -> Result<(), Never> {
    match remaining {
      0 => self.at = self.end,
      _ => {
        self.at = self
          .at
          .wrapping_add(self.delta.wrapping_shr(bits.step_shift));
        self.delta = self
          .delta
          .wrapping_add(self.change.wrapping_shr(bits.subdivision));
        self.change = self.change.wrapping_add(self.third);
      }
    }

    Ok(())
  }
}

impl CubicEdge {
  pub fn new(points: &[Point; 4], shift: i32) -> Result<Option<Self>, Never> {
    let [first, control, control_2, last] = *points;
    let Ok(sampling) = wrapped::<i32, u32>(shift);
    let Ok(scale) = grid_scale(sampling);
    let Ok(a) = scale.point(first);
    let Ok(b) = scale.point(control);
    let Ok(c) = scale.point(control_2);
    let Ok(d) = scale.point(last);
    let ([start, control_a, control_b, end], winding) = match (a.y > d.y).then_some(()) {
      Some(()) => ([d, c, b, a], EdgeWinding::Reverse),
      None => ([a, b, c, d], EdgeWinding::Forward),
    };
    let Ok(top) = fdot6::round(start.y);
    let Ok(bottom) = fdot6::round(end.y);

    match (top < bottom).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    let xs = [start.x, control_a.x, control_b.x, end.x];
    let ys = [start.y, control_a.y, control_b.y, end.y];
    let Ok(dx) = cubic_delta(xs);
    let Ok(dy) = cubic_delta(ys);
    let Ok(adaptive) = subdivision_bits([dx, dy], 2);
    let step = adaptive.saturating_add(1).min(MAX_COEFF_BITS);
    let coefficient_shift = 6u32.max(10u32.saturating_sub(step));
    let bits = CubicBits {
      subdivision: step,
      coefficient_shift,
      step_shift: step.saturating_add(coefficient_shift).saturating_sub(10),
    };
    let Ok(horizontal) = CubicAxis::new(xs, bits);
    let Ok(vertical) = CubicAxis::new(ys, bits);
    let mut edge = Self {
      line: LineEdge {
        winding,
        ..LineEdge::default()
      },
      remaining: 1u8.wrapping_shl(step),
      horizontal,
      vertical,
      step_bits: bits,
    };
    let Ok(updated) = edge.update();

    Ok(match updated {
      UpdateAnswer::Yes => Some(edge),
      UpdateAnswer::No => None,
    })
  }

  pub fn update(&mut self) -> Result<UpdateAnswer, Never> {
    for remaining in (0..self.remaining).rev() {
      let start = FixedPoint {
        x: self.horizontal.at,
        y: self.vertical.at,
      };
      let Ok(()) = self.horizontal.advance(remaining, self.step_bits);
      let Ok(()) = self.vertical.advance(remaining, self.step_bits);
      self.vertical.at = self.vertical.at.max(start.y);
      let end = FixedPoint {
        x: self.horizontal.at,
        y: self.vertical.at,
      };
      self.remaining = remaining;

      let Ok(updated) = self.line.update([start, end]);

      match updated {
        UpdateAnswer::Yes => return Ok(UpdateAnswer::Yes),
        UpdateAnswer::No => {}
      }
    }

    Ok(UpdateAnswer::No)
  }
}

fn subdivision_bits([dx, dy]: [FDot6; 2], samples: u32) -> Result<u32, Never> {
  let x = dx.wrapping_abs();
  let y = dy.wrapping_abs();
  let distance = x.max(y).wrapping_add(x.min(y).wrapping_shr(1));
  let adjusted = distance
    .wrapping_add(1i32.wrapping_shl(samples.wrapping_add(2)))
    .wrapping_shr(samples.wrapping_add(3));

  Ok(
    i32::BITS
      .saturating_sub(adjusted.leading_zeros())
      .wrapping_shr(1),
  )
}

fn cubic_delta([a, b, c, d]: [FDot6; 4]) -> Result<FDot6, Never> {
  let near_first = a
    .wrapping_mul(8)
    .wrapping_sub(b.wrapping_mul(15))
    .wrapping_add(c.wrapping_mul(6))
    .wrapping_add(d)
    .wrapping_mul(19)
    .wrapping_shr(9);
  let near_second = a
    .wrapping_add(b.wrapping_mul(6))
    .wrapping_sub(c.wrapping_mul(15))
    .wrapping_add(d.wrapping_mul(8))
    .wrapping_mul(19)
    .wrapping_shr(9);

  Ok(near_first.wrapping_abs().max(near_second.wrapping_abs()))
}

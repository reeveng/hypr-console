use alloc::vec::Vec;

use console_core_never::Never;
use console_core_paths::{NormalizedF32, Scalar, ScalarProximity, StrictNormalized, StrictNum};

use crate::pipeline::{
  GradientBand, GradientCoefficients, GradientColor, RasterPipelineBuilder, Stage,
};
use crate::{Color, ColorSpace, IsOpaqueAnswer, PushStagesAnswer, SpreadMode, Transform};

pub const DEGENERATE_THRESHOLD: f32 = 1.0 / 32_768.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GradientStop {
  pub(crate) position: NormalizedF32,
  pub(crate) color: Color,
}

impl GradientStop {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the public stop position is a dimensionless interpolation fraction, clamped into NormalizedF32 by this constructor"
    )
  )]
  pub fn new(position: f32, color: Color) -> Result<Self, Never> {
    Ok(Self {
      position: NormalizedF32::new_clamped(position),
      color,
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Spacing {
  Uniform,
  Uneven,
}

enum FirstStop {
  Existing,
  Inserted,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Gradient {
  stops: Vec<GradientStop>,
  tile_mode: SpreadMode,
  pub(crate) transform: Transform,
  points_to_unit: Transform,
  pub(crate) opacity: IsOpaqueAnswer,
  spacing: Spacing,
}

impl Gradient {
  pub fn new(
    mut stops: Vec<GradientStop>,
    tile_mode: SpreadMode,
    transform: Transform,
    points_to_unit: Transform,
  ) -> Result<Option<Self>, Never> {
    let (first, last) = match stops.as_slice() {
      [first, .., last] => (*first, *last),
      [] | [_] => return Ok(None),
    };
    let anchored = match first.position.get().classify() {
      core::num::FpCategory::Zero => FirstStop::Existing,
      core::num::FpCategory::Nan
      | core::num::FpCategory::Infinite
      | core::num::FpCategory::Subnormal
      | core::num::FpCategory::Normal => {
        stops.insert(
          0,
          GradientStop {
            position: NormalizedF32::ZERO,
            color: first.color,
          },
        );
        FirstStop::Inserted
      }
    };

    match (last.position.get().to_bits() == 1.0_f32.to_bits()).then_some(()) {
      Some(()) => {}
      None => stops.push(GradientStop {
        position: NormalizedF32::ONE,
        color: last.color,
      }),
    }

    let to_distribute = match (anchored, stops.as_mut_slice()) {
      (FirstStop::Existing, [_, rest @ ..]) => rest,
      (FirstStop::Inserted, added) => added,
      (FirstStop::Existing, []) => return Ok(None),
    };
    let Ok(spacing) = distribute(to_distribute);
    let Ok(opacity) = opacity_of(&stops);

    Ok(Some(Self {
      stops,
      tile_mode,
      transform,
      points_to_unit,
      opacity,
      spacing,
    }))
  }

  pub fn push_stages(
    &self,
    pipeline: &mut RasterPipelineBuilder,
    colorspace: ColorSpace,
    before: &dyn Fn(&mut RasterPipelineBuilder),
    after: &dyn Fn(&mut RasterPipelineBuilder),
  ) -> Result<PushStagesAnswer, Never> {
    let Ok(()) = pipeline.push(Stage::SeedShader);
    let Ok(inverse) = self.transform.invert();
    let transform = match inverse {
      Some(transform) => transform,
      None => {
        console_core_logging::warn!(
          "failed to invert a gradient transform. Nothing will be rendered"
        );
        return Ok(PushStagesAnswer::No);
      }
    };
    let Ok(adjusted) = transform.post_concat(self.points_to_unit);
    let Ok(()) = pipeline.push_transform(adjusted);

    before(pipeline);

    let Ok(()) = match (self.tile_mode, self.spacing) {
      (SpreadMode::Reflect, _) => pipeline.push(Stage::ReflectX1),
      (SpreadMode::Repeat, _) => pipeline.push(Stage::RepeatX1),
      (SpreadMode::Pad, Spacing::Uniform) => pipeline.push(Stage::PadX1),
      (SpreadMode::Pad, Spacing::Uneven) => Ok(()),
    };

    match self.stops.as_slice() {
      [left, right] => {
        let Ok(start) = colorspace.expand_color(left.color);
        let Ok(end) = colorspace.expand_color(right.color);
        let start_color = GradientColor::from(start);
        let Ok(interpolation) = coefficients(
          start_color,
          GradientColor::from(end),
          NormalizedF32::ZERO..NormalizedF32::ONE,
        );
        pipeline.ctx.evenly_spaced_2_stop_gradient = GradientCoefficients {
          factor: interpolation.factor,
          bias: start_color,
        };

        let Ok(()) = pipeline.push(Stage::EvenlySpaced2StopGradient);
      }
      [_, _, _, ..] => {
        let Ok(bands) = gradient_bands(&self.stops, colorspace);
        pipeline.ctx.gradient = bands;

        let Ok(()) = pipeline.push(Stage::Gradient);
      }
      [] | [_] => return Ok(PushStagesAnswer::No),
    }

    match self.opacity {
      IsOpaqueAnswer::No => {
        let Ok(()) = pipeline.push(Stage::Premultiply);
      }
      IsOpaqueAnswer::Yes => {}
    }

    after(pipeline);

    Ok(PushStagesAnswer::Yes)
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the opacity is the original dimensionless multiplier passed through to each stop's color"
    )
  )]
  pub fn apply_opacity(&mut self, opacity: f32) -> Result<(), Never> {
    for stop in &mut self.stops {
      let Ok(()) = stop.color.apply_opacity(opacity);
    }

    let Ok(updated) = opacity_of(&self.stops);
    self.opacity = updated;

    Ok(())
  }
}

fn opacity_of(stops: &[GradientStop]) -> Result<IsOpaqueAnswer, Never> {
  let opaque = stops.iter().all(|stop| {
    let Ok(opacity) = stop.color.is_opaque();

    opacity == IsOpaqueAnswer::Yes
  });

  Ok(match opaque.then_some(()) {
    Some(()) => IsOpaqueAnswer::Yes,
    None => IsOpaqueAnswer::No,
  })
}

fn distribute(stops: &mut [GradientStop]) -> Result<Spacing, Never> {
  let expected = match stops.first() {
    Some(first) => first.position.get(),
    None => return Ok(Spacing::Uneven),
  };
  let (last, interior) = match stops.split_last_mut() {
    Some(split) => split,
    None => return Ok(Spacing::Uneven),
  };
  let mut previous = 0.0;
  let mut equal = true;

  for stop in interior {
    let position = stop.position.get().bound(previous, 1.0);
    equal &= expected.is_nearly_equal(position - previous) == ScalarProximity::WithinTolerance;
    stop.position = NormalizedF32::new_clamped(position);
    previous = position;
  }

  equal &= expected.is_nearly_equal(1.0 - previous) == ScalarProximity::WithinTolerance;
  last.position = NormalizedF32::ONE;

  Ok(match equal.then_some(()) {
    Some(()) => Spacing::Uniform,
    None => Spacing::Uneven,
  })
}

fn gradient_bands(
  stops: &[GradientStop],
  colorspace: ColorSpace,
) -> Result<Vec<GradientBand>, Never> {
  let Ok(selected) = deduplicated_ends(stops);
  let (first, others) = match selected.split_first() {
    Some((first, others)) => (first, others),
    None => return Ok(Vec::new()),
  };
  let Ok(color) = colorspace.expand_color(first.color);
  let mut left = GradientColor::from(color);
  let mut start = first.position;
  let mut bands = Vec::with_capacity(stops.len().saturating_add(1));
  bands.push(GradientBand {
    start: NormalizedF32::ZERO,
    coeffs: GradientCoefficients {
      factor: GradientColor::default(),
      bias: left,
    },
  });

  for stop in others {
    let Ok(color_2) = colorspace.expand_color(stop.color);
    let right = GradientColor::from(color_2);

    match (start.get() < stop.position.get()).then_some(()) {
      Some(()) => {
        let Ok(coeffs) = coefficients(left, right, start..stop.position);
        bands.push(GradientBand { start, coeffs });
      }
      None => {}
    }

    start = stop.position;
    left = right;
  }

  bands.push(GradientBand {
    start,
    coeffs: GradientCoefficients {
      factor: GradientColor::default(),
      bias: left,
    },
  });

  Ok(bands)
}

fn deduplicated_ends(stops: &[GradientStop]) -> Result<&[GradientStop], Never> {
  let head = match stops {
    [first, second, ..] => match (first.color == second.color).then_some(()) {
      Some(()) => match stops.split_first() {
        Some((_, rest)) => rest,
        None => stops,
      },
      None => stops,
    },
    [] | [_] => stops,
  };

  Ok(match head {
    [.., before, last] => match (before.color == last.color).then_some(()) {
      Some(()) => match head.split_last() {
        Some((_, rest)) => rest,
        None => head,
      },
      None => head,
    },
    [] | [_] => head,
  })
}

fn coefficients(
  GradientColor([lr, lg, lb, la]): GradientColor,
  GradientColor([rr, rg, rb, ra]): GradientColor,
  span: core::ops::Range<NormalizedF32>,
) -> Result<GradientCoefficients, Never> {
  let start = span.start.get();
  let distance = span.end.get() - start;
  let factor = [
    (rr - lr) / distance,
    (rg - lg) / distance,
    (rb - lb) / distance,
    (ra - la) / distance,
  ];
  let [r, g, b, a] = factor;
  let bias = [
    lr - r * start,
    lg - g * start,
    lb - b * start,
    la - a * start,
  ];

  Ok(GradientCoefficients {
    factor: GradientColor(factor),
    bias: GradientColor(bias),
  })
}

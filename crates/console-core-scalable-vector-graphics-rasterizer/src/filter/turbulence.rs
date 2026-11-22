//! Copyright 2020 the Resvg Authors
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{Error, ImageRefMut};
use console_core_never::Never;
use console_core_number_conversion::{
  fitted, index, nearest_f32_from_f64, toward_zero_i32, toward_zero_u8, wrapped,
};
use console_core_scalable_vector_graphics::filter::TurbulenceKind;
use console_core_scalable_vector_graphics::{ApproxZeroUlps, StitchTilesAnswer, ZeroProximity};
use console_core_size::Size;

const RANDOM_MAX: i32 = 2_147_483_647;
const INITIAL_RANDOM_MAX: u32 = 2_147_483_646;
const POSITIVE_RANDOM_MAX: i32 = 2_147_483_646;
const RANDOM_MULTIPLIER: i32 = 16_807;
const RANDOM_QUOTIENT: i32 = 127_773;
const RANDOM_REMAINDER: i32 = 2_836;
const LATTICE_ORIGIN: i32 = 4_096;
const MAX_OCTAVES: u32 = 64;

pub(super) struct Settings {
  pub offset: (f64, f64),
  pub scale: (f64, f64),
  pub base_frequency: (f64, f64),
  pub num_octaves: u32,
  pub seed: i32,
  pub stitch_tiles: StitchTilesAnswer,
  pub kind: TurbulenceKind,
}

#[doc = "Applies Perlin turbulence to the four unpremultiplied channels."]
pub(super) fn apply(settings: Settings, dest: ImageRefMut<'_>) -> Result<(), Error> {
  let size = dest.dimensions()?;

  match (size.width, size.height) {
    (0, _) | (_, 0) => return Ok(()),
    (_, _) => {}
  }

  let Ok(noise) = Noise::new(settings.seed);
  let Ok(tiles) = TileAxes::new(size, &settings);
  let mut pixels = dest.data.iter_mut();

  for y in 0..size.height {
    for x in 0..size.width {
      let destination = pixels.next().ok_or(Error::InvalidRegion)?;
      let Ok(sample) = tiles.sample(super::PixelPosition { x, y }, &settings);
      let Ok(red) = noise.channel(&noise.gradients.red, sample, &settings);
      let Ok(green) = noise.channel(&noise.gradients.green, sample, &settings);
      let Ok(blue) = noise.channel(&noise.gradients.blue, sample, &settings);
      let Ok(alpha) = noise.channel(&noise.gradients.alpha, sample, &settings);
      *destination = [red, green, blue, alpha];
    }
  }

  Ok(())
}

struct Random(i32);

impl Random {
  fn new(seed: i32) -> Result<Self, Never> {
    let present = match (seed <= 0).then_some(()) {
      Some(()) => {
        let initial = (seed.unsigned_abs().wrapping_rem(INITIAL_RANDOM_MAX)).saturating_add(1);
        let Ok(signed) = fitted::<u32, i32>(initial);
        signed
      }
      None => seed.min(POSITIVE_RANDOM_MAX),
    };

    Ok(Self(present))
  }

  fn next(&mut self) -> Result<i32, Never> {
    let low = RANDOM_MULTIPLIER.saturating_mul(self.0.wrapping_rem(RANDOM_QUOTIENT));
    let high = RANDOM_REMAINDER.saturating_mul(self.0.wrapping_div(RANDOM_QUOTIENT));
    let next = low.saturating_sub(high);
    self.0 = match (next <= 0).then_some(()) {
      Some(()) => next.saturating_add(RANDOM_MAX),
      None => next,
    };

    Ok(self.0)
  }
}

type Gradient = [[f64; 2]; 256];

struct Gradients {
  red: Gradient,
  green: Gradient,
  blue: Gradient,
  alpha: Gradient,
}

struct Noise {
  permutation: [u8; 256],
  gradients: Gradients,
}

impl Noise {
  fn new(seed: i32) -> Result<Self, Never> {
    let Ok(mut random) = Random::new(seed);
    let Ok(red) = gradient_field(&mut random);
    let Ok(green) = gradient_field(&mut random);
    let Ok(blue) = gradient_field(&mut random);
    let Ok(alpha) = gradient_field(&mut random);
    let mut permutation = [0; 256];

    for (entry, value) in permutation.iter_mut().zip(0_u8..=u8::MAX) {
      *entry = value;
    }

    for current in (1_u8..=u8::MAX).rev() {
      let Ok(random_word) = random.next();
      let Ok(other) = wrapped::<i32, u8>(random_word);
      let Ok(left) = index(u64::from(current));
      let Ok(right) = index(u64::from(other));
      let values = match (permutation.get(left), permutation.get(right)) {
        (Some(left_2), Some(right_2)) => (*left_2, *right_2),
        (None, _) | (_, None) => continue,
      };

      match permutation.get_mut(left) {
        Some(slot) => *slot = values.1,
        None => continue,
      }

      match permutation.get_mut(right) {
        Some(slot) => *slot = values.0,
        None => {}
      }
    }

    Ok(Self {
      permutation,
      gradients: Gradients {
        red,
        green,
        blue,
        alpha,
      },
    })
  }

  fn channel(
    &self,
    gradient: &Gradient,
    mut sample: TileSample,
    settings: &Settings,
  ) -> Result<u8, Never> {
    let mut sum = 0.0;
    let mut weight = 1.0;

    for _ in 0..settings.num_octaves.min(MAX_OCTAVES) {
      let Ok(value) = self.noise_at(gradient, sample);
      sum += match settings.kind {
        TurbulenceKind::FractalNoise => value / weight,
        TurbulenceKind::Turbulence => value.abs() / weight,
      };
      let Ok(()) = sample.x.double();
      let Ok(()) = sample.y.double();
      weight *= 2.0;
    }

    let brightness = match settings.kind {
      TurbulenceKind::FractalNoise => (sum * 255.0 + 255.0) / 2.0,
      TurbulenceKind::Turbulence => sum * 255.0,
    };
    let Ok(channel) = nearest_f32_from_f64(brightness);

    toward_zero_u8(f64::from(channel + 0.5))
  }

  fn permute(&self, input: u8) -> Result<u8, Never> {
    let Ok(location) = index(u64::from(input));
    Ok(match self.permutation.get(location) {
      Some(output) => *output,
      None => input,
    })
  }

  fn noise_at(&self, gradient: &Gradient, sample: TileSample) -> Result<f64, Never> {
    let Ok(x) = sample.x.lattice();
    let Ok(y) = sample.y.lattice();
    let Ok(left) = self.permute(x.first);
    let Ok(right) = self.permute(x.second);
    let Ok(top_left) = self.permute(left.wrapping_add(y.first));
    let Ok(top_right) = self.permute(right.wrapping_add(y.first));
    let Ok(bottom_left) = self.permute(left.wrapping_add(y.second));
    let Ok(bottom_right) = self.permute(right.wrapping_add(y.second));

    let [left_distance, right_distance] = x.distances;
    let [top_distance, bottom_distance] = y.distances;
    let Ok(a) = dot_gradient(gradient, top_left, [left_distance, top_distance]);
    let Ok(b) = dot_gradient(gradient, top_right, [right_distance, top_distance]);
    let Ok(c) = dot_gradient(gradient, bottom_left, [left_distance, bottom_distance]);
    let Ok(d) = dot_gradient(gradient, bottom_right, [right_distance, bottom_distance]);
    let Ok(horizontal) = Interpolation::smooth(x.distances);
    let Ok(vertical) = Interpolation::smooth(y.distances);
    let Ok(top) = horizontal.between([a, b]);
    let Ok(bottom) = horizontal.between([c, d]);

    vertical.between([top, bottom])
  }
}

fn gradient_field(random: &mut Random) -> Result<Gradient, Never> {
  let mut field = [[0.0; 2]; 256];

  for vector in &mut field {
    for component in &mut *vector {
      let Ok(word) = random.next();
      *component = f64::from(word.wrapping_rem(512).saturating_sub(256)) / 256.0;
    }

    let [x, y] = vector;
    let length = (*x * *x + *y * *y).sqrt();
    *x /= length;
    *y /= length;
  }

  Ok(field)
}

fn dot_gradient(gradient: &Gradient, point: u8, [x, y]: [f64; 2]) -> Result<f64, Never> {
  let Ok(location) = index(u64::from(point));
  let [gx, gy] = match gradient.get(location) {
    Some(vector) => *vector,
    None => [0.0, 0.0],
  };

  Ok(x * gx + y * gy)
}

struct TileAxes {
  x: TileAxis,
  y: TileAxis,
}

impl TileAxes {
  fn new(size: Size<u32>, settings: &Settings) -> Result<Self, Never> {
    let (horizontal, vertical) = settings.base_frequency;
    let Ok(x) = tile_axis(size.width, Frequency(horizontal), settings.stitch_tiles);
    let Ok(y) = tile_axis(size.height, Frequency(vertical), settings.stitch_tiles);

    Ok(Self { x, y })
  }

  fn sample(
    &self,
    pixel: super::PixelPosition,
    settings: &Settings,
  ) -> Result<TileSample, Never> {
    let (offset_x, offset_y) = settings.offset;
    let (scale_x, scale_y) = settings.scale;
    let x = f64::from(pixel.x);
    let y = f64::from(pixel.y);
    let Ok(horizontal) = self.x.sample([(x + offset_x) / scale_x, x]);
    let Ok(vertical) = self.y.sample([(y + offset_y) / scale_y, y]);

    Ok(TileSample {
      x: horizontal,
      y: vertical,
    })
  }
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Frequency counts lattice cells per output pixel along one axis"
  )
)]
struct Frequency(f64);

struct TileAxis {
  frequency: Frequency,
  width: Option<i32>,
}

fn tile_axis(
  span: u32,
  frequency: Frequency,
  stitch: StitchTilesAnswer,
) -> Result<TileAxis, Never> {
  match stitch {
    StitchTilesAnswer::No => {
      return Ok(TileAxis {
        frequency,
        width: None,
      });
    }
    StitchTilesAnswer::Yes => {}
  }

  let span_2 = f64::from(span);
  let original = frequency.0;
  let chosen = match original.zero_proximity(4) {
    ZeroProximity::NearZero => original,
    ZeroProximity::Nonzero => {
      let lower = (span_2 * original).floor() / span_2;
      let upper = (span_2 * original).ceil() / span_2;

      match (original / lower < upper / original).then_some(()) {
        Some(()) => lower,
        None => upper,
      }
    }
  };

  let Ok(width) = toward_zero_i32(span_2 * chosen + 0.5);

  Ok(TileAxis {
    frequency: Frequency(chosen),
    width: Some(width),
  })
}

impl TileAxis {
  fn sample(&self, [position, tile_position]: [f64; 2]) -> Result<AxisSample, Never> {
    let frequency = self.frequency.0;
    let period = match self.width {
      Some(width) => {
        let Ok(wrap) = toward_zero_i32(
          tile_position * frequency + f64::from(LATTICE_ORIGIN) + f64::from(width),
        );
        Some(Period {
          width: i64::from(width),
          wrap: i64::from(wrap),
        })
      }
      None => None,
    };

    Ok(AxisSample {
      position: NoisePosition(position * frequency),
      period,
    })
  }
}

#[derive(Clone, Copy)]
struct TileSample {
  x: AxisSample,
  y: AxisSample,
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The position is measured along the Perlin lattice in cells"
  )
)]
struct NoisePosition(f64);

#[derive(Clone, Copy)]
struct AxisSample {
  position: NoisePosition,
  period: Option<Period>,
}

#[derive(Clone, Copy)]
struct Period {
  width: i64,
  wrap: i64,
}

impl AxisSample {
  fn double(&mut self) -> Result<(), Never> {
    self.position.0 *= 2.0;

    match self.period.as_mut() {
      Some(period) => {
        period.width = period.width.saturating_mul(2);
        period.wrap = period
          .wrap
          .saturating_mul(2)
          .saturating_sub(i64::from(LATTICE_ORIGIN));
      }
      None => {}
    }

    Ok(())
  }

  fn lattice(self) -> Result<LatticeAxis, Never> {
    let shifted = self.position.0 + f64::from(LATTICE_ORIGIN);
    let Ok(first_cell) = toward_zero_i32(shifted);
    let first = i64::from(first_cell);
    let second = first.saturating_add(1);
    let (left, right) = match self.period {
      Some(period) => {
        let Ok(wrapped_left) = period.wrap_cell(first);
        let Ok(wrapped_right) = period.wrap_cell(second);
        (wrapped_left, wrapped_right)
      }
      None => (first, second),
    };
    let Ok(first_2) = wrapped::<i64, u8>(left);
    let Ok(second_2) = wrapped::<i64, u8>(right);
    let distance = shifted - shifted.trunc();

    Ok(LatticeAxis {
      first: first_2,
      second: second_2,
      distances: [distance, distance - 1.0],
    })
  }
}

impl Period {
  fn wrap_cell(&self, cell: i64) -> Result<i64, Never> {
    Ok(match (cell >= self.wrap).then_some(()) {
      Some(()) => cell.saturating_sub(self.width),
      None => cell,
    })
  }
}

struct LatticeAxis {
  first: u8,
  second: u8,
  distances: [f64; 2],
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The weight is a dimensionless interpolation ratio"
  )
)]
struct Interpolation(f64);

impl Interpolation {
  fn smooth([t, _]: [f64; 2]) -> Result<Self, Never> {
    Ok(Self(t * t * (3.0 - 2.0 * t)))
  }

  fn between(&self, [a, b]: [f64; 2]) -> Result<f64, Never> {
    Ok(a + self.0 * (b - a))
  }
}

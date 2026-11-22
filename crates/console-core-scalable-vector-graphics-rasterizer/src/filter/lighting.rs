//! Copyright 2020 the Resvg Authors
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{Error, ImageRef, ImageRefMut, PixelPosition, pixel_offset};
use console_core_never::Never;
use console_core_number_conversion::{nearest_f32_from_u64, toward_zero_u8};
use console_core_pixel_buffers::Rgba8;
use console_core_scalable_vector_graphics::console_core_paths::StrictNum;
use console_core_scalable_vector_graphics::filter::{
  DiffuseLighting, LightSource, SpecularLighting,
};
use console_core_scalable_vector_graphics::{ApproxEqUlps, ApproxZeroUlps, Color, ZeroProximity};
use console_core_size::Size;

const HALF: f32 = 1.0 / 2.0;
const THIRD: f32 = 1.0 / 3.0;
const QUARTER: f32 = 1.0 / 4.0;
const TWO_THIRDS: f32 = 2.0 / 3.0;

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "World vectors use output pixels and normalized directions are dimensionless"
  )
)]
struct Vector3 {
  x: f32,
  y: f32,
  z: f32,
}

impl Vector3 {
  fn dot(self, other: Self) -> Result<f32, Never> {
    Ok(self.x * other.x + self.y * other.y + self.z * other.z)
  }

  fn length(self) -> Result<f32, Never> {
    Ok((self.x * self.x + self.y * self.y + self.z * self.z).sqrt())
  }

  fn direction(self) -> Result<Self, Never> {
    let Ok(length) = self.length();

    Ok(match length.zero_proximity(4) {
      ZeroProximity::NearZero => self,
      ZeroProximity::Nonzero => Self {
        x: self.x / length,
        y: self.y / length,
        z: self.z / length,
      },
    })
  }
}

impl core::ops::Sub for Vector3 {
  type Output = Self;

  fn sub(self, rhs: Self) -> Self {
    Self {
      x: self.x - rhs.x,
      y: self.y - rhs.y,
      z: self.z - rhs.z,
    }
  }
}

#[derive(Clone, Copy)]
enum LightingModel {
  Diffuse { strength: f32 },
  Specular { strength: f32, shininess: f32 },
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Surface scale is the height in output pixels of fully opaque samples"
  )
)]
struct SurfaceHeight(f32);

struct Lighting {
  source: LightSource,
  surface: SurfaceHeight,
  model: LightingModel,
  color: Color,
}

#[doc = "Renders diffuse lighting from the input alpha channel. Small images are unchanged."]
pub(super) fn diffuse_lighting(
  filter: &DiffuseLighting,
  source: LightSource,
  input: ImageRef<'_>,
  output: ImageRefMut<'_>,
) -> Result<(), Error> {
  let Ok(strength) = filter.diffuse_constant();
  let Ok(surface) = filter.surface_scale();
  let Ok(color) = filter.lighting_color();

  apply(
    Lighting {
      source,
      surface: SurfaceHeight(surface),
      model: LightingModel::Diffuse { strength },
      color,
    },
    input,
    output,
  )
}

#[doc = "Renders specular lighting from the input alpha channel. Small images are unchanged."]
pub(super) fn specular_lighting(
  filter: &SpecularLighting,
  source: LightSource,
  input: ImageRef<'_>,
  output: ImageRefMut<'_>,
) -> Result<(), Error> {
  let Ok(strength) = filter.specular_constant();
  let Ok(shininess) = filter.specular_exponent();
  let Ok(surface) = filter.surface_scale();
  let Ok(color) = filter.lighting_color();

  apply(
    Lighting {
      source,
      surface: SurfaceHeight(surface),
      model: LightingModel::Specular {
        strength,
        shininess,
      },
      color,
    },
    input,
    output,
  )
}

enum Light {
  Distant(Vector3),
  Point(Vector3),
  Spot {
    origin: Vector3,
    axis: Vector3,
    exponent: f32,
    cone_limit: Option<f32>,
  },
}

impl Light {
  fn from_source(source: LightSource) -> Result<Self, Never> {
    Ok(match source {
      LightSource::DistantLight(light) => {
        let azimuth = light.azimuth.to_radians();
        let elevation = light.elevation.to_radians();
        Self::Distant(Vector3 {
          x: azimuth.cos() * elevation.cos(),
          y: azimuth.sin() * elevation.cos(),
          z: elevation.sin(),
        })
      }
      LightSource::PointLight(light) => Self::Point(Vector3 {
        x: light.x,
        y: light.y,
        z: light.z,
      }),
      LightSource::SpotLight(light) => {
        let origin = Vector3 {
          x: light.x,
          y: light.y,
          z: light.z,
        };
        let aim = Vector3 {
          x: light.points_at_x,
          y: light.points_at_y,
          z: light.points_at_z,
        };
        let Ok(axis) = (aim - origin).direction();
        Self::Spot {
          origin,
          axis,
          exponent: light.specular_exponent.get(),
          cone_limit: light
            .limiting_cone_angle
            .map(|degrees| degrees.to_radians().cos()),
        }
      }
    })
  }

  fn direction_at(
    &self,
    position: PixelPosition,
    input: ImageRef<'_>,
    SurfaceHeight(surface): SurfaceHeight,
  ) -> Result<Vector3, Never> {
    let origin = match self {
      Self::Distant(direction) => return Ok(*direction),
      Self::Point(origin) | Self::Spot { origin, .. } => *origin,
    };
    let Ok(x) = nearest_f32_from_u64(u64::from(position.x));
    let Ok(y) = nearest_f32_from_u64(u64::from(position.y));
    let Ok(alpha) = input.alpha_at(position);
    let z = f32::from(alpha) / 255.0 * surface;

    (origin - Vector3 { x, y, z }).direction()
  }

  fn color_at(&self, color: Color, direction: Vector3) -> Result<Color, Never> {
    let (axis, exponent, cone_limit) = match self {
      Self::Distant(_) | Self::Point(_) => return Ok(color),
      Self::Spot {
        axis,
        exponent,
        cone_limit,
        ..
      } => (*axis, *exponent, *cone_limit),
    };
    let Ok(alignment) = direction.dot(axis);
    let cosine = -alignment;
    let outside_cone = cone_limit.is_some_and(|minimum| cosine < minimum);

    match (cosine <= 0.0 || outside_cone).then_some(()) {
      Some(()) => return Color::black(),
      None => {}
    }

    let dimming = Intensity(cosine.powf(exponent));
    let Ok(red) = dimming.apply(color.red);
    let Ok(green) = dimming.apply(color.green);
    let Ok(blue) = dimming.apply(color.blue);

    Color::new_rgb(red, green, blue)
  }
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Channel intensity is a dimensionless multiplier"
  )
)]
struct Intensity(f32);

impl Intensity {
  fn apply(&self, channel: u8) -> Result<u8, Never> {
    let scaled = f32::from(channel) * self.0;
    let bounded = match (scaled > 255.0, scaled >= 0.0) {
      (true, _) => 255.0,
      (false, true) => scaled,
      (false, false) => 0.0,
    };
    toward_zero_u8(f64::from(bounded + 0.5))
  }
}

fn apply(lighting: Lighting, input: ImageRef<'_>, output: ImageRefMut<'_>) -> Result<(), Error> {
  let source_size = input.dimensions()?;
  let target_size = output.dimensions()?;

  match (
    source_size.width == target_size.width && source_size.height == target_size.height,
    source_size.width >= 3 && source_size.height >= 3,
  ) {
    (false, _) => return Err(Error::InvalidRegion),
    (true, false) => return Ok(()),
    (true, true) => {}
  }

  let Ok(light) = Light::from_source(lighting.source);
  let shader = Shade {
    light,
    input,
    size: source_size,
    surface: lighting.surface,
    model: lighting.model,
    color: lighting.color,
  };

  for y in 0..source_size.height {
    for x in 0..source_size.width {
      let position = PixelPosition { x, y };
      let Ok(offset) = pixel_offset(position, source_size.width);
      let Ok(location) = console_core_number_conversion::index(offset);
      let target = output.data.get_mut(location).ok_or(Error::InvalidRegion)?;
      let Ok(pixel) = shader.pixel(position);
      *target = pixel;
    }
  }

  Ok(())
}

struct Shade<'a> {
  light: Light,
  input: ImageRef<'a>,
  size: Size<u32>,
  surface: SurfaceHeight,
  model: LightingModel,
  color: Color,
}

impl Shade<'_> {
  fn pixel(&self, position: PixelPosition) -> Result<Rgba8, Never> {
    let Ok(normal) = self.normal_at(position);
    let Ok(direction) = self.light.direction_at(position, self.input, self.surface);
    let Ok(color) = self.light.color_at(self.color, direction);
    let Ok(intensity) = self.model.intensity(normal, direction);
    let Ok(r) = intensity.apply(color.red);
    let Ok(g) = intensity.apply(color.green);
    let Ok(b) = intensity.apply(color.blue);
    let a = match self.model {
      LightingModel::Diffuse { .. } => 255,
      LightingModel::Specular { .. } => r.max(g).max(b),
    };

    Ok([r, g, b, a])
  }

  fn normal_at(&self, position: PixelPosition) -> Result<Normal, Never> {
    let Ok(column) = AxisPosition::classify([position.x, self.size.width]);
    let Ok(row) = AxisPosition::classify([position.y, self.size.height]);
    let Ok(stencil) = Stencil::for_edges([column, row]);
    let xs = [
      position.x.saturating_sub(1),
      position.x,
      position
        .x
        .saturating_add(1)
        .min(self.size.width.saturating_sub(1)),
    ];
    let ys = [
      position.y.saturating_sub(1),
      position.y,
      position
        .y
        .saturating_add(1)
        .min(self.size.height.saturating_sub(1)),
    ];
    let mut horizontal = 0_i16;
    let mut vertical = 0_i16;

    for (y, [dx, dy]) in ys.into_iter().zip(stencil.weights) {
      for (x, (wx, wy)) in xs.into_iter().zip(dx.into_iter().zip(dy)) {
        match (wx, wy) {
          (0, 0) => {}
          _ => {
            let Ok(alpha) = self.input.alpha_at(PixelPosition { x, y });
            horizontal = horizontal.saturating_add(wx.saturating_mul(alpha));
            vertical = vertical.saturating_add(wy.saturating_mul(alpha));
          }
        }
      }
    }

    let gradient = [
      f32::from(horizontal.saturating_neg()),
      f32::from(vertical.saturating_neg()),
    ];
    let [x, y] = gradient;
    let flat = matches!(
      (x.zero_proximity(4), y.zero_proximity(4)),
      (ZeroProximity::NearZero, ZeroProximity::NearZero)
    );

    match flat.then_some(()) {
      Some(()) => Ok(Normal::Flat),
      None => {
        let SurfaceHeight(surface) = self.surface;
        let scale = surface / 255.0;
        let [gx, gy] = stencil.scale;
        Ok(Normal::Inclined(Vector3 {
          x: x * scale * gx,
          y: y * scale * gy,
          z: 1.0,
        }))
      }
    }
  }
}

#[derive(Clone, Copy)]
enum Normal {
  Flat,
  Inclined(Vector3),
}

impl LightingModel {
  fn intensity(self, normal: Normal, direction: Vector3) -> Result<Intensity, Never> {
    match self {
      Self::Diffuse { strength } => {
        let Ok(cosine) = surface_cosine(normal, direction);
        Ok(Intensity(strength * cosine))
      }
      Self::Specular {
        strength,
        shininess,
      } => {
        let halfway = Vector3 {
          x: direction.x + 0.0,
          y: direction.y + 0.0,
          z: direction.z + 1.0,
        };
        let Ok(length) = halfway.length();

        match length.zero_proximity(4) {
          ZeroProximity::NearZero => return Ok(Intensity(0.0)),
          ZeroProximity::Nonzero => {}
        }

        let Ok(surface_angle) = surface_cosine(normal, halfway);
        let cosine = surface_angle / length;
        let factor = match shininess.approx_eq_ulps(&1.0, 4).then_some(()) {
          Some(()) => cosine,
          None => cosine.powf(shininess),
        };

        Ok(Intensity(strength * factor))
      }
    }
  }
}

fn surface_cosine(normal: Normal, ray: Vector3) -> Result<f32, Never> {
  match normal {
    Normal::Flat => Ok(ray.z),
    Normal::Inclined(vector) => {
      let Ok(dot) = vector.dot(ray);
      let Ok(length) = vector.length();
      Ok(dot / length)
    }
  }
}

#[derive(Clone, Copy)]
enum AxisPosition {
  Start,
  Interior,
  End,
}

impl AxisPosition {
  fn classify([position, span]: [u32; 2]) -> Result<Self, Never> {
    Ok(match (position, position == span.saturating_sub(1)) {
      (0, _) => Self::Start,
      (_, true) => Self::End,
      (_, false) => Self::Interior,
    })
  }
}

struct Stencil {
  weights: [[[i16; 3]; 2]; 3],
  scale: [f32; 2],
}

impl Stencil {
  #[rustfmt::skip]
  fn for_edges([column, row]: [AxisPosition; 2]) -> Result<Self, Never> {
    use AxisPosition::{End, Interior, Start};

    let stencil = match (column, row) {
      (Start, Start) => Self {
        weights: [[[0, 0, 0], [0, 0, 0]], [[0, -2, 2], [0, -2, -1]], [[0, -1, 1], [0, 2, 1]]],
        scale: [TWO_THIRDS, TWO_THIRDS],
      },
      (End, Start) => Self {
        weights: [[[0, 0, 0], [0, 0, 0]], [[-2, 2, 0], [-1, -2, 0]], [[-1, 1, 0], [1, 2, 0]]],
        scale: [TWO_THIRDS, TWO_THIRDS],
      },
      (Start, End) => Self {
        weights: [[[0, -1, 1], [0, -2, -1]], [[0, -2, 2], [0, 2, 1]], [[0, 0, 0], [0, 0, 0]]],
        scale: [TWO_THIRDS, TWO_THIRDS],
      },
      (End, End) => Self {
        weights: [[[-1, 1, 0], [-1, -2, 0]], [[-2, 2, 0], [1, 2, 0]], [[0, 0, 0], [0, 0, 0]]],
        scale: [TWO_THIRDS, TWO_THIRDS],
      },
      (Interior, Start) => Self {
        weights: [[[0, 0, 0], [0, 0, 0]], [[-2, 0, 2], [-1, -2, -1]], [[-1, 0, 1], [1, 2, 1]]],
        scale: [THIRD, HALF],
      },
      (Interior, End) => Self {
        weights: [[[-1, 0, 1], [-1, -2, -1]], [[-2, 0, 2], [1, 2, 1]], [[0, 0, 0], [0, 0, 0]]],
        scale: [THIRD, HALF],
      },
      (Start, Interior) => Self {
        weights: [[[0, -1, 1], [0, -2, -1]], [[0, -2, 2], [0, 0, 0]], [[0, -1, 1], [0, 2, 1]]],
        scale: [HALF, THIRD],
      },
      (End, Interior) => Self {
        weights: [[[-1, 1, 0], [-1, -2, 0]], [[-2, 2, 0], [0, 0, 0]], [[-1, 1, 0], [1, 2, 0]]],
        scale: [HALF, THIRD],
      },
      (Interior, Interior) => Self {
        weights: [[[-1, 0, 1], [-1, -2, -1]], [[-2, 0, 2], [0, 0, 0]], [[-1, 0, 1], [1, 2, 1]]],
        scale: [QUARTER, QUARTER],
      },
    };

    Ok(stencil)
  }
}

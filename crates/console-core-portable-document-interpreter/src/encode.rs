//! Encoding shading patterns for easy sampling.

use crate::color::{AlphaColor, ColorComponents, ColorSpace};
use crate::interpret::state::{ActiveTransferFunction, Opacity};
use crate::pattern::ShadingPattern;
use crate::shading::{
  GradientExtension, GradientGeometry, ShadingFunction, ShadingType, Triangle,
};
use console_core_circle::Circle;
use console_core_never::Never;
use console_core_number_conversion::nearest_f32_from_f64;
use console_core_perfect_hash::collections::FixedMap;
use console_core_point::Point as Coordinates;
use console_core_triangle::{Triangle as GeometryTriangle, Winding};
use console_core_vector::Vector2;
use kurbo::{Affine, Point};

#[derive(Debug)]
pub struct EncodedShadingPattern {
  pub base_transform: Affine,
  pub(crate) color_space: ColorSpace,
  pub(crate) background_color: AlphaColor,
  pub(crate) shading_type: EncodedShadingType,
  pub(crate) opacity: Opacity,
  pub(crate) transfer_function: Option<ActiveTransferFunction>,
}

impl EncodedShadingPattern {
  #[inline]
  pub fn sample(&self, pos: Point) -> core::result::Result<[f32; 4], console_core_never::Never> {
    let Ok(eval) = self
      .shading_type
      .eval(pos, self.background_color, &self.color_space);

    Ok(
      match eval.map(|v| {
        let Ok(mut components) = v.components();
        *{
          let [_, _, _, _explicit_part, ..] = &mut components;
          _explicit_part
        } *= self.opacity.0;

        match &self.transfer_function {
          Some(tf) => {
            let Ok(alpha_color) = AlphaColor::new(components);
            let Ok(apply) = tf.apply(&alpha_color);
            let Ok(apply_components) = apply.components();
            return apply_components;
          }
          None => {}
        }

        components
      }) {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => [0.0, 0.0, 0.0, 0.0],
      },
    )
  }
}

impl ShadingPattern {
  pub fn encode(&self) -> core::result::Result<EncodedShadingPattern, console_core_never::Never> {
    let base_transform;

    let shading_type = match self.shading.shading_type.as_ref() {
      ShadingType::FunctionBased {
        domain,
        matrix,
        function,
      } => {
        base_transform = (self.matrix * *matrix).inverse();

        {
          let Ok(explicit_value) = encode_function_shading(domain, function);
          explicit_value
        }
      }
      ShadingType::RadialAxial {
        coords,
        domain,
        function,
        extend,
        geometry,
      } => {
        let Ok((encoded, initial_transform)) =
          encode_axial_shading(*coords, *domain, function, *extend, *geometry);

        base_transform = initial_transform * self.matrix.inverse();

        encoded
      }
      ShadingType::TriangleMesh {
        triangles,
        function,
      } => {
        let full_transform = self.matrix;
        let Ok(samples) = sample_triangles(triangles, full_transform);

        base_transform = Affine::IDENTITY;

        EncodedShadingType::Sampled {
          samples,
          function: function.clone(),
        }
      }
      ShadingType::CoonsPatchMesh { patches, function } => {
        let mut triangles = vec![];

        for patch in patches {
          let Ok(()) = patch.to_triangles(&mut triangles);
        }

        let full_transform = self.matrix;
        let Ok(samples) = sample_triangles(&triangles, full_transform);

        base_transform = Affine::IDENTITY;

        EncodedShadingType::Sampled {
          samples,
          function: function.clone(),
        }
      }
      ShadingType::TensorProductPatchMesh { patches, function } => {
        let mut triangles = vec![];

        for patch in patches {
          let Ok(()) = patch.to_triangles(&mut triangles);
        }

        let full_transform = self.matrix;
        let Ok(samples) = sample_triangles(&triangles, full_transform);

        base_transform = Affine::IDENTITY;

        EncodedShadingType::Sampled {
          samples,
          function: function.clone(),
        }
      }
      ShadingType::Dummy => {
        base_transform = Affine::IDENTITY;

        EncodedShadingType::Dummy
      }
    };

    let color_space = self.shading.color_space.clone();

    let background_color = match self.shading.background.as_ref().map(|b| {
      let Ok(rgba) = color_space.to_rgba(b, 1.0, crate::color::InputScaling::ApplyColorSpace);

      rgba
    }) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => AlphaColor::TRANSPARENT,
    };

    Ok(EncodedShadingPattern {
      color_space,
      background_color,
      shading_type,
      base_transform,
      opacity: Opacity(self.opacity),
      transfer_function: self.transfer_function.clone(),
    })
  }
}

fn encode_axial_shading(
  coords: [f32; 6],
  domain: [f32; 2],
  function: &ShadingFunction,
  extend: [GradientExtension; 2],
  geometry: GradientGeometry,
) -> core::result::Result<(EncodedShadingType, Affine), console_core_never::Never> {
  let initial_transform;

  let params = match geometry {
    GradientGeometry::Axial => {
      let [x_0, y_0, x_1, y_1, _, _] = coords;

      initial_transform = {
        let Ok(explicit_value) = ts_from_line_to_line(
          Point::new(f64::from(x_0), f64::from(y_0)),
          Point::new(f64::from(x_1), f64::from(y_1)),
          Point::ZERO,
          Point::new(1.0, 0.0),
        );
        explicit_value
      };

      RadialAxialParams::Axial
    }
    GradientGeometry::Radial => {
      let [x_0, y_0, r0, x_1, y_1, r_1] = coords;

      initial_transform = Affine::translate((f64::from(-x_0), f64::from(-y_0)));
      let new_x1 = x_1 - x_0;
      let new_y1 = y_1 - y_0;

      let Ok(first) = Circle::new(Coordinates::default(), f64::from(r0));
      let Ok(last) = Circle::new(
        Coordinates {
          x: f64::from(new_x1),
          y: f64::from(new_y1),
        },
        f64::from(r_1),
      );

      RadialAxialParams::Radial([first, last])
    }
  };

  Ok((
    EncodedShadingType::RadialAxial {
      function: function.clone(),
      params,
      domain,
      extend,
    },
    initial_transform,
  ))
}

fn sample_triangles(
  triangles: &[Triangle],
  transform: Affine,
) -> Result<FixedMap<(u16, u16), ColorComponents>, Never> {
  let mut map = FixedMap::default();

  for triangle in triangles {
    let geometry = GeometryTriangle::from(triangle);
    let transformed = GeometryTriangle::from(
      geometry
        .vertices
        .map(|vertex| Coordinates::from(transform * Point::from(vertex))),
    );
    let Ok(bounds) = transformed.bounds();
    let Ok(prepared) = transformed.barycentric();
    let Ok(top) = console_core_number_conversion::toward_zero_u16(bounds.y0.floor());
    let Ok(bottom) = console_core_number_conversion::toward_zero_u16(bounds.y1.ceil());
    let Ok(left) = console_core_number_conversion::toward_zero_u16(bounds.x0.floor());
    let Ok(right) = console_core_number_conversion::toward_zero_u16(bounds.x1.ceil());

    for y in top..bottom {
      for x in left..right {
        let point = Coordinates {
          x: f64::from(x),
          y: f64::from(y),
        };
        let Ok(winding) = transformed.winding(point);

        match winding {
          Winding::Positive | Winding::Negative => {
            let Ok(weights) = prepared.weights_at(point);
            let Ok(color) = triangle.interpolate_weights(weights);
            map.insert((x, y), color);
          }
          Winding::Outside => {}
        }
      }
    }
  }

  Ok(map)
}

fn encode_function_shading(
  domain: &[f32; 4],
  function: &ShadingFunction,
) -> core::result::Result<EncodedShadingType, console_core_never::Never> {
  let domain_2 = kurbo::Rect::new(
    f64::from(match domain {
      &[element, ..] => element,
    }),
    f64::from(match domain {
      &[_, _, element, ..] => element,
    }),
    f64::from(match domain {
      &[_, element, ..] => element,
    }),
    f64::from(match domain {
      &[_, _, _, element, ..] => element,
    }),
  );

  Ok(EncodedShadingType::FunctionBased {
    domain: domain_2,
    function: function.clone(),
  })
}

#[derive(Debug)]
pub(crate) enum RadialAxialParams {
  Axial,
  Radial([Circle<f64>; 2]),
}

#[derive(Debug)]
pub(crate) enum EncodedShadingType {
  FunctionBased {
    domain: kurbo::Rect,
    function: ShadingFunction,
  },
  RadialAxial {
    function: ShadingFunction,
    params: RadialAxialParams,
    domain: [f32; 2],
    extend: [GradientExtension; 2],
  },
  Sampled {
    samples: FixedMap<(u16, u16), ColorComponents>,
    function: Option<ShadingFunction>,
  },
  Dummy,
}

fn gradient_sample_parameter(
  pos: Point,
  params: &RadialAxialParams,
  &[start, end]: &[f32; 2],
  &[first, last]: &[GradientExtension; 2],
) -> Result<Option<f32>, Never> {
  let sample = match params {
    RadialAxialParams::Axial => {
      let Ok(position) = nearest_f32_from_f64(pos.x);

      match position.partial_cmp(&f32::MIN) {
        Some(core::cmp::Ordering::Equal) => return Ok(None),
        Some(core::cmp::Ordering::Less | core::cmp::Ordering::Greater) | None => position,
      }
    }
    RadialAxialParams::Radial(circles) => {
      let Ok(position) = radial_pos(pos, circles, [first, last]);

      match position {
        Some(value) => match value.partial_cmp(&f32::MIN) {
          Some(core::cmp::Ordering::Equal) => return Ok(None),
          Some(core::cmp::Ordering::Less | core::cmp::Ordering::Greater) | None => value,
        },
        None => return Ok(None),
      }
    }
  };
  let t = match (
    sample.partial_cmp(&0.0),
    sample.partial_cmp(&1.0),
    first,
    last,
  ) {
    (Some(core::cmp::Ordering::Less), _, GradientExtension::Extend, _) => 0.0,
    (_, Some(core::cmp::Ordering::Greater), _, GradientExtension::Extend) => 1.0,
    (Some(core::cmp::Ordering::Less), _, GradientExtension::Stop, _)
    | (_, Some(core::cmp::Ordering::Greater), _, GradientExtension::Stop) => return Ok(None),
    _within_or_unordered => sample,
  };

  Ok(Some(start + (end - start) * t))
}

impl EncodedShadingType {
  pub(crate) fn eval(
    &self,
    pos: Point,
    bg_color: AlphaColor,
    color_space: &ColorSpace,
  ) -> core::result::Result<Option<AlphaColor>, console_core_never::Never> {
    Ok(match self {
      Self::FunctionBased { domain, function } => {
        match ::core::primitive::bool::then_some(!domain.contains(pos), ()) {
          ::core::option::Option::Some(()) => Some(bg_color),
          ::core::option::Option::None => {
            let Ok(eval) = function.eval(&vec![
              match ::console_core_number_conversion::nearest_f32_from_f64(pos.x) {
                ::core::result::Result::Ok(_converted_number) => _converted_number,
              },
              match ::console_core_number_conversion::nearest_f32_from_f64(pos.y) {
                ::core::result::Result::Ok(_converted_number) => _converted_number,
              },
            ]);
            let out = match eval {
              Some(explicit_value) => explicit_value,
              None => return Ok(None),
            };

            let Ok(rgba) =
              color_space.to_rgba(&out, 1.0, crate::color::InputScaling::ApplyColorSpace);

            Some(rgba)
          }
        }
      }
      Self::RadialAxial {
        function,
        params,
        domain,
        extend,
      } => {
        let Ok(sample) = gradient_sample_parameter(pos, params, domain, extend);
        let t_2 = match sample {
          Some(sample_2) => sample_2,
          None => return Ok(Some(bg_color)),
        };

        let Ok(eval) = function.eval(&vec![t_2]);
        let val = match eval {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        };

        let Ok(rgba) =
          color_space.to_rgba(&val, 1.0, crate::color::InputScaling::ApplyColorSpace);

        Some(rgba)
      }
      Self::Sampled { samples, function } => {
        let Ok(pos_x) = console_core_number_conversion::toward_zero_u16(pos.x);
        let Ok(pos_y) = console_core_number_conversion::toward_zero_u16(pos.y);
        let sample_point = (pos_x, pos_y);

        let color = match samples.get(&sample_point) {
          Some(color) => color,
          None => return Ok(Some(bg_color)),
        };
        let Ok(sample) = sample_function(function.as_ref(), color, color_space);

        sample
      }

      Self::Dummy => Some(AlphaColor::TRANSPARENT),
    })
  }
}

fn sample_function(
  function: Option<&ShadingFunction>,
  color: &ColorComponents,
  color_space: &ColorSpace,
) -> Result<Option<AlphaColor>, Never> {
  let evaluated;
  let components = match function {
    Some(function_2) => {
      let Ok(sample) = function_2.eval(color);
      evaluated = match sample {
        Some(sample_2) => sample_2,
        None => return Ok(None),
      };
      &evaluated
    }
    None => color,
  };
  let Ok(rgba) =
    color_space.to_rgba(components, 1.0, crate::color::InputScaling::ApplyColorSpace);

  Ok(Some(rgba))
}

fn ts_from_line_to_line(
  src1: Point,
  src2: Point,
  dst1: Point,
  dst2: Point,
) -> core::result::Result<Affine, console_core_never::Never> {
  let Ok(unit_to_line1) = unit_to_line(src1, src2);
  let line1_to_unit = unit_to_line1.inverse();
  let Ok(unit_to_line2) = unit_to_line(dst1, dst2);

  Ok(unit_to_line2 * line1_to_unit)
}

fn unit_to_line(p0: Point, p1: Point) -> core::result::Result<Affine, console_core_never::Never> {
  Ok(Affine::new([
    p1.y - p0.y,
    p0.x - p1.x,
    p1.x - p0.x,
    p1.y - p0.y,
    p0.x,
    p0.y,
  ]))
}

fn radial_pos(
  pos: Point,
  [first, last]: &[Circle<f64>; 2],
  [min_extend, max_extend]: [GradientExtension; 2],
) -> Result<Option<f32>, Never> {
  let Ok(distance) = Vector2::<f64>::between([first.center, last.center]);
  let Ok(from_first) = Vector2::<f64>::between([first.center, Coordinates::from(pos)]);
  let Ok(axis) = distance.try_map(nearest_f32_from_f64);
  let Ok(offset) = from_first.try_map(nearest_f32_from_f64);
  let Ok(r0) = nearest_f32_from_f64(first.radius);
  let Ok(r1) = nearest_f32_from_f64(last.radius);
  let dr = r1 - r0;
  let Ok(axis_length_squared) = axis.length_squared();
  let Ok(projected) = offset.dot(axis);
  let Ok(offset_squared) = offset.length_squared();
  let a = axis_length_squared - dr * dr;
  let b = -2.0 * (projected + r0 * dr);
  let c = offset_squared - r0 * r0;

  let discriminant = b * b - 4.0 * a * c;

  match ::core::primitive::bool::then_some(discriminant < 0.0, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(a.abs() < 1e-6, ()) {
    ::core::option::Option::Some(()) => {
      match ::core::primitive::bool::then_some(b.abs() < 1e-6, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(None);
        }
        ::core::option::Option::None => {}
      }

      let t = -c / b;

      match ::core::primitive::bool::then_some(
        (matches!(min_extend, GradientExtension::Stop) && t < 0.0)
          || (matches!(max_extend, GradientExtension::Stop) && t > 1.0),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          return Ok(None);
        }
        ::core::option::Option::None => {}
      }

      let r_t = r0 + dr * t;

      match ::core::primitive::bool::then_some(r_t < 0.0, ()) {
        ::core::option::Option::Some(()) => {
          return Ok(None);
        }
        ::core::option::Option::None => {}
      }

      return Ok(Some(t));
    }
    ::core::option::Option::None => {}
  }

  let sqrt_d = discriminant.sqrt();
  let t1 = (-b - sqrt_d) / (2.0 * a);
  let t2 = (-b + sqrt_d) / (2.0 * a);

  let max = t1.max(t2);
  let mut take_max = Some(max);
  let min = t1.min(t2);
  let mut take_min = Some(min);

  match ::core::primitive::bool::then_some(
    (matches!(min_extend, GradientExtension::Stop) && min < 0.0) || r0 + dr * min < 0.0,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      take_min = None;
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    (matches!(max_extend, GradientExtension::Stop) && max > 1.0) || r0 + dr * max < 0.0,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      take_max = None;
    }
    ::core::option::Option::None => {}
  }

  Ok(match (take_min, take_max) {
    (Some(_), Some(max_2)) => Some(max_2),
    (Some(min_2), None) => Some(min_2),
    (None, Some(max_3)) => Some(max_3),
    (None, None) => None,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn radial_samples_follow_the_centers_and_radii() {
    let Ok(first) = Circle::new(Coordinates::from((4.0, 5.0)), 1.0);
    let Ok(last) = Circle::new(Coordinates::from((6.0, 5.0)), 3.0);
    let circles = [first, last];
    let stop = [GradientExtension::Stop; 2];

    assert_eq!(
      radial_pos(Point::new(4.0, 7.0), &circles, stop),
      Ok(Some(0.75))
    );
    assert_eq!(radial_pos(Point::new(4.0, 9.0), &circles, stop), Ok(None));
  }
}

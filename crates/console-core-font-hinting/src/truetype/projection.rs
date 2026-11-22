//! Point projection.

use super::graphics::{CoordAxis, GraphicsState};
use raw::types::{F26Dot6, Point};

impl GraphicsState<'_> {
  pub fn update_projection_state(&mut self) -> Result<(), console_core_never::Never> {
    let _: () = {
      const ONE: i32 = 0x4000;

      match ::core::primitive::bool::then_some(self.freedom_vector.x == ONE, ()) {
        ::core::option::Option::Some(()) => {
          self.fdotp = self.proj_vector.x;
        }
        ::core::option::Option::None => match ::core::primitive::bool::then_some(self.freedom_vector.y == ONE, ()) {
          ::core::option::Option::Some(()) => {
            self.fdotp = self.proj_vector.y;
          }
          ::core::option::Option::None => {
            let px = self.proj_vector.x;
            let py = self.proj_vector.y;
            let fx = self.freedom_vector.x;
            let fy = self.freedom_vector.y;
            self.fdotp = i32::wrapping_shr(
              i32::wrapping_add(i32::wrapping_mul(px, fx), i32::wrapping_mul(py, fy)),
              14_u32,
            );
          }
        },
      }

      self.proj_axis = CoordAxis::Both;

      match ::core::primitive::bool::then_some(self.proj_vector.x == ONE, ()) {
        ::core::option::Option::Some(()) => {
          self.proj_axis = CoordAxis::X;
        }
        ::core::option::Option::None => match ::core::primitive::bool::then_some(self.proj_vector.y == ONE, ()) {
          ::core::option::Option::Some(()) => {
            self.proj_axis = CoordAxis::Y;
          }
          ::core::option::Option::None => {}
        },
      }

      self.dual_proj_axis = CoordAxis::Both;

      match ::core::primitive::bool::then_some(self.dual_proj_vector.x == ONE, ()) {
        ::core::option::Option::Some(()) => {
          self.dual_proj_axis = CoordAxis::X;
        }
        ::core::option::Option::None => match ::core::primitive::bool::then_some(self.dual_proj_vector.y == ONE, ()) {
          ::core::option::Option::Some(()) => {
            self.dual_proj_axis = CoordAxis::Y;
          }
          ::core::option::Option::None => {}
        },
      }

      self.freedom_axis = CoordAxis::Both;

      match ::core::primitive::bool::then_some(self.fdotp == ONE, ()) {
        ::core::option::Option::Some(()) => match ::core::primitive::bool::then_some(self.freedom_vector.x == ONE, ()) {
          ::core::option::Option::Some(()) => {
            self.freedom_axis = CoordAxis::X;
          }
          ::core::option::Option::None => match ::core::primitive::bool::then_some(self.freedom_vector.y == ONE, ()) {
            ::core::option::Option::Some(()) => {
              self.freedom_axis = CoordAxis::Y;
            }
            ::core::option::Option::None => {}
          },
        },
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(self.fdotp.abs() < 0x400, ()) {
        ::core::option::Option::Some(()) => {
          self.fdotp = ONE;
        }
        ::core::option::Option::None => {}
      }
    };
    Ok(())
  }

  #[inline(always)]
  pub fn project(
    &self,
    v1: Point<F26Dot6>,
    v2: Point<F26Dot6>,
  ) -> Result<F26Dot6, console_core_never::Never> {
    Ok(match self.proj_axis {
      CoordAxis::X => v1.x - v2.x,

      CoordAxis::Y => v1.y - v2.y,
      CoordAxis::Both => {
        let dx = v1.x - v2.x;
        let dy = v1.y - v2.y;

        {
          let Ok(value) = F26Dot6::from_bits({
            let Ok(value) = dot14([
              Point {
                x: {
                  let Ok(bits) = dx.to_bits();
                  bits
                },
                y: {
                  let Ok(bits) = dy.to_bits();
                  bits
                },
              },
              Point {
                x: self.proj_vector.x,
                y: self.proj_vector.y,
              },
            ]);
            value
          });
          value
        }
      }
    })
  }

  #[inline(always)]
  pub fn dual_project(
    &self,
    v1: Point<F26Dot6>,
    v2: Point<F26Dot6>,
  ) -> Result<F26Dot6, console_core_never::Never> {
    Ok(match self.dual_proj_axis {
      CoordAxis::X => v1.x - v2.x,

      CoordAxis::Y => v1.y - v2.y,
      CoordAxis::Both => {
        let dx = v1.x - v2.x;
        let dy = v1.y - v2.y;

        {
          let Ok(value) = F26Dot6::from_bits({
            let Ok(value) = dot14([
              Point {
                x: {
                  let Ok(bits) = dx.to_bits();
                  bits
                },
                y: {
                  let Ok(bits) = dy.to_bits();
                  bits
                },
              },
              Point {
                x: self.dual_proj_vector.x,
                y: self.dual_proj_vector.y,
              },
            ]);
            value
          });
          value
        }
      }
    })
  }

  #[inline(always)]
  pub fn dual_project_unscaled(
    &self,
    v1: Point<i32>,
    v2: Point<i32>,
  ) -> Result<i32, console_core_never::Never> {
    Ok(match self.dual_proj_axis {
      CoordAxis::X => i32::wrapping_sub(v1.x, v2.x),

      CoordAxis::Y => i32::wrapping_sub(v1.y, v2.y),
      CoordAxis::Both => {
        let dx = i32::wrapping_sub(v1.x, v2.x);
        let dy = i32::wrapping_sub(v1.y, v2.y);

        {
          let Ok(value) = dot14([
            Point { x: dx, y: dy },
            Point {
              x: self.dual_proj_vector.x,
              y: self.dual_proj_vector.y,
            },
          ]);
          value
        }
      }
    })
  }
}

fn dot14(vectors: [Point<i32>; 2]) -> Result<i32, console_core_never::Never> {
  let [Point { x: ax, y: ay }, Point { x: bx, y: by }] = vectors;
  Ok({
    let mut v1 = i64::wrapping_mul(
      {
        let Ok(value) = console_core_number_conversion::wrapped::<_, i64>(ax);
        value
      },
      {
        let Ok(value) = console_core_number_conversion::wrapped::<_, i64>(bx);
        value
      },
    );
    let v2 = i64::wrapping_mul(
      {
        let Ok(value) = console_core_number_conversion::wrapped::<_, i64>(ay);
        value
      },
      {
        let Ok(value) = console_core_number_conversion::wrapped::<_, i64>(by);
        value
      },
    );
    v1 = i64::wrapping_add(v1, v2);
    v1 = i64::wrapping_add(v1, i64::wrapping_add(0x2000, i64::wrapping_shr(v1, 63_u32)));

    {
      let Ok(value) =
        console_core_number_conversion::wrapped::<_, i32>(i64::wrapping_shr(v1, 14_u32));
      value
    }
  })
}

#[cfg(test)]
mod tests {
  use super::{super::math, CoordAxis, F26Dot6, GraphicsState, Point};

  #[test]
  fn project_one_axis() {
    let mut state = GraphicsState {
      proj_vector: {
        let Ok(math_result_1) = math::normalize14(read_fonts::types::Point { x: 1, y: 0 });
        math_result_1
      },
      ..Default::default()
    };
    let ::core::result::Result::Ok(()) = state.update_projection_state();
    assert_eq!(state.proj_axis, CoordAxis::X);
    assert_eq!(
      state.proj_vector,
      ({
        let Ok(value) = Point::new(0x4000, 0);
        value
      })
    );
    let cases = &[
      (
        ({
          let Ok(value) = Point::new(0, 0);
          value
        }),
        ({
          let Ok(value) = Point::new(0, 0);
          value
        }),
        0,
      ),
      (
        ({
          let Ok(value) = Point::new(100, 100);
          value
        }),
        ({
          let Ok(value) = Point::new(0, 0);
          value
        }),
        100,
      ),
      (
        ({
          let Ok(value) = Point::new(42, 100);
          value
        }),
        ({
          let Ok(value) = Point::new(100, 0);
          value
        }),
        -58,
      ),
      (
        ({
          let Ok(value) = Point::new(0, 0);
          value
        }),
        ({
          let Ok(value) = Point::new(100, 100);
          value
        }),
        -100,
      ),
    ];
    ({
      let Ok(migrated_value) = test_project_cases(&state, cases);
      migrated_value
    });
  }

  #[test]
  fn project_both_axes() {
    let mut state = GraphicsState {
      proj_vector: {
        let Ok(math_result_2) = math::normalize14(read_fonts::types::Point {
          x: 0x4000,
          y: 0x4000,
        });
        math_result_2
      },
      ..Default::default()
    };
    let ::core::result::Result::Ok(()) = state.update_projection_state();
    assert_eq!(state.proj_axis, CoordAxis::Both);
    let cases = &[
      (
        ({
          let Ok(value) = Point::new(0, 0);
          value
        }),
        ({
          let Ok(value) = Point::new(0, 0);
          value
        }),
        0,
      ),
      (
        ({
          let Ok(value) = Point::new(100, 100);
          value
        }),
        ({
          let Ok(value) = Point::new(0, 0);
          value
        }),
        141,
      ),
      (
        ({
          let Ok(value) = Point::new(42, 100);
          value
        }),
        ({
          let Ok(value) = Point::new(100, 0);
          value
        }),
        30,
      ),
      (
        ({
          let Ok(value) = Point::new(0, 0);
          value
        }),
        ({
          let Ok(value) = Point::new(100, 100);
          value
        }),
        -141,
      ),
    ];
    ({
      let Ok(migrated_value) = test_project_cases(&state, cases);
      migrated_value
    });
  }

  fn test_project_cases(
    state: &GraphicsState,
    cases: &[(Point<i32>, Point<i32>, i32)],
  ) -> core::result::Result<(), console_core_never::Never> {
    for (left_bits, right_bits, expected) in cases.iter().copied() {
      let Ok(v1) = left_bits.map(|bits| {
        let Ok(coordinate) = F26Dot6::from_bits(bits);
        coordinate
      });
      let Ok(v2) = right_bits.map(|bits| {
        let Ok(coordinate) = F26Dot6::from_bits(bits);
        coordinate
      });
      let Ok(result) = ({
        let Ok(value) = state.project(v1, v2);
        value
      })
      .to_bits();
      assert_eq!(
        result, expected,
        "project({v1:?}, {v2:?}) = {result} (expected {expected})"
      );
    }

    Ok(())
  }
}

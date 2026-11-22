#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use crate::NoStdFloat;

#[allow(
  non_camel_case_types,
  reason = "the name is the lane type and count, as core::simd and the rasterizer's own lanes spell them"
)]
#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub struct f32x2(pub [f32; 2]);

impl f32x2 {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The lane constructor accepts raw values and cannot infer their units"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public lane constructor specifies lane zero then lane one"
    )
  )]
  pub fn new(a: f32, b: f32) -> core::result::Result<f32x2, console_core_never::Never> {
    Ok(f32x2([a, b]))
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The operation copies one raw value into both lanes"
    )
  )]
  pub fn splat(x: f32) -> core::result::Result<f32x2, console_core_never::Never> {
    Ok(f32x2([x, x]))
  }

  pub fn abs(self) -> core::result::Result<f32x2, console_core_never::Never> {
    Ok(f32x2([
      ({
        let Ok(explicit_value) = self.x();
        explicit_value
      })
      .abs(),
      ({
        let Ok(explicit_value) = self.y();
        explicit_value
      })
      .abs(),
    ]))
  }

  pub fn min(self, other: f32x2) -> core::result::Result<f32x2, console_core_never::Never> {
    let [x, y] = self.0;
    let [other_x, other_y] = other.0;
    let first = match ::core::primitive::bool::then_some(other_x < x, ()) {
      ::core::option::Option::Some(()) => other_x,
      ::core::option::Option::None => x,
    };
    let second = match ::core::primitive::bool::then_some(other_y < y, ()) {
      ::core::option::Option::Some(()) => other_y,
      ::core::option::Option::None => y,
    };

    Ok(f32x2([first, second]))
  }

  pub fn max(self, other: f32x2) -> core::result::Result<f32x2, console_core_never::Never> {
    let [x, y] = self.0;
    let [other_x, other_y] = other.0;
    let first = match ::core::primitive::bool::then_some(x < other_x, ()) {
      ::core::option::Option::Some(()) => other_x,
      ::core::option::Option::None => x,
    };
    let second = match ::core::primitive::bool::then_some(y < other_y, ()) {
      ::core::option::Option::Some(()) => other_y,
      ::core::option::Option::None => y,
    };

    Ok(f32x2([first, second]))
  }

  pub fn max_component(self) -> core::result::Result<f32, console_core_never::Never> {
    let [x, y] = self.0;
    Ok(match ::core::primitive::bool::then_some(x < y, ()) {
      ::core::option::Option::Some(()) => y,
      ::core::option::Option::None => x,
    })
  }

  pub fn x(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(match self.0 {
      [element, ..] => element,
    })
  }

  pub fn y(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(match self.0 {
      [_, element, ..] => element,
    })
  }
}

impl core::ops::Add<f32x2> for f32x2 {
  type Output = f32x2;

  fn add(self, other: f32x2) -> f32x2 {
    f32x2([
      ({
        let Ok(explicit_value) = self.x();
        explicit_value
      }) + ({
        let Ok(explicit_value) = other.x();
        explicit_value
      }),
      ({
        let Ok(explicit_value) = self.y();
        explicit_value
      }) + ({
        let Ok(explicit_value) = other.y();
        explicit_value
      }),
    ])
  }
}

impl core::ops::Sub<f32x2> for f32x2 {
  type Output = f32x2;

  fn sub(self, other: f32x2) -> f32x2 {
    f32x2([
      ({
        let Ok(explicit_value) = self.x();
        explicit_value
      }) - ({
        let Ok(explicit_value) = other.x();
        explicit_value
      }),
      ({
        let Ok(explicit_value) = self.y();
        explicit_value
      }) - ({
        let Ok(explicit_value) = other.y();
        explicit_value
      }),
    ])
  }
}

impl core::ops::Mul<f32x2> for f32x2 {
  type Output = f32x2;

  fn mul(self, other: f32x2) -> f32x2 {
    f32x2([
      ({
        let Ok(explicit_value) = self.x();
        explicit_value
      }) * ({
        let Ok(explicit_value) = other.x();
        explicit_value
      }),
      ({
        let Ok(explicit_value) = self.y();
        explicit_value
      }) * ({
        let Ok(explicit_value) = other.y();
        explicit_value
      }),
    ])
  }
}

impl core::ops::Div<f32x2> for f32x2 {
  type Output = f32x2;

  fn div(self, other: f32x2) -> f32x2 {
    f32x2([
      ({
        let Ok(explicit_value) = self.x();
        explicit_value
      }) / ({
        let Ok(explicit_value) = other.x();
        explicit_value
      }),
      ({
        let Ok(explicit_value) = self.y();
        explicit_value
      }) / ({
        let Ok(explicit_value) = other.y();
        explicit_value
      }),
    ])
  }
}

use super::{f32x8, u16x16};

#[derive(Copy, Clone, Debug)]
#[repr(C, align(32))]
pub struct f32x16(pub f32x8, pub f32x8);

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "all selected lane storage layouts have a valid zero bit pattern in every lane"
  )
)]
unsafe impl bytemuck::Zeroable for f32x16 {}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "each selected layout is one register, a fixed lane array, or adjacent equal-width lane blocks without padding"
  )
)]
unsafe impl bytemuck::Pod for f32x16 {}

impl Default for f32x16 {
  fn default() -> Self {
    let Ok(splat) = Self::splat(0.0);

    splat
  }
}

impl f32x16 {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the generic SIMD broadcast preserves the units of its caller's lane value"
    )
  )]
  pub fn splat(n: f32) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(splat) = f32x8::splat(n);

    Ok(Self(splat, {
      let Ok(explicit_value) = f32x8::splat(n);
      explicit_value
    }))
  }

  #[inline]
  pub fn abs(&self) -> core::result::Result<Self, console_core_never::Never> {
    let abs = |x| bytemuck::cast::<i32, f32>(bytemuck::cast::<f32, i32>(x) & 0x7fffffff);

    let n0: [f32; 8] = <[::core::primitive::f32; 8] as ::core::convert::From<
      crate::wide::f32x8_t::f32x8,
    >>::from(self.0);
    let n1: [f32; 8] = <[::core::primitive::f32; 8] as ::core::convert::From<
      crate::wide::f32x8_t::f32x8,
    >>::from(self.1);

    Ok(Self(
      f32x8::from([
        abs(match n0 {
          [element, ..] => element,
        }),
        abs(match n0 {
          [_, element, ..] => element,
        }),
        abs(match n0 {
          [_, _, element, ..] => element,
        }),
        abs(match n0 {
          [_, _, _, element, ..] => element,
        }),
        abs(match n0 {
          [_, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        abs(match n0 {
          [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        abs(match n0 {
          [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        abs(match n0 {
          [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
      ]),
      f32x8::from([
        abs(match n1 {
          [element, ..] => element,
        }),
        abs(match n1 {
          [_, element, ..] => element,
        }),
        abs(match n1 {
          [_, _, element, ..] => element,
        }),
        abs(match n1 {
          [_, _, _, element, ..] => element,
        }),
        abs(match n1 {
          [_, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        abs(match n1 {
          [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        abs(match n1 {
          [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
        abs(match n1 {
          [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
        }),
      ]),
    ))
  }

  pub fn cmp_gt(self, rhs: &Self) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(cmp_gt) = self.0.cmp_gt(rhs.0);
    let Ok(own_cmp_gt) = self.1.cmp_gt(rhs.1);

    Ok(Self(cmp_gt, own_cmp_gt))
  }

  pub fn blend(self, t: Self, f: Self) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(blend) = self.0.blend(t.0, f.0);
    let Ok(own_blend) = self.1.blend(t.1, f.1);

    Ok(Self(blend, own_blend))
  }

  pub fn normalize(&self) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(normalize) = self.0.normalize();
    let Ok(own_normalize) = self.1.normalize();

    Ok(Self(normalize, own_normalize))
  }

  pub fn floor(&self) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(roundtrip) = self.round();

    Ok(
      roundtrip
        - ({
          let Ok(explicit_value) = ({
            let Ok(explicit_value) = roundtrip.cmp_gt(self);
            explicit_value
          })
          .blend(
            {
              let Ok(explicit_value) = f32x16::splat(1.0);
              explicit_value
            },
            {
              let Ok(explicit_value) = f32x16::splat(0.0);
              explicit_value
            },
          );
          explicit_value
        }),
    )
  }

  pub fn sqrt(&self) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self(
      {
        let Ok(explicit_value) = self.0.sqrt();
        explicit_value
      },
      {
        let Ok(explicit_value) = self.1.sqrt();
        explicit_value
      },
    ))
  }

  pub fn round(&self) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(round) = self.0.round();
    let Ok(own_round) = self.1.round();

    Ok(Self(round, own_round))
  }

  pub fn save_to_u16x16(&self, dst: &mut u16x16) -> Result<(), console_core_never::Never> {
    let low = <[f32; 8]>::from(self.0);
    let high = <[f32; 8]>::from(self.1);

    for (output, input) in dst.0.iter_mut().zip(low.into_iter().chain(high)) {
      let Ok(value) = console_core_number_conversion::toward_zero_u16(f64::from(input));
      *output = value;
    }

    Ok(())
  }
}

impl core::ops::Add<f32x16> for f32x16 {
  type Output = Self;

  fn add(self, rhs: Self) -> Self::Output {
    Self(self.0 + rhs.0, self.1 + rhs.1)
  }
}

impl core::ops::Sub<f32x16> for f32x16 {
  type Output = Self;

  fn sub(self, rhs: Self) -> Self::Output {
    Self(self.0 - rhs.0, self.1 - rhs.1)
  }
}

impl core::ops::Mul<f32x16> for f32x16 {
  type Output = Self;

  fn mul(self, rhs: Self) -> Self::Output {
    Self(self.0 * rhs.0, self.1 * rhs.1)
  }
}

#[derive(Default, Clone, Copy, PartialEq, Debug)]
#[repr(C, align(16))]
pub struct f32x4(pub [f32; 4]);

impl f32x4 {
  pub fn max(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    self.combine(rhs, f32::max)
  }

  pub fn min(self, rhs: Self) -> core::result::Result<Self, console_core_never::Never> {
    self.combine(rhs, f32::min)
  }

  fn combine(
    self,
    rhs: Self,
    select: impl Fn(f32, f32) -> f32,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let [a, b, c, d] = self.0;
    let [w, x, y, z] = rhs.0;
    Ok(Self([
      select(a, w),
      select(b, x),
      select(c, y),
      select(d, z),
    ]))
  }
}

impl core::ops::Add for f32x4 {
  type Output = Self;

  fn add(self, rhs: Self) -> Self::Output {
    Self([
      (match self.0 {
        [element, ..] => element,
      }) + (match rhs.0 {
        [element, ..] => element,
      }),
      (match self.0 {
        [_, element, ..] => element,
      }) + (match rhs.0 {
        [_, element, ..] => element,
      }),
      (match self.0 {
        [_, _, element, ..] => element,
      }) + (match rhs.0 {
        [_, _, element, ..] => element,
      }),
      (match self.0 {
        [_, _, _, element, ..] => element,
      }) + (match rhs.0 {
        [_, _, _, element, ..] => element,
      }),
    ])
  }
}

impl core::ops::AddAssign for f32x4 {
  fn add_assign(&mut self, rhs: f32x4) {
    *self = *self + rhs;
  }
}

impl core::ops::Sub for f32x4 {
  type Output = Self;

  fn sub(self, rhs: Self) -> Self::Output {
    Self([
      (match self.0 {
        [element, ..] => element,
      }) - (match rhs.0 {
        [element, ..] => element,
      }),
      (match self.0 {
        [_, element, ..] => element,
      }) - (match rhs.0 {
        [_, element, ..] => element,
      }),
      (match self.0 {
        [_, _, element, ..] => element,
      }) - (match rhs.0 {
        [_, _, element, ..] => element,
      }),
      (match self.0 {
        [_, _, _, element, ..] => element,
      }) - (match rhs.0 {
        [_, _, _, element, ..] => element,
      }),
    ])
  }
}

impl core::ops::Mul for f32x4 {
  type Output = Self;

  fn mul(self, rhs: Self) -> Self::Output {
    Self([
      (match self.0 {
        [element, ..] => element,
      }) * (match rhs.0 {
        [element, ..] => element,
      }),
      (match self.0 {
        [_, element, ..] => element,
      }) * (match rhs.0 {
        [_, element, ..] => element,
      }),
      (match self.0 {
        [_, _, element, ..] => element,
      }) * (match rhs.0 {
        [_, _, element, ..] => element,
      }),
      (match self.0 {
        [_, _, _, element, ..] => element,
      }) * (match rhs.0 {
        [_, _, _, element, ..] => element,
      }),
    ])
  }
}

impl core::ops::MulAssign for f32x4 {
  fn mul_assign(&mut self, rhs: f32x4) {
    *self = *self * rhs;
  }
}

use crate::{math::approx_powf, pipeline};
use console_core_paths::{NormalizedF32, Scalar};
use console_core_paths::{StrictNormalized, StrictNum};

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use console_core_paths::NoStdFloat;

pub type AlphaU8 = u8;

pub const ALPHA_U8_TRANSPARENT: AlphaU8 = 0x00;

pub const ALPHA_U8_OPAQUE: AlphaU8 = 0xFF;

pub const ALPHA_TRANSPARENT: NormalizedF32 = NormalizedF32::ZERO;

pub const ALPHA_OPAQUE: NormalizedF32 = NormalizedF32::ONE;

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq)]
pub struct ColorU8([u8; 4]);

impl ColorU8 {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the established public constructor accepts its channels in red, green, blue, alpha order"
    )
  )]
  pub const fn from_rgba(
    r: u8,
    g: u8,
    b: u8,
    a: u8,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(ColorU8([r, g, b, a]))
  }

  pub const fn red(self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.0[0])
  }

  pub const fn green(self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.0[1])
  }

  pub const fn blue(self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.0[2])
  }

  pub const fn alpha(self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.0[3])
  }

  pub fn is_opaque(
    &self,
  ) -> core::result::Result<crate::IsOpaqueAnswer, console_core_never::Never> {
    let decided = {
      let Ok(alpha) = self.alpha();

      alpha == ALPHA_U8_OPAQUE
    };

    Ok(match ::core::primitive::bool::then_some(decided, ()) {
      ::core::option::Option::Some(()) => crate::IsOpaqueAnswer::Yes,
      ::core::option::Option::None => crate::IsOpaqueAnswer::No,
    })
  }

  pub fn premultiply(&self) -> Result<PremultipliedColorU8, console_core_never::Never> {
    let [red, green, blue, alpha] = self.0;

    match alpha {
      ALPHA_U8_OPAQUE => PremultipliedColorU8::from_components(self.0),
      _ => {
        let opacity = Alpha(alpha);
        let Ok(red_2) = opacity.premultiply(red);
        let Ok(green_2) = opacity.premultiply(green);
        let Ok(blue_2) = opacity.premultiply(blue);

        PremultipliedColorU8::from_components([red_2, green_2, blue_2, alpha])
      }
    }
  }
}

impl core::fmt::Debug for ColorU8 {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("ColorU8")
      .field(
        "r",
        &({
          let Ok(explicit_value) = self.red();
          explicit_value
        }),
      )
      .field(
        "g",
        &({
          let Ok(explicit_value) = self.green();
          explicit_value
        }),
      )
      .field(
        "b",
        &({
          let Ok(explicit_value) = self.blue();
          explicit_value
        }),
      )
      .field(
        "a",
        &({
          let Ok(explicit_value) = self.alpha();
          explicit_value
        }),
      )
      .finish()
  }
}

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq)]
pub struct PremultipliedColorU8([u8; 4]);

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the transparent byte array has a valid all-zero representation, which is the transparent color"
  )
)]
unsafe impl bytemuck::Zeroable for PremultipliedColorU8 {}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the transparent wrapper has exactly four bytes, no padding and no invalid Rust bit pattern"
  )
)]
unsafe impl bytemuck::Pod for PremultipliedColorU8 {}

impl PremultipliedColorU8 {
  pub const TRANSPARENT: Self = Self([0; 4]);

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the established public constructor accepts its channels in red, green, blue, alpha order"
    )
  )]
  pub fn from_rgba(
    r: u8,
    g: u8,
    b: u8,
    a: u8,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(r <= a && g <= a && b <= a, ()) {
        ::core::option::Option::Some(()) => Some(PremultipliedColorU8([r, g, b, a])),
        ::core::option::Option::None => None,
      },
    )
  }

  pub(crate) const fn from_components(rgba: [u8; 4]) -> Result<Self, console_core_never::Never> {
    Ok(Self(rgba))
  }

  pub const fn red(self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.0[0])
  }

  pub const fn green(self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.0[1])
  }

  pub const fn blue(self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.0[2])
  }

  pub const fn alpha(self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.0[3])
  }

  pub fn is_opaque(
    &self,
  ) -> core::result::Result<crate::IsOpaqueAnswer, console_core_never::Never> {
    let decided = {
      let Ok(alpha) = self.alpha();

      alpha == ALPHA_U8_OPAQUE
    };

    Ok(match ::core::primitive::bool::then_some(decided, ()) {
      ::core::option::Option::Some(()) => crate::IsOpaqueAnswer::Yes,
      ::core::option::Option::None => crate::IsOpaqueAnswer::No,
    })
  }

  pub fn demultiply(&self) -> core::result::Result<ColorU8, console_core_never::Never> {
    let Ok(alpha) = self.alpha();

    Ok(
      match ::core::primitive::bool::then_some(alpha == ALPHA_U8_OPAQUE, ()) {
        ::core::option::Option::Some(()) => ColorU8(self.0),
        ::core::option::Option::None => {
          let a = f64::from(alpha) / 255.0;

          let Ok(red) = self.red();
          let Ok(color_u8) = ColorU8::from_rgba(
            {
              let Ok(numeric_value) =
                console_core_number_conversion::toward_zero_u8(f64::from(red) / a + 0.5);
              numeric_value
            },
            {
              let Ok(numeric_value) = console_core_number_conversion::toward_zero_u8(
                f64::from({
                  let Ok(explicit_value) = self.green();
                  explicit_value
                }) / a
                  + 0.5,
              );
              numeric_value
            },
            {
              let Ok(numeric_value) = console_core_number_conversion::toward_zero_u8(
                f64::from({
                  let Ok(explicit_value) = self.blue();
                  explicit_value
                }) / a
                  + 0.5,
              );
              numeric_value
            },
            alpha,
          );

          color_u8
        }
      },
    )
  }
}

impl core::fmt::Debug for PremultipliedColorU8 {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("PremultipliedColorU8")
      .field(
        "r",
        &({
          let Ok(explicit_value) = self.red();
          explicit_value
        }),
      )
      .field(
        "g",
        &({
          let Ok(explicit_value) = self.green();
          explicit_value
        }),
      )
      .field(
        "b",
        &({
          let Ok(explicit_value) = self.blue();
          explicit_value
        }),
      )
      .field(
        "a",
        &({
          let Ok(explicit_value) = self.alpha();
          explicit_value
        }),
      )
      .finish()
  }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Color {
  r: NormalizedF32,
  g: NormalizedF32,
  b: NormalizedF32,
  a: NormalizedF32,
}

const NV_ZERO: NormalizedF32 = NormalizedF32::ZERO;
const NV_ONE: NormalizedF32 = NormalizedF32::ONE;

impl Color {
  pub const TRANSPARENT: Color = Color {
    r: NV_ZERO,
    g: NV_ZERO,
    b: NV_ZERO,
    a: NV_ZERO,
  };

  pub const BLACK: Color = Color {
    r: NV_ZERO,
    g: NV_ZERO,
    b: NV_ZERO,
    a: NV_ONE,
  };

  pub const WHITE: Color = Color {
    r: NV_ONE,
    g: NV_ONE,
    b: NV_ONE,
    a: NV_ONE,
  };

  #[expect(
    clippy::missing_safety_doc,
    reason = "every component is clamped into range by NormalizedF32::nearest, so the caller is promising nothing; the unsafe stays because the signature is public"
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the established public compatibility constructor accepts its channels in RGBA order"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the public channel fractions are clamped to the normalized range within this constructor"
    )
  )]
  pub const unsafe fn from_rgba_unchecked(
    r: f32,
    g: f32,
    b: f32,
    a: f32,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(nearest) = NormalizedF32::nearest(r);
    let Ok(normalized_f32_nearest) = NormalizedF32::nearest(g);

    Ok(Color {
      r: nearest,
      g: normalized_f32_nearest,
      b: match NormalizedF32::nearest(b) {
        Ok(held) => held,
      },
      a: match NormalizedF32::nearest(a) {
        Ok(held) => held,
      },
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the established public constructor accepts its channels in red, green, blue, alpha order"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "each public channel is a dimensionless fraction that the constructor validates as NormalizedF32"
    )
  )]
  pub fn from_rgba(
    r: f32,
    g: f32,
    b: f32,
    a: f32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(Some(Color {
      r: match NormalizedF32::new(r) {
        Some(value) => value,
        None => return Ok(None),
      },
      g: match NormalizedF32::new(g) {
        Some(value) => value,
        None => return Ok(None),
      },
      b: match NormalizedF32::new(b) {
        Some(value) => value,
        None => return Ok(None),
      },
      a: match NormalizedF32::new(a) {
        Some(value) => value,
        None => return Ok(None),
      },
    }))
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the public byte constructor accepts the standard red, green, blue, alpha layout in order"
    )
  )]
  pub fn from_rgba8(
    r: u8,
    g: u8,
    b: u8,
    a: u8,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Color {
      r: NormalizedF32::new_u8(r),
      g: NormalizedF32::new_u8(g),
      b: NormalizedF32::new_u8(b),
      a: NormalizedF32::new_u8(a),
    })
  }

  pub fn red(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.r.get())
  }

  pub fn green(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.g.get())
  }

  pub fn blue(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.b.get())
  }

  pub fn alpha(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.a.get())
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the existing setter accepts the red channel as a normalized fraction and checks it before assignment"
    )
  )]
  pub fn set_red(&mut self, c: f32) -> core::result::Result<(), console_core_never::Never> {
    self.r = NormalizedF32::new_clamped(c);

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the existing setter accepts the green channel as a normalized fraction and checks it before assignment"
    )
  )]
  pub fn set_green(&mut self, c: f32) -> core::result::Result<(), console_core_never::Never> {
    self.g = NormalizedF32::new_clamped(c);

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the existing setter accepts the blue channel as a normalized fraction and checks it before assignment"
    )
  )]
  pub fn set_blue(&mut self, c: f32) -> core::result::Result<(), console_core_never::Never> {
    self.b = NormalizedF32::new_clamped(c);

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the existing setter accepts the alpha channel as a normalized fraction and checks it before assignment"
    )
  )]
  pub fn set_alpha(&mut self, c: f32) -> core::result::Result<(), console_core_never::Never> {
    self.a = NormalizedF32::new_clamped(c);

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "this public opacity multiplier is a dimensionless ratio applied to the current alpha"
    )
  )]
  pub fn apply_opacity(
    &mut self,
    opacity: f32,
  ) -> core::result::Result<(), console_core_never::Never> {
    self.a = NormalizedF32::new_clamped(self.a.get() * opacity.bound(0.0, 1.0));

    Ok(())
  }

  pub fn is_opaque(
    &self,
  ) -> core::result::Result<crate::IsOpaqueAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.a == ALPHA_OPAQUE, ()) {
        ::core::option::Option::Some(()) => crate::IsOpaqueAnswer::Yes,
        ::core::option::Option::None => crate::IsOpaqueAnswer::No,
      },
    )
  }

  pub fn premultiply(
    &self,
  ) -> core::result::Result<PremultipliedColor, console_core_never::Never> {
    let Ok(is_opaque) = self.is_opaque();

    Ok(match is_opaque {
      crate::IsOpaqueAnswer::Yes => PremultipliedColor {
        r: self.r,
        g: self.g,
        b: self.b,
        a: self.a,
      },
      crate::IsOpaqueAnswer::No => PremultipliedColor {
        r: NormalizedF32::new_clamped(self.r.get() * self.a.get()),
        g: NormalizedF32::new_clamped(self.g.get() * self.a.get()),
        b: NormalizedF32::new_clamped(self.b.get() * self.a.get()),
        a: self.a,
      },
    })
  }

  pub fn to_color_u8(&self) -> core::result::Result<ColorU8, console_core_never::Never> {
    let Ok(c) = color_f32_to_u8(self.r, self.g, self.b, self.a);

    let Ok(color_u8) = ColorU8::from_rgba(
      match c {
        [element, ..] => element,
      },
      match c {
        [_, element, ..] => element,
      },
      match c {
        [_, _, element, ..] => element,
      },
      match c {
        [_, _, _, element, ..] => element,
      },
    );

    Ok(color_u8)
  }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct PremultipliedColor {
  r: NormalizedF32,
  g: NormalizedF32,
  b: NormalizedF32,
  a: NormalizedF32,
}

impl PremultipliedColor {
  pub fn red(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.r.get())
  }

  pub fn green(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.g.get())
  }

  pub fn blue(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.b.get())
  }

  pub fn alpha(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.a.get())
  }

  pub fn demultiply(&self) -> core::result::Result<Color, console_core_never::Never> {
    let a = self.a.get();

    Ok(match ::core::primitive::f32::classify(a) {
      ::core::num::FpCategory::Zero => Color::TRANSPARENT,
      ::core::num::FpCategory::Nan
      | ::core::num::FpCategory::Infinite
      | ::core::num::FpCategory::Subnormal
      | ::core::num::FpCategory::Normal => Color {
        r: NormalizedF32::new_clamped(self.r.get() / a),
        g: NormalizedF32::new_clamped(self.g.get() / a),
        b: NormalizedF32::new_clamped(self.b.get() / a),
        a: self.a,
      },
    })
  }

  pub fn to_color_u8(
    &self,
  ) -> core::result::Result<PremultipliedColorU8, console_core_never::Never> {
    let Ok(c) = color_f32_to_u8(self.r, self.g, self.b, self.a);

    PremultipliedColorU8::from_components(c)
  }
}

#[derive(Clone, Copy)]
pub(crate) struct Alpha(pub u8);

impl Alpha {
  pub(crate) fn premultiply(self, channel: u8) -> Result<u8, console_core_never::Never> {
    let product = u32::from(channel)
      .wrapping_mul(u32::from(self.0))
      .wrapping_add(128);
    let rounded = product
      .wrapping_add(product.wrapping_shr(8))
      .wrapping_shr(8);

    console_core_number_conversion::wrapped::<u32, u8>(rounded)
  }
}

fn color_f32_to_u8(
  r: NormalizedF32,
  g: NormalizedF32,
  b: NormalizedF32,
  a: NormalizedF32,
) -> core::result::Result<[u8; 4], console_core_never::Never> {
  Ok({
    [
      ({
        let Ok(numeric_value) =
          console_core_number_conversion::toward_zero_u8(f64::from(r.get() * 255.0 + 0.5));
        numeric_value
      }),
      ({
        let Ok(numeric_value) =
          console_core_number_conversion::toward_zero_u8(f64::from(g.get() * 255.0 + 0.5));
        numeric_value
      }),
      ({
        let Ok(numeric_value) =
          console_core_number_conversion::toward_zero_u8(f64::from(b.get() * 255.0 + 0.5));
        numeric_value
      }),
      ({
        let Ok(numeric_value) =
          console_core_number_conversion::toward_zero_u8(f64::from(a.get() * 255.0 + 0.5));
        numeric_value
      }),
    ]
  })
}

#[derive(Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum ColorSpace {
  #[default]
  Linear,

  Gamma2,

  SimpleSRGB,

  FullSRGBGamma,
}

impl ColorSpace {
  pub(crate) fn expand_channel(
    self,
    x: NormalizedF32,
  ) -> core::result::Result<NormalizedF32, console_core_never::Never> {
    Ok(match self {
      ColorSpace::Linear => x,
      ColorSpace::Gamma2 => x * x,
      ColorSpace::SimpleSRGB => NormalizedF32::new_clamped({
        let Ok(explicit_value) = approx_powf(x, crate::math::GammaExponent(2.2));
        explicit_value
      }),
      ColorSpace::FullSRGBGamma => {
        let x_2 = x.get();
        let x_3 = match ::core::primitive::bool::then_some(x_2 <= 0.04045, ()) {
          ::core::option::Option::Some(()) => x_2 / 12.92,
          ::core::option::Option::None => {
            let normalized = NormalizedF32::new_clamped((x_2 + 0.055) / 1.055);
            let Ok(explicit_value) = approx_powf(normalized, crate::math::GammaExponent(2.4));
            explicit_value
          }
        };
        NormalizedF32::new_clamped(x_3)
      }
    })
  }

  pub(crate) fn expand_color(
    self,
    mut color: Color,
  ) -> core::result::Result<Color, console_core_never::Never> {
    let Ok(expand_channel) = self.expand_channel(color.r);
    color.r = expand_channel;
    let Ok(own_expand_channel) = self.expand_channel(color.g);
    color.g = own_expand_channel;
    color.b = {
      let Ok(explicit_value) = self.expand_channel(color.b);
      explicit_value
    };

    Ok(color)
  }

  pub(crate) fn expand_stage(
    self,
  ) -> core::result::Result<Option<pipeline::Stage>, console_core_never::Never> {
    Ok(match self {
      ColorSpace::Linear => None,
      ColorSpace::Gamma2 => Some(pipeline::Stage::GammaExpand2),
      ColorSpace::SimpleSRGB => Some(pipeline::Stage::GammaExpand22),
      ColorSpace::FullSRGBGamma => Some(pipeline::Stage::GammaExpandSrgb),
    })
  }

  pub(crate) fn expand_dest_stage(
    self,
  ) -> core::result::Result<Option<pipeline::Stage>, console_core_never::Never> {
    Ok(match self {
      ColorSpace::Linear => None,
      ColorSpace::Gamma2 => Some(pipeline::Stage::GammaExpandDestination2),
      ColorSpace::SimpleSRGB => Some(pipeline::Stage::GammaExpandDestination22),
      ColorSpace::FullSRGBGamma => Some(pipeline::Stage::GammaExpandDestinationSrgb),
    })
  }

  pub(crate) fn compress_stage(
    self,
  ) -> core::result::Result<Option<pipeline::Stage>, console_core_never::Never> {
    Ok(match self {
      ColorSpace::Linear => None,
      ColorSpace::Gamma2 => Some(pipeline::Stage::GammaCompress2),
      ColorSpace::SimpleSRGB => Some(pipeline::Stage::GammaCompress22),
      ColorSpace::FullSRGBGamma => Some(pipeline::Stage::GammaCompressSrgb),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn premultiply_u8() {
    let Ok(color) = ColorU8::from_rgba(10, 20, 30, 40);
    assert_eq!(
      color.premultiply(),
      PremultipliedColorU8::from_components([2, 3, 5, 40])
    );
  }

  #[test]
  fn premultiply_u8_opaque() {
    let Ok(color) = ColorU8::from_rgba(10, 20, 30, 255);
    assert_eq!(
      color.premultiply(),
      PremultipliedColorU8::from_components([10, 20, 30, 255])
    );
  }

  #[test]
  fn demultiply_u8_1() {
    let Ok(color) = PremultipliedColorU8::from_components([2, 3, 5, 40]);
    assert_eq!(color.demultiply(), ColorU8::from_rgba(13, 19, 32, 40));
  }

  #[test]
  fn demultiply_u8_2() {
    let Ok(color) = PremultipliedColorU8::from_components([10, 20, 30, 255]);
    assert_eq!(color.demultiply(), ColorU8::from_rgba(10, 20, 30, 255));
  }

  #[test]
  fn demultiply_u8_3() {
    let Ok(color) = PremultipliedColorU8::from_components([153, 99, 54, 180]);
    assert_eq!(color.demultiply(), ColorU8::from_rgba(217, 140, 77, 180));
  }

  #[test]
  fn bytemuck_casts_rgba() {
    let Ok(first) = PremultipliedColorU8::from_components([0, 1, 2, 3]);
    let Ok(second) = PremultipliedColorU8::from_components([10, 11, 12, 13]);
    let slice = &[first, second];
    let bytes: &[u8] = bytemuck::cast_slice(slice);
    assert_eq!(bytes, &[0, 1, 2, 3, 10, 11, 12, 13]);
  }
}

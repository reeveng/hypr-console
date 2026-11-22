#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CacheKeyFlags(u32);

impl CacheKeyFlags {
  pub const EMPTY: Self = Self(0);
  pub const FAKE_ITALIC: Self = Self(1);
  pub const DISABLE_HINTING: Self = Self(2);
  pub const PIXEL_FONT: Self = Self(4);

  pub const fn contains(
    self,
    wanted: Self,
  ) -> Result<crate::Presence, console_core_never::Never> {
    Ok(match wanted.0 & !self.0 {
      0 => crate::Presence::Present,
      _ => crate::Presence::Absent,
    })
  }
}

impl core::ops::BitOr for CacheKeyFlags {
  type Output = Self;

  fn bitor(self, other: Self) -> Self {
    Self(self.0 | other.0)
  }
}

impl core::ops::BitOrAssign for CacheKeyFlags {
  fn bitor_assign(&mut self, other: Self) {
    self.0 |= other.0;
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CacheKey {
  pub font_id: fontdb::ID,

  pub glyph_id: u16,

  pub font_size_bits: u32,

  pub x_bin: SubpixelBin,

  pub y_bin: SubpixelBin,

  pub font_weight: fontdb::Weight,

  pub flags: CacheKeyFlags,
}

impl CacheKey {
  pub fn new(
    font_id: fontdb::ID,
    glyph_id: u16,
    font_size: impl Into<crate::FontSize>,
    pos: (f32, f32),
    weight: fontdb::Weight,
    flags: CacheKeyFlags,
  ) -> Result<(Self, i32, i32), console_core_never::Never> {
    {
      let crate::FontSize(size) = Into::<crate::FontSize>::into(font_size);
      let Ok((x, x_bin)) = SubpixelBin::new(pos.0);
      let Ok((y, y_bin)) = SubpixelBin::new(pos.1);
      Ok((
        Self {
          font_id,
          glyph_id,
          font_size_bits: size.to_bits(),
          x_bin,
          y_bin,
          flags,
          font_weight: weight,
        },
        x,
        y,
      ))
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SubpixelBin {
  Zero,
  One,
  Two,
  Three,
}

impl SubpixelBin {
  pub fn new(
    pos: impl Into<crate::SubpixelPosition>,
  ) -> Result<(i32, Self), console_core_never::Never> {
    let crate::SubpixelPosition(position) = Into::<crate::SubpixelPosition>::into(pos);
    let Ok(trunc) = console_core_number_conversion::toward_zero_i32(f64::from(position));
    let Ok(integer) = console_core_number_conversion::narrow_f32(f64::from(trunc));
    let fraction = position - integer;

    Ok(match position.is_sign_negative().then_some(()) {
      Some(()) => match (
        fraction > -0.125,
        fraction > -0.375,
        fraction > -0.625,
        fraction > -0.875,
      ) {
        (true, _, _, _) => (trunc, Self::Zero),
        (false, true, _, _) => (trunc.saturating_sub(1), Self::Three),
        (false, false, true, _) => (trunc.saturating_sub(1), Self::Two),
        (false, false, false, true) => (trunc.saturating_sub(1), Self::One),
        (false, false, false, false) => (trunc.saturating_sub(1), Self::Zero),
      },
      None => match (
        fraction < 0.125,
        fraction < 0.375,
        fraction < 0.625,
        fraction < 0.875,
      ) {
        (true, _, _, _) => (trunc, Self::Zero),
        (false, true, _, _) => (trunc, Self::One),
        (false, false, true, _) => (trunc, Self::Two),
        (false, false, false, true) => (trunc, Self::Three),
        (false, false, false, false) => (trunc.saturating_add(1), Self::Zero),
      },
    })
  }

  pub const fn as_float(&self) -> Result<f32, console_core_never::Never> {
    {
      Ok(match self {
        Self::Zero => 0.0,
        Self::One => 0.25,
        Self::Two => 0.5,
        Self::Three => 0.75,
      })
    }
  }
}

#[test]
fn test_subpixel_bins() {
  assert_eq!(SubpixelBin::new(0.0), Ok((0, SubpixelBin::Zero)));
  assert_eq!(SubpixelBin::new(0.124), Ok((0, SubpixelBin::Zero)));

  assert_eq!(SubpixelBin::new(0.125), Ok((0, SubpixelBin::One)));
  assert_eq!(SubpixelBin::new(0.25), Ok((0, SubpixelBin::One)));
  assert_eq!(SubpixelBin::new(0.374), Ok((0, SubpixelBin::One)));

  assert_eq!(SubpixelBin::new(0.375), Ok((0, SubpixelBin::Two)));
  assert_eq!(SubpixelBin::new(0.5), Ok((0, SubpixelBin::Two)));
  assert_eq!(SubpixelBin::new(0.624), Ok((0, SubpixelBin::Two)));

  assert_eq!(SubpixelBin::new(0.625), Ok((0, SubpixelBin::Three)));
  assert_eq!(SubpixelBin::new(0.75), Ok((0, SubpixelBin::Three)));
  assert_eq!(SubpixelBin::new(0.874), Ok((0, SubpixelBin::Three)));

  assert_eq!(SubpixelBin::new(0.875), Ok((1, SubpixelBin::Zero)));
  assert_eq!(SubpixelBin::new(0.999), Ok((1, SubpixelBin::Zero)));
  assert_eq!(SubpixelBin::new(1.0), Ok((1, SubpixelBin::Zero)));
  assert_eq!(SubpixelBin::new(1.124), Ok((1, SubpixelBin::Zero)));

  assert_eq!(SubpixelBin::new(-0.0), Ok((0, SubpixelBin::Zero)));
  assert_eq!(SubpixelBin::new(-0.124), Ok((0, SubpixelBin::Zero)));

  assert_eq!(SubpixelBin::new(-0.125), Ok((-1, SubpixelBin::Three)));
  assert_eq!(SubpixelBin::new(-0.25), Ok((-1, SubpixelBin::Three)));
  assert_eq!(SubpixelBin::new(-0.374), Ok((-1, SubpixelBin::Three)));

  assert_eq!(SubpixelBin::new(-0.375), Ok((-1, SubpixelBin::Two)));
  assert_eq!(SubpixelBin::new(-0.5), Ok((-1, SubpixelBin::Two)));
  assert_eq!(SubpixelBin::new(-0.624), Ok((-1, SubpixelBin::Two)));

  assert_eq!(SubpixelBin::new(-0.625), Ok((-1, SubpixelBin::One)));
  assert_eq!(SubpixelBin::new(-0.75), Ok((-1, SubpixelBin::One)));
  assert_eq!(SubpixelBin::new(-0.874), Ok((-1, SubpixelBin::One)));

  assert_eq!(SubpixelBin::new(-0.875), Ok((-1, SubpixelBin::Zero)));
  assert_eq!(SubpixelBin::new(-0.999), Ok((-1, SubpixelBin::Zero)));
  assert_eq!(SubpixelBin::new(-1.0), Ok((-1, SubpixelBin::Zero)));
  assert_eq!(SubpixelBin::new(-1.124), Ok((-1, SubpixelBin::Zero)));
}

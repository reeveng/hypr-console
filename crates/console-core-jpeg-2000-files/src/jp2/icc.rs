#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub(crate) enum ICCColorSpace {
  Xyz,
  Lab,
  Luv,
  Ycbr,
  Yxy,
  Lms,
  Rgb,
  Gray,
  Hsv,
  Hls,
  Cmyk,
  Cmy,
  OneClr,
  ThreeClr,
  FourClr,
}

impl ICCColorSpace {
  pub(crate) fn num_components(&self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::Xyz => 3,
      Self::Lab => 3,
      Self::Luv => 3,
      Self::Ycbr => 3,
      Self::Yxy => 3,
      Self::Lms => 3,
      Self::Rgb => 3,
      Self::Gray => 1,
      Self::Hsv => 3,
      Self::Hls => 3,
      Self::Cmyk => 4,
      Self::Cmy => 3,
      Self::OneClr => 1,
      Self::ThreeClr => 3,
      Self::FourClr => 4,
    })
  }
}

impl ICCColorSpace {
  #[must_use]
  fn from_signature(value: u32) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(match value {
      0x58595A20 => Some(Self::Xyz),
      0x4C616220 => Some(Self::Lab),
      0x4C757620 => Some(Self::Luv),
      0x59436272 => Some(Self::Ycbr),
      0x59787920 => Some(Self::Yxy),
      0x4C4D5320 => Some(Self::Lms),
      0x52474220 => Some(Self::Rgb),
      0x47524159 => Some(Self::Gray),
      0x48535620 => Some(Self::Hsv),
      0x484C5320 => Some(Self::Hls),
      0x434D594B => Some(Self::Cmyk),
      0x434D5920 => Some(Self::Cmy),
      0x31434C52 => Some(Self::OneClr),
      0x33434C52 => Some(Self::ThreeClr),
      0x34434C52 => Some(Self::FourClr),
      _ => None,
    })
  }
}

#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub(crate) struct ICCMetadata {
  pub(crate) color_space: ICCColorSpace,
}

impl ICCMetadata {
  pub(crate) fn from_data(
    data: &[u8],
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let signature = match data.get(16..20) {
      Some(&[a, b, c, d]) => u32::from_be_bytes([a, b, c, d]),
      Some(_) | None => return Ok(None),
    };
    let Ok(parsed) = ICCColorSpace::from_signature(signature);
    Ok(parsed.map(|color_space| Self { color_space }))
  }
}

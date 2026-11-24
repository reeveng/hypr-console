use console_core_never::Never;
use moxcms::{
  ColorProfile, Layout, Transform8BitExecutor, TransformF32Executor, TransformOptions, Xyzd,
};
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, Copy)]
pub enum Channels {
  Gray,
  Rgb,
  Cmyk,
}

impl Channels {
  pub fn count(self) -> Result<u8, Never> {
    Ok(match self {
      Self::Gray => 1,
      Self::Rgb => 3,
      Self::Cmyk => 4,
    })
  }

  fn layout(self) -> Result<Layout, Never> {
    Ok(match self {
      Self::Gray => Layout::Gray,
      Self::Rgb => Layout::Rgb,
      Self::Cmyk => Layout::Rgba,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformError {
  InvalidProfile,
  UnsupportedTransform,
  InvalidSamples,
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "moxcms returns executors behind Arc and the transform owns those handles"
  )
)]
pub struct ToSrgb {
  profile: ColorProfile,
  channels: Channels,
  bytes: Arc<Transform8BitExecutor>,
  floats: OnceLock<Result<Arc<TransformF32Executor>, TransformError>>,
}

impl ToSrgb {
  pub fn new(data: &[u8], channels: Channels) -> Result<Self, TransformError> {
    let parsed = ColorProfile::new_from_slice(data).map_err(|_| TransformError::InvalidProfile);
    let profile = parsed?;

    Self::from_profile(profile, channels)
  }

  pub fn for_lab(white_point: [f64; 3]) -> Result<Self, TransformError> {
    let parsed =
      ColorProfile::new_from_slice(crate::LAB).map_err(|_| TransformError::InvalidProfile);
    let mut profile = parsed?;
    let [x, y, z] = white_point;
    profile.white_point = Xyzd::new(x, y, z);

    Self::from_profile(profile, Channels::Rgb)
  }

  fn from_profile(profile: ColorProfile, channels: Channels) -> Result<Self, TransformError> {
    let Ok(layout) = channels.layout();
    let srgb = ColorProfile::new_srgb();
    let compiled = profile
      .create_transform_8bit(layout, &srgb, Layout::Rgb, TransformOptions::default())
      .map_err(|_| TransformError::UnsupportedTransform);
    let bytes = compiled?;

    Ok(Self {
      profile,
      channels,
      bytes,
      floats: OnceLock::new(),
    })
  }

  pub fn channels(&self) -> Result<Channels, Never> {
    Ok(self.channels)
  }

  pub fn transform_u8(&self, input: &[u8], output: &mut [u8]) -> Result<(), TransformError> {
    self
      .bytes
      .transform(input, output)
      .map_err(|_| TransformError::InvalidSamples)
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "these slices contain normalized ICC input and encoded sRGB components"
    )
  )]
  pub fn transform_f32(&self, input: &[f32], output: &mut [f32]) -> Result<(), TransformError> {
    let transform = self.floats.get_or_init(|| {
      let Ok(layout) = self.channels.layout();
      let srgb = ColorProfile::new_srgb();

      self
        .profile
        .create_transform_f32(layout, &srgb, Layout::Rgb, TransformOptions::default())
        .map_err(|_| TransformError::UnsupportedTransform)
    });
    let compiled = transform.as_ref().map_err(|error| *error);
    let executor = compiled?;

    executor
      .transform(input, output)
      .map_err(|_| TransformError::InvalidSamples)
  }
}

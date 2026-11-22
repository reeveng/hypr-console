//! Integration with the [image] crate

use std::{
  ffi::OsStr,
  io::{BufRead, Seek},
};

use crate::{AlphaChannel, ColorSpace, DecodeSettings, Image};
use ::image::error::{DecodingError, ImageFormatHint};
use ::image::{ColorType, ExtendedColorType, ImageDecoder, ImageError, ImageResult};
use image::hooks::{decoding_hook_registered, register_format_detection_hook};
use moxcms::{CmsError, ColorProfile, Layout, TransformOptions};

const CMYK_PROFILE: &[u8] = include_bytes!("../assets/CGATS001Compat-v2-micro.icc");

impl ImageDecoder for Image<'_> {
  fn dimensions(&self) -> (u32, u32) {
    let Ok(width) = self.width();
    let Ok(height) = self.height();

    (width, height)
  }

  fn color_type(&self) -> ColorType {
    let Ok(channel_count) = self.color_space.num_channels();
    let has_alpha = self.has_alpha;

    match (channel_count, has_alpha) {
      (1, AlphaChannel::Absent) => ColorType::L8,
      (1, AlphaChannel::Present) => ColorType::La8,
      (3, AlphaChannel::Absent) => ColorType::Rgb8,
      (3, AlphaChannel::Present) => ColorType::Rgba8,

      (4, AlphaChannel::Absent) => ColorType::Rgb8,
      (4, AlphaChannel::Present) => ColorType::Rgba8,

      _ => ColorType::Rgb8,
    }
  }

  fn original_color_type(&self) -> ExtendedColorType {
    let Ok(channel_count) = self.color_space.num_channels();
    let has_alpha = self.has_alpha;
    let Ok(depth) = self.original_bit_depth();

    match (channel_count, depth, has_alpha) {
      (1, 1, AlphaChannel::Absent) => ExtendedColorType::L1,
      (1, 1, AlphaChannel::Present) => ExtendedColorType::La1,
      (1, 2, AlphaChannel::Absent) => ExtendedColorType::L2,
      (1, 2, AlphaChannel::Present) => ExtendedColorType::La2,
      (1, 4, AlphaChannel::Absent) => ExtendedColorType::L4,
      (1, 4, AlphaChannel::Present) => ExtendedColorType::La4,
      (1, 8, AlphaChannel::Absent) => ExtendedColorType::L8,
      (1, 8, AlphaChannel::Present) => ExtendedColorType::La8,
      (1, 16, AlphaChannel::Absent) => ExtendedColorType::L8,
      (1, 16, AlphaChannel::Present) => ExtendedColorType::La8,

      (3, 1, AlphaChannel::Absent) => ExtendedColorType::Rgb1,
      (3, 1, AlphaChannel::Present) => ExtendedColorType::Rgba1,
      (3, 2, AlphaChannel::Absent) => ExtendedColorType::Rgb2,
      (3, 2, AlphaChannel::Present) => ExtendedColorType::Rgba2,
      (3, 4, AlphaChannel::Absent) => ExtendedColorType::Rgb4,
      (3, 4, AlphaChannel::Present) => ExtendedColorType::Rgba4,
      (3, 8, AlphaChannel::Absent) => ExtendedColorType::Rgb8,
      (3, 8, AlphaChannel::Present) => ExtendedColorType::Rgba8,
      (3, 16, AlphaChannel::Absent) => ExtendedColorType::Rgb8,
      (3, 16, AlphaChannel::Present) => ExtendedColorType::Rgba8,

      (4, 8, AlphaChannel::Absent) => ExtendedColorType::Cmyk8,
      (4, 16, AlphaChannel::Absent) => ExtendedColorType::Cmyk16,

      _ => ExtendedColorType::Unknown(match orig_bits_per_pixel(self) {
        Ok(value) => value,
      }),
    }
  }

  fn read_image(self, buf: &mut [u8]) -> ImageResult<()>
  where
    Self: Sized,
  {
    convert_inner(&self, buf)
  }

  fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
    (*self).read_image(buf)
  }
}

pub struct Jp2Decoder {
  input: Vec<u8>,
  width: u32,
  height: u32,
  color_type: ColorType,
  orig_color_type: ExtendedColorType,
}

impl Jp2Decoder {
  pub fn new<R: BufRead + Seek>(r: R) -> ImageResult<Self> {
    const IMAGE_BYTES: u32 = u32::wrapping_mul(u32::wrapping_mul(256, 1024), 1024);

    let input =
      console_core_bounded_reads::bytes(r, IMAGE_BYTES).map_err(std::io::Error::from)?;
    let headers = Image::new(&input, &DecodeSettings::default())?;
    let Ok(width) = headers.width();
    let Ok(height) = headers.height();

    Ok(Self {
      width,
      height,
      color_type: headers.color_type(),
      orig_color_type: headers.original_color_type(),
      input,
    })
  }
}

impl ImageDecoder for Jp2Decoder {
  fn dimensions(&self) -> (u32, u32) {
    (self.width, self.height)
  }

  fn color_type(&self) -> ColorType {
    self.color_type
  }

  fn original_color_type(&self) -> ExtendedColorType {
    self.orig_color_type
  }

  fn read_image(self, buf: &mut [u8]) -> ImageResult<()>
  where
    Self: Sized,
  {
    let decoder = Image::new(&self.input, &DecodeSettings::default())?;

    decoder.read_image(buf)
  }

  fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
    (*self).read_image(buf)
  }
}

fn orig_bits_per_pixel(img: &Image<'_>) -> core::result::Result<u8, console_core_never::Never> {
  let Ok(color_space) = img.color_space();
  let Ok(mut channel_count) = color_space.num_channels();

  match img.has_alpha {
    AlphaChannel::Present => channel_count = channel_count.wrapping_add(1),
    AlphaChannel::Absent => {}
  }

  let Ok(original_bit_depth) = img.original_bit_depth();

  Ok(u8::wrapping_mul(channel_count, original_bit_depth))
}

fn convert_inner(image: &Image<'_>, buf: &mut [u8]) -> ImageResult<()> {
  let Ok(color_space) = image.color_space();
  let Ok(alpha) = image.has_alpha();

  match color_space {
    ColorSpace::Gray | ColorSpace::RGB => decode_direct(image, buf),
    ColorSpace::CMYK => decode_cmyk(image, buf, alpha),
    ColorSpace::Icc {
      profile,
      num_channels,
    } => decode_icc(
      image,
      buf,
      profile,
      ProfileChannels {
        colors: *num_channels,
        alpha,
      },
    ),
    ColorSpace::Unknown { .. } => {
      let Ok(error) = unsupported_color_error(image.original_color_type());
      Err(error)
    }
  }
}

fn decode_direct(image: &Image<'_>, buf: &mut [u8]) -> ImageResult<()> {
  let mut decoder_context = crate::DecoderContext::default();
  image.decode_into(buf, &mut decoder_context)?;
  Ok(())
}

fn decode_cmyk(image: &Image<'_>, buf: &mut [u8], alpha: AlphaChannel) -> ImageResult<()> {
  let decoded = image.decode()?;
  let channels = ProfileChannels {
    colors: 4,
    alpha: AlphaChannel::Absent,
  };

  match alpha {
    AlphaChannel::Absent => {
      let transformed = transform_profile(CMYK_PROFILE, channels, &decoded).map_err(|error| {
        let Ok(converted) = icc_err_to_image(error);
        converted
      })?;
      copy_pixels(buf, &transformed)
    }
    AlphaChannel::Present => {
      let (samples, _) = decoded.as_chunks::<5>();
      let mut cmyk = Vec::new();

      for &[c, m, y, k, _] in samples {
        cmyk.extend_from_slice(&[c, m, y, k]);
      }

      let rgb = transform_profile(CMYK_PROFILE, channels, &cmyk).map_err(|error| {
        let Ok(converted) = icc_err_to_image(error);
        converted
      })?;

      for ((output, &[r, g, b]), &[_, _, _, _, a]) in buf
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(rgb.as_chunks::<3>().0)
        .zip(samples)
      {
        *output = [r, g, b, a];
      }

      Ok(())
    }
  }
}

fn decode_icc(
  image: &Image<'_>,
  buf: &mut [u8],
  profile: &[u8],
  channels: ProfileChannels,
) -> ImageResult<()> {
  let decoded = image.decode()?;

  match transform_profile(profile, channels, &decoded) {
    Ok(converted) => copy_pixels(buf, &converted),
    Err(error) => {
      let Ok(()) = crate::log::warn(format_args!("JPEG 2000 ICC transform failed: {error}"));

      match channels.colors {
        1 | 3 => decode_direct(image, buf),
        4 => decode_cmyk(image, buf, channels.alpha),
        _ => {
          let Ok(unsupported) = unsupported_color_error(image.original_color_type());
          Err(unsupported)
        }
      }
    }
  }
}

#[derive(Clone, Copy)]
struct ProfileChannels {
  colors: u8,
  alpha: AlphaChannel,
}

fn transform_profile(
  icc: &[u8],
  channels: ProfileChannels,
  input: &[u8],
) -> Result<Vec<u8>, CmsError> {
  let source = ColorProfile::new_from_slice(icc)?;
  let destination = ColorProfile::new_srgb();

  let (src_layout, dst_layout, src_channels, dst_channels) =
    match (channels.colors, channels.alpha) {
      (1, AlphaChannel::Absent) => (Layout::Gray, Layout::Gray, 1u8, 1u8),
      (1, AlphaChannel::Present) => (Layout::GrayAlpha, Layout::GrayAlpha, 2, 2),
      (3, AlphaChannel::Absent) => (Layout::Rgb, Layout::Rgb, 3, 3),
      (3, AlphaChannel::Present) => (Layout::Rgba, Layout::Rgba, 4, 4),
      (4, AlphaChannel::Absent) => (Layout::Rgba, Layout::Rgb, 4, 3),
      _ => return Err(CmsError::UnsupportedChannelConfiguration),
    };
  let transform = source.create_transform_8bit(
    src_layout,
    &destination,
    dst_layout,
    TransformOptions::default(),
  )?;
  let Ok(input_channels) = console_core_number_conversion::index(src_channels);
  let Ok(output_channels) = console_core_number_conversion::index(dst_channels);
  let mut transformed = vec![
    0;
    input
      .len()
      .wrapping_div(input_channels.max(1))
      .wrapping_mul(output_channels)
      .min(input.len())
  ];
  transform.transform(input, &mut transformed)?;
  Ok(transformed)
}

fn copy_pixels(output: &mut [u8], pixels: &[u8]) -> ImageResult<()> {
  match output.len().cmp(&pixels.len()) {
    core::cmp::Ordering::Equal => {}
    core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
      return Err(ImageError::Parameter(
        ::image::error::ParameterError::from_kind(
          ::image::error::ParameterErrorKind::DimensionMismatch,
        ),
      ));
    }
  }

  for (out, pixel) in output.iter_mut().zip(pixels) {
    *out = *pixel;
  }

  Ok(())
}

impl From<crate::DecodeError> for DecodingError {
  fn from(value: crate::DecodeError) -> Self {
    let format = ImageFormatHint::Name("JPEG2000".to_owned());
    Self::new(format, value)
  }
}

impl From<crate::DecodeError> for ImageError {
  fn from(value: crate::DecodeError) -> Self {
    Self::Decoding(<::image::error::DecodingError as ::core::convert::From<
      crate::error::DecodeError,
    >>::from(value))
  }
}

fn icc_err_to_image(
  err: CmsError,
) -> core::result::Result<ImageError, console_core_never::Never> {
  let format = ImageFormatHint::Name("JPEG2000".to_owned());

  Ok(ImageError::Decoding(DecodingError::new(format, err)))
}

fn unsupported_color_error(
  color: ExtendedColorType,
) -> core::result::Result<ImageError, console_core_never::Never> {
  Ok(ImageError::Unsupported(
    image::error::UnsupportedError::from_format_and_kind(
      ImageFormatHint::Name("JPEG2000".to_owned()),
      image::error::UnsupportedErrorKind::Color(color),
    ),
  ))
}

pub fn register_decoding_hook()
-> core::result::Result<crate::states::Registration, console_core_never::Never> {
  match ::core::primitive::bool::then_some(decoding_hook_registered(OsStr::new("jp2")), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(crate::states::Registration::AlreadyPresent);
    }
    ::core::option::Option::None => {}
  }

  for extension in ["jp2", "jpg2", "j2k", "jpf"] {
    image::hooks::register_decoding_hook(
      <::std::ffi::OsString as ::core::convert::From<&::core::primitive::str>>::from(extension),
      Box::new(|r| {
        let jp2decoder = Jp2Decoder::new(r)?;

        Ok(Box::new(jp2decoder))
      }),
    );
    register_format_detection_hook(
      <::std::ffi::OsString as ::core::convert::From<&::core::primitive::str>>::from(extension),
      crate::JP2_MAGIC,
      None,
    );
  }

  for extension in ["j2c", "jpc"] {
    image::hooks::register_decoding_hook(
      <::std::ffi::OsString as ::core::convert::From<&::core::primitive::str>>::from(extension),
      Box::new(|r| {
        let jp2decoder = Jp2Decoder::new(r)?;

        Ok(Box::new(jp2decoder))
      }),
    );
    register_format_detection_hook(
      <::std::ffi::OsString as ::core::convert::From<&::core::primitive::str>>::from(extension),
      crate::CODESTREAM_MAGIC,
      None,
    );
  }

  Ok(crate::states::Registration::Installed)
}

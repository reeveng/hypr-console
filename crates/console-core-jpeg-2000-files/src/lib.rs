/*!
A memory-safe, pure-Rust JPEG 2000 decoder.

`console_core_portable_document_rasterizer-jpeg2000` can decode both raw JPEG 2000 codestreams (`.j2c`) and images wrapped
inside the JP2 container format. The decoder supports the vast majority of features
defined in the JPEG2000 core coding system (ISO/IEC 15444-1) as well as some color
spaces from the extensions (ISO/IEC 15444-2). There are still some missing pieces
for some "obscure" features(like for example support for progression order
changes in tile-parts), but all features that actually commonly appear in real-life
images should be supported (if not, please open an issue!).

The decoder abstracts away most of the internal complexity of JPEG2000
and yields a simple 8-bit image with either greyscale, RGB, CMYK or an ICC-based
color space, which can then be processed further according to your needs.

# Example
```rust,no_run
use console_core_jpeg_2000_files::{Image, DecodeSettings};
# fn main() -> Result<(), Box<dyn std::error::Error>> {

let data = std::fs::read("image.jp2")?;
let image = Image::new(&data, &DecodeSettings::default())?;
let Ok(width) = image.width();
let Ok(height) = image.height();
let Ok(color_space) = image.color_space();
let Ok(alpha) = image.has_alpha();

println!("{width}x{height} image in {color_space:?} with alpha={alpha:?}");

let bitmap = image.decode()?;
# Ok(())
# }
```

If you want to see a more comprehensive example, please take a look
at the example in [GitHub](https://github.com/LaurenzV/console_core_portable_document_rasterizer/blob/main/console_core_portable_document_rasterizer-jpeg2000/examples/png.rs),
which shows you the main steps needed to convert a JPEG2000 image into PNG for example.

# Testing
The decoder has been tested against 20.000+ images scraped from random PDFs
on the internet and also passes a large part of the `OpenJPEG` test suite. So you
can expect the crate to perform decently in terms of decoding correctness.

# Performance
A decent amount of effort has already been put into optimizing this crate
(both in terms of raw performance but also memory allocations). However, there
are some more important optimizations that have not been implemented yet, so
there is definitely still room for improvement (and I am planning on implementing
them eventually).

Overall, you should expect this crate to have worse performance than `OpenJPEG`,
but the difference gap should not be too large.

# Safety
By default, the crate has the `simd` feature enabled, which uses the
shared SIMD dispatcher to accelerate
important parts of the pipeline. If you want to eliminate any usage of unsafe
in this crate as well as its dependencies, you can simply disable this
feature, at the cost of worse decoding performance. Unsafe code is forbidden
via a crate-level attribute.

The crate is `no_std` compatible but requires an allocator to be available.
*/

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::vec::Vec;
use console_core_never::Never;
use console_core_number_conversion::{
  checked_index, fixed_index, index, narrow_f32, toward_zero_u8,
};

use crate::j2c::{ComponentData, Header};
use crate::jp2::cdef::{ChannelAssociation, ChannelType};
use crate::jp2::cmap::ComponentMappingType;
use crate::jp2::colr::{CieLab, EnumeratedColorspace};
use crate::jp2::icc::ICCMetadata;
use crate::jp2::{DecodedImage, ImageBoxes};

pub mod error;
pub(crate) mod log;
pub(crate) mod math;

use crate::math::SIMD_WIDTH;
pub use error::{
  ColorError, DecodeError, DecodingError, FormatError, MarkerError, Result, TileError,
  ValidationError,
};
pub use j2c::DecoderContext;

#[cfg(feature = "image")]
pub mod integration;
mod j2c;
mod jp2;
pub(crate) mod states;

pub use states::{
  AlphaChannel, EphMarkers, NextStripeNeighbor, PaletteMode, Precincts, ReaderEnd, Registration,
  Significance, SopMarkers, StuffBit, ValidationMode, ZeroCoding,
};

mod reader;

pub(crate) const JP2_MAGIC: &[u8] = b"\x00\x00\x00\x0C\x6A\x50\x20\x20";

pub(crate) const MAX_IMAGE_SIDE: u64 = 60_000;
pub(crate) const MAX_COMPONENT_SAMPLES: u64 = MAX_IMAGE_SIDE.wrapping_mul(MAX_IMAGE_SIDE);
pub(crate) const MAX_TILE_COEFFICIENTS: u64 = MAX_COMPONENT_SAMPLES.wrapping_mul(65_535);
const MAX_OUTPUT_BYTES: u64 = MAX_COMPONENT_SAMPLES.wrapping_mul(256);

#[derive(Clone, Copy)]
pub(crate) enum DecoderBuffer {
  Output,
  Component,
  Tile,
  Block,
  TagTree,
}

#[cfg_attr(
  dylint_lib = "explicit088_no_arriving_length",
  expect(
    explicit088_no_arriving_length,
    reason = "this shared allocator checks its format-specific ceiling before converting the element count and trying to reserve memory"
  )
)]
pub(crate) fn resize_decoder_buffer<T: Clone>(
  buffer: &mut Vec<T>,
  length: u64,
  purpose: DecoderBuffer,
  fill: T,
) -> Result<()> {
  let limit = match purpose {
    DecoderBuffer::Output => MAX_OUTPUT_BYTES,
    DecoderBuffer::Component => MAX_COMPONENT_SAMPLES.wrapping_add(8),
    DecoderBuffer::Tile => MAX_TILE_COEFFICIENTS,
    DecoderBuffer::Block => MAX_IMAGE_SIDE.wrapping_add(2).wrapping_pow(2),
    DecoderBuffer::TagTree => MAX_TILE_COEFFICIENTS.wrapping_mul(66),
  };

  match length.cmp(&limit) {
    core::cmp::Ordering::Greater => {
      return Err(DecodeError::Validation(ValidationError::ImageTooLarge));
    }
    core::cmp::Ordering::Equal | core::cmp::Ordering::Less => {}
  }

  let count = checked_index(length).map_err(|_overflow| ValidationError::ImageTooLarge)?;
  let Ok(current) = fixed_index(buffer.len());
  let additional = checked_index(length.saturating_sub(current))
    .map_err(|_overflow| ValidationError::ImageTooLarge)?;
  buffer
    .try_reserve(additional)
    .map_err(|_allocation| ValidationError::ImageTooLarge)?;
  buffer.resize(count, fill);
  Ok(())
}

pub(crate) const CODESTREAM_MAGIC: &[u8] = b"\xFF\x4F\xFF\x51";

#[derive(Debug, Copy, Clone)]
pub struct DecodeSettings {
  pub palette: PaletteMode,

  pub validation: ValidationMode,

  pub target_resolution: Option<(u32, u32)>,
}

impl Default for DecodeSettings {
  fn default() -> Self {
    Self {
      palette: PaletteMode::ResolveColors,
      validation: ValidationMode::Lenient,
      target_resolution: None,
    }
  }
}

pub struct Image<'a> {
  pub(crate) codestream: &'a [u8],

  pub(crate) header: Header<'a>,

  pub(crate) boxes: ImageBoxes,

  pub(crate) settings: DecodeSettings,

  pub(crate) has_alpha: AlphaChannel,

  pub(crate) color_space: ColorSpace,
}

impl<'a> Image<'a> {
  pub fn new(data: &'a [u8], settings: &DecodeSettings) -> Result<Self> {
    match ::core::primitive::bool::then_some(data.starts_with(JP2_MAGIC), ()) {
      ::core::option::Option::Some(()) => jp2::parse(data, *settings),
      ::core::option::Option::None => {
        match ::core::primitive::bool::then_some(data.starts_with(CODESTREAM_MAGIC), ()) {
          ::core::option::Option::Some(()) => j2c::parse(data, settings),
          ::core::option::Option::None => {
            Err(<crate::error::DecodeError as ::core::convert::From<
              crate::error::FormatError,
            >>::from(FormatError::InvalidSignature))
          }
        }
      }
    }
  }

  pub fn has_alpha(
    &self,
  ) -> core::result::Result<crate::states::AlphaChannel, console_core_never::Never> {
    Ok(self.has_alpha)
  }

  pub fn color_space(&self) -> core::result::Result<&ColorSpace, console_core_never::Never> {
    Ok(&self.color_space)
  }

  pub fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(image_width) = self.header.size_data.image_width();

    Ok(image_width)
  }

  pub fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(image_height) = self.header.size_data.image_height();

    Ok(image_height)
  }

  #[cfg_attr(
    dylint_lib = "explicit014_no_index_slice",
    expect(
      explicit014_no_index_slice,
      reason = "the SIZ marker rejects zero image components before constructing Image"
    )
  )]
  pub fn original_bit_depth(&self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.header.component_infos[0].size_info.precision)
  }

  pub fn decode(&self) -> Result<Vec<u8>> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let Ok(colors) = self.color_space.num_channels();
    let extra_channel = match self.has_alpha {
      AlphaChannel::Present => 1,
      AlphaChannel::Absent => 0,
    };
    let channels = u64::from(colors).wrapping_add(extra_channel);
    let byte_count = u64::from(width)
      .wrapping_mul(u64::from(height))
      .wrapping_mul(channels);
    let mut buf = Vec::new();
    resize_decoder_buffer(&mut buf, byte_count, DecoderBuffer::Output, 0)?;

    let mut decoder_context = DecoderContext::default();
    self.decode_into(&mut buf, &mut decoder_context)?;

    Ok(buf)
  }

  pub fn decode_into(
    &'a self,
    buf: &mut [u8],
    decoder_context: &mut DecoderContext<'a>,
  ) -> Result<()> {
    let settings = &self.settings;
    j2c::decode(self.codestream, &self.header, decoder_context)?;
    let mut decoded_image = DecodedImage {
      decoded_components: &mut decoder_context.channel_data,
      boxes: self.boxes.clone(),
    };

    match settings.palette {
      PaletteMode::ResolveColors => {
        let components = core::mem::take(decoded_image.decoded_components);
        let resolved = resolve_palette_indices(components, &decoded_image.boxes)?;
        *decoded_image.decoded_components = resolved;
      }
      PaletteMode::KeepIndices => {}
    }

    match &decoded_image.boxes.channel_definition {
      Some(cdef) => {
        let mut components = decoded_image
          .decoded_components
          .iter()
          .cloned()
          .zip(
            cdef
              .channel_definitions
              .iter()
              .map(|c| match c._association {
                ChannelAssociation::WholeImage => u16::MAX,
                ChannelAssociation::Colour(c_2) => c_2,
              }),
          )
          .collect::<Vec<_>>();
        components.sort_by_key(|component| component.1);
        *decoded_image.decoded_components = components.into_iter().map(|c| c.0).collect();
      }
      None => {}
    }

    let first = decoded_image
      .decoded_components
      .first()
      .ok_or(ValidationError::InvalidComponentMetadata)?;
    let bit_depth = first.bit_depth;
    convert_color_space(&mut decoded_image, bit_depth)?;
    interleave_and_convert(decoded_image.decoded_components, buf)
  }
}

pub(crate) fn resolve_alpha_and_color_space(
  boxes: &ImageBoxes,
  header: &Header<'_>,
  settings: &DecodeSettings,
) -> Result<(ColorSpace, AlphaChannel)> {
  let Ok(actual_components) = fixed_index(header.component_infos.len());
  let color_components = match (settings.palette, &boxes.palette) {
    (PaletteMode::ResolveColors, Some(palette)) => {
      let Ok(count) = fixed_index(palette.columns.len());
      count
    }
    _ => actual_components,
  };

  let mut has_alpha = match boxes
    .channel_definition
    .as_ref()
    .and_then(|box_data| box_data.channel_definitions.last())
    .map(|definition| definition.channel_type)
  {
    Some(ChannelType::Opacity) => AlphaChannel::Present,
    Some(ChannelType::Colour) | None => AlphaChannel::Absent,
  };
  let mut color_space = get_color_space(boxes, color_components)?;

  match (settings.palette, &boxes.palette) {
    (PaletteMode::KeepIndices, Some(_)) => {
      return Ok((ColorSpace::Gray, AlphaChannel::Absent));
    }
    (PaletteMode::ResolveColors, Some(_)) => return Ok((color_space, has_alpha)),
    (_, None) => {}
  }

  let Ok(color_channels) = color_space.num_channels();
  let expected_components = u64::from(color_channels).wrapping_add(match has_alpha {
    AlphaChannel::Present => 1,
    AlphaChannel::Absent => 0,
  });

  match actual_components.cmp(&expected_components) {
    core::cmp::Ordering::Equal => {}
    core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
      match (
        settings.validation,
        has_alpha,
        actual_components.checked_sub(u64::from(color_channels)),
      ) {
        (ValidationMode::Lenient, AlphaChannel::Absent, Some(1)) => {
          has_alpha = AlphaChannel::Present;
        }
        _ => match (actual_components, has_alpha) {
          (1, _) | (2, AlphaChannel::Present) => color_space = ColorSpace::Gray,
          (3, _) | (4, AlphaChannel::Present) => color_space = ColorSpace::RGB,
          (4, AlphaChannel::Absent) => color_space = ColorSpace::CMYK,
          _ => {
            return Err(<crate::error::DecodeError as ::core::convert::From<
              crate::error::ValidationError,
            >>::from(ValidationError::TooManyChannels));
          }
        },
      }
    }
  }

  Ok((color_space, has_alpha))
}

#[derive(Debug, Clone)]
pub enum ColorSpace {
  Gray,

  RGB,

  CMYK,

  Unknown { num_channels: u8 },

  Icc { profile: Vec<u8>, num_channels: u8 },
}

impl ColorSpace {
  pub fn num_channels(&self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::Gray => 1,
      Self::RGB => 3,
      Self::CMYK => 4,
      Self::Unknown { num_channels } => *num_channels,
      Self::Icc {
        num_channels: num_components,
        ..
      } => *num_components,
    })
  }
}

pub struct Bitmap {
  pub color_space: ColorSpace,

  pub data: Vec<u8>,

  pub has_alpha: AlphaChannel,

  pub width: u32,

  pub height: u32,

  pub original_bit_depth: u8,
}

fn interleave_and_convert(components: &[ComponentData], buf: &mut [u8]) -> Result<()> {
  let first = components
    .first()
    .ok_or(ValidationError::InvalidComponentMetadata)?;
  let first_samples = first.container.truncated()?;
  let Ok(channel_count) = fixed_index(components.len());
  let Ok(pixel_count) = fixed_index(first_samples.len());
  let expected = pixel_count
    .checked_mul(channel_count)
    .ok_or(ValidationError::ImageTooLarge)?;
  let end = checked_index(expected).map_err(|_overflow| ValidationError::ImageTooLarge)?;
  let output = buf
    .get_mut(..end)
    .ok_or(ValidationError::InvalidDimensions)?;
  let Ok(stride) = index(channel_count);

  let unscaled = (components.len() <= 4
    && components.iter().all(|channel| channel.bit_depth == 8))
  .then_some(());
  let mut raster = Vec::with_capacity(components.len());

  for channel in components {
    let samples = channel.container.truncated()?;

    match samples.len().cmp(&first_samples.len()) {
      core::cmp::Ordering::Equal => {}
      core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
        return Err(DecodeError::Validation(
          ValidationError::InvalidComponentMetadata,
        ));
      }
    }

    let denominator = match unscaled {
      Some(()) => None,
      None => {
        let maximum = 1u32
          .wrapping_shl(u32::from(channel.bit_depth))
          .wrapping_sub(1);
        let Ok(converted) = narrow_f32(f64::from(maximum));
        Some(converted)
      }
    };

    raster.push((samples, denominator));
  }

  for (pixel, bytes) in output.chunks_exact_mut(stride.max(1)).enumerate() {
    for (out, (samples, denominator)) in bytes.iter_mut().zip(&raster) {
      let raw = ::core::result::Result::<&_, _>::copied(
        samples.get(pixel).ok_or(ValidationError::InvalidDimensions),
      )?;
      let normalized = match denominator {
        Some(maximum) => (raw / maximum) * 255.0,
        None => raw,
      };
      let Ok(rounded) = math::round_f32(math::Sample(normalized));
      let Ok(byte) = toward_zero_u8(f64::from(rounded));
      *out = byte;
    }
  }

  Ok(())
}

fn convert_color_space(image: &mut DecodedImage<'_>, bit_depth: u8) -> Result<()> {
  match &image
    .boxes
    .color_specification
    .as_ref()
    .map(|i| &i.color_space)
  {
    Some(jp2::colr::ColorSpace::Enumerated(e)) => match e {
      EnumeratedColorspace::Sycc => {
        sycc_to_rgb(image.decoded_components, bit_depth)?;
      }
      EnumeratedColorspace::CieLab(cielab) => {
        cielab_to_rgb(image.decoded_components, bit_depth, cielab)?;
      }
      EnumeratedColorspace::BiLevel1
      | EnumeratedColorspace::YCbCr1
      | EnumeratedColorspace::YCbCr2
      | EnumeratedColorspace::YCbCr3
      | EnumeratedColorspace::PhotoYcc
      | EnumeratedColorspace::Cmy
      | EnumeratedColorspace::Cmyk
      | EnumeratedColorspace::Ycck
      | EnumeratedColorspace::BiLevel2
      | EnumeratedColorspace::Srgb
      | EnumeratedColorspace::Greyscale
      | EnumeratedColorspace::CieJab
      | EnumeratedColorspace::EsRgb
      | EnumeratedColorspace::RommRgb
      | EnumeratedColorspace::YPbPr112560
      | EnumeratedColorspace::YPbPr125050
      | EnumeratedColorspace::EsYcc
      | EnumeratedColorspace::ScRgb
      | EnumeratedColorspace::ScRgbGray => {}
    },
    None | Some(jp2::colr::ColorSpace::Icc(_) | jp2::colr::ColorSpace::Unknown) => {}
  }

  Ok(())
}

fn get_color_space(boxes: &ImageBoxes, num_components: u64) -> Result<ColorSpace> {
  let color = boxes
    .color_specification
    .as_ref()
    .map(|entry| &entry.color_space);
  let cs = match color {
    Some(jp2::colr::ColorSpace::Enumerated(e)) => match e {
      EnumeratedColorspace::Cmyk => ColorSpace::CMYK,
      EnumeratedColorspace::Srgb => ColorSpace::RGB,
      EnumeratedColorspace::RommRgb => ColorSpace::Icc {
        profile: include_bytes!("../assets/ProPhoto-v2-micro.icc").to_vec(),
        num_channels: 3,
      },
      EnumeratedColorspace::EsRgb => ColorSpace::RGB,
      EnumeratedColorspace::Greyscale => ColorSpace::Gray,
      EnumeratedColorspace::Sycc => ColorSpace::RGB,
      EnumeratedColorspace::CieLab(_) => ColorSpace::Icc {
        profile: include_bytes!("../assets/LAB.icc").to_vec(),
        num_channels: 3,
      },
      EnumeratedColorspace::BiLevel1
      | EnumeratedColorspace::YCbCr1
      | EnumeratedColorspace::YCbCr2
      | EnumeratedColorspace::YCbCr3
      | EnumeratedColorspace::PhotoYcc
      | EnumeratedColorspace::Cmy
      | EnumeratedColorspace::Ycck
      | EnumeratedColorspace::BiLevel2
      | EnumeratedColorspace::CieJab
      | EnumeratedColorspace::YPbPr112560
      | EnumeratedColorspace::YPbPr125050
      | EnumeratedColorspace::EsYcc
      | EnumeratedColorspace::ScRgb
      | EnumeratedColorspace::ScRgbGray => {
        return Err(<crate::error::DecodeError as ::core::convert::From<
          crate::error::FormatError,
        >>::from(FormatError::Unsupported));
      }
    },
    Some(jp2::colr::ColorSpace::Icc(icc)) => {
      let Ok(icc_metadata) = ICCMetadata::from_data(icc);

      match icc_metadata {
        Some(metadata) => ColorSpace::Icc {
          profile: icc.clone(),
          num_channels: (match metadata.color_space.num_components() {
            Ok(value) => value,
          }),
        },
        None => ColorSpace::RGB,
      }
    }
    Some(jp2::colr::ColorSpace::Unknown) | None => match num_components {
      1 => ColorSpace::Gray,
      3 => ColorSpace::RGB,
      4 => ColorSpace::CMYK,
      _ => {
        let Ok(num_channels) = console_core_number_conversion::wrapped::<_, u8>(num_components);

        ColorSpace::Unknown { num_channels }
      }
    },
  };

  Ok(cs)
}

fn resolve_palette_indices(
  components: Vec<ComponentData>,
  boxes: &ImageBoxes,
) -> Result<Vec<ComponentData>> {
  let palette = match boxes.palette.as_ref() {
    Some(palette) => palette,
    None => {
      return Ok(components);
    }
  };

  let mapping = boxes
    .component_mapping
    .as_ref()
    .ok_or(ColorError::PaletteResolutionFailed)?;
  let mut resolved = Vec::with_capacity(mapping.entries.len());

  for entry in &mapping.entries {
    let Ok(component_idx) = console_core_number_conversion::wrapped_index(entry.component_index);
    let component = components
      .get(component_idx)
      .ok_or(ColorError::PaletteResolutionFailed)?;

    match entry.mapping_type {
      ComponentMappingType::Direct => resolved.push(component.clone()),
      ComponentMappingType::Palette { column } => {
        let Ok(column_idx) = console_core_number_conversion::wrapped_index(column);
        let column_info = palette
          .columns
          .get(column_idx)
          .ok_or(ColorError::PaletteResolutionFailed)?;

        let component_truncated = component.container.truncated()?;
        let mut mapped = Vec::with_capacity(component_truncated.len());

        for &sample in component_truncated {
          let Ok(round_f32) = math::round_f32(math::Sample(sample));
          let Ok(index) = console_core_number_conversion::toward_zero_i64(f64::from(round_f32));
          let Ok(palette_value) = palette.map(index, column);
          let value = palette_value.ok_or(ColorError::PaletteResolutionFailed)?;
          mapped.push(
            match console_core_number_conversion::nearest_f32_from_u64(value) {
              Ok(converted) => converted,
            },
          );
        }

        let container = math::SimdBuffer::new(mapped)?;
        resolved.push(ComponentData {
          container,
          bit_depth: column_info.bit_depth,
        });
      }
    }
  }

  Ok(resolved)
}

#[inline(always)]
fn cielab_to_rgb(components: &mut [ComponentData], bit_depth: u8, lab: &CieLab) -> Result<()> {
  let (l, a, b) = match components {
    [l, a, b, ..] => (l, a, b),
    _ => return Err(DecodeError::Color(ColorError::LabConversionFailed)),
  };

  match (l.bit_depth, a.bit_depth, b.bit_depth) {
    (4.., 4.., 4..) => {}
    _ => return Err(DecodeError::Color(ColorError::LabConversionFailed)),
  }

  let exponent = u32::from(bit_depth);
  let reference_a = 1u32.wrapping_shl(exponent.wrapping_sub(1));
  let reference_b = 1u32
    .wrapping_shl(exponent.wrapping_sub(2))
    .wrapping_add(1u32.wrapping_shl(exponent.wrapping_sub(3)));
  let Ok(bit_max) = narrow_f32(f64::from(1u32.wrapping_shl(exponent).wrapping_sub(1)));

  let specs = [
    LabChannel {
      range: lab.rl.map_or(100, core::convert::identity),
      origin: lab.ol.map_or(0, core::convert::identity),
      bits: l.bit_depth,
    },
    LabChannel {
      range: lab.ra.map_or(170, core::convert::identity),
      origin: lab.oa.map_or(reference_a, core::convert::identity),
      bits: a.bit_depth,
    },
    LabChannel {
      range: lab.ra.map_or(200, core::convert::identity),
      origin: lab.ob.map_or(reference_b, core::convert::identity),
      bits: b.bit_depth,
    },
  ];
  let [l_params, a_params, b_params] = specs;
  let Ok((l_scale, l_offset)) = lab_normalization(
    l_params,
    LabOutput::Luminance(math::Sample(bit_max / 100.0)),
  );
  let Ok((a_scale, a_offset)) =
    lab_normalization(a_params, LabOutput::Chroma(math::Sample(bit_max / 255.0)));
  let Ok((b_scale, b_offset)) =
    lab_normalization(b_params, LabOutput::Chroma(math::Sample(bit_max / 255.0)));

  let Ok(l_samples) = l.container.samples_mut();
  let Ok(a_samples) = a.container.samples_mut();
  let Ok(b_samples) = b.container.samples_mut();
  let Ok(measured) = fixed_index(l_samples.len().min(a_samples.len()).min(b_samples.len()));
  let Ok(lanes) = fixed_index(SIMD_WIDTH);
  let complete = measured.wrapping_sub(measured.wrapping_rem(lanes.max(1)));
  let Ok(count) = index(complete);
  let Ok(backend) = math::simd();

  for (samples, gain, offset) in [
    (l_samples, l_scale, l_offset),
    (a_samples, a_scale, a_offset),
    (b_samples, b_scale, b_offset),
  ] {
    let prefix = samples
      .get_mut(..count)
      .ok_or(ValidationError::InvalidDimensions)?;
    let Ok(()) = math::affine(backend, prefix, math::Gain(gain), math::Sample(offset));
  }

  Ok(())
}

struct LabChannel {
  range: u32,
  origin: u32,
  bits: u8,
}

enum LabOutput {
  Luminance(math::Sample),
  Chroma(math::Sample),
}

fn lab_normalization(
  channel: LabChannel,
  output: LabOutput,
) -> core::result::Result<(f32, f32), Never> {
  let Ok(range) = narrow_f32(f64::from(channel.range));
  let Ok(origin) = narrow_f32(f64::from(channel.origin));
  let sample_max = 1i32.wrapping_shl(u32::from(channel.bits)).wrapping_sub(1);
  let Ok(divisor) = narrow_f32(f64::from(sample_max));
  let min = -(range * origin) / divisor;
  let max = min + range;
  let scale = (max - min) / divisor;

  Ok(match output {
    LabOutput::Luminance(math::Sample(factor)) => (scale * factor, min * factor),
    LabOutput::Chroma(math::Sample(factor)) => (scale * factor, (min + 128.0) * factor),
  })
}

#[inline(always)]
fn sycc_to_rgb(components: &mut [ComponentData], bit_depth: u8) -> Result<()> {
  let bits = u32::from(bit_depth);
  let Ok(offset) = narrow_f32(f64::from(1u32.wrapping_shl(bits.wrapping_sub(1))));
  let Ok(max_value) = narrow_f32(f64::from(1u32.wrapping_shl(bits).wrapping_sub(1)));

  let (y, cb, cr) = match components {
    [y, cb, cr, ..] => (y, cb, cr),
    _ => return Err(DecodeError::Color(ColorError::SyccConversionFailed)),
  };

  let Ok(y_samples) = y.container.samples_mut();
  let Ok(cb_samples) = cb.container.samples_mut();
  let Ok(cr_samples) = cr.container.samples_mut();
  let Ok(()) = math::color(
    console_core_simd::ComponentTransform::Ycbcr {
      offset,
      gains: [1.402, -0.344136, -0.714136, 1.772],
      maximum: Some(max_value),
    },
    [y_samples, cb_samples, cr_samples],
  );

  Ok(())
}

use crate::bit_reader::BitWriter;
use crate::filter::FilterResult;
use crate::math::round_f32;
use crate::object::stream::{ImageColorSpace, ImageData, ImageDecodeParams};
use alloc::borrow::Cow;
use alloc::vec::Vec;
use console_core_jpeg_2000_files::{
  AlphaChannel, ColorSpace, DecodeSettings, PaletteMode, ValidationMode,
};
use console_core_never::Never;
use console_core_number_conversion::{checked_index, fixed_index, index};

impl ImageColorSpace {
  fn num_components(&self) -> core::result::Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::Gray => 1,
      Self::Rgb => 3,
      Self::Cmyk => 4,
      Self::Unknown(num) => *num,
    })
  }
}

pub(crate) fn decode(
  data: &[u8],
  params: &ImageDecodeParams,
) -> core::result::Result<Option<FilterResult<'static>>, console_core_never::Never> {
  use crate::object::stream::ImageColorSpace;

  let settings = DecodeSettings {
    palette: PaletteMode::KeepIndices,
    validation: ValidationMode::Lenient,
    target_resolution: params.target_dimension,
  };

  let image = match console_core_jpeg_2000_files::Image::new(data, &settings) {
    Ok(image) => image,
    Err(error) => {
      warn!("invalid JPEG 2000 stream {:?}", error);
      return Ok(None);
    }
  };

  let Ok(width) = image.width();
  let Ok(height) = image.height();
  let Ok(original_bit_depth) = image.original_bit_depth();
  let bpc = match params.bpc {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => original_bit_depth,
  };
  let Ok(color_space) = image.color_space();
  let cs = match color_space {
    ColorSpace::Gray => ImageColorSpace::Gray,
    ColorSpace::RGB => ImageColorSpace::Rgb,
    ColorSpace::CMYK => ImageColorSpace::Cmyk,
    ColorSpace::Unknown { num_channels } => ImageColorSpace::Unknown(*num_channels),
    ColorSpace::Icc {
      num_channels: num_components,
      ..
    } => match num_components {
      1 => ImageColorSpace::Gray,
      3 => ImageColorSpace::Rgb,
      4 => ImageColorSpace::Cmyk,
      _ => return Ok(None),
    },
  };
  let Ok(has_alpha) = image.has_alpha();
  let bitmap = match image.decode() {
    Ok(bitmap) => bitmap,
    Err(error) => {
      warn!("couldn't decode JPEG 2000 image {:?}", error);
      return Ok(None);
    }
  };

  let Ok(components) = cs.num_components();
  let (mut data_2, mut alpha) = match has_alpha {
    AlphaChannel::Absent => (bitmap, None),
    AlphaChannel::Present => {
      let mut color_samples = Vec::new();
      let mut alpha_samples = Vec::new();
      let Ok(sample_width) = index(u16::from(components).wrapping_add(1));

      for sample in bitmap.chunks_exact(sample_width.max(1)) {
        let (alpha_sample, color) = match sample.split_last() {
          Some(channels) => channels,
          None => return Ok(None),
        };
        alpha_samples.push(*alpha_sample);
        color_samples.extend_from_slice(color);
      }

      (color_samples, Some(alpha_samples))
    }
  };

  match ::core::primitive::bool::then_some(bpc != 8, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(cs_num_components) = cs.num_components();
      data_2 = match match scale(
        &data_2,
        self::ScaleArguments {
          bit_per_component: bpc,
          num_components: cs_num_components,
          width,
          height,
        },
      ) {
        Ok(explicit_value) => explicit_value,
      } {
        Some(value) => value,
        None => return Ok(None),
      };
      alpha = alpha.and_then(|alpha_2| {
        let Ok(explicit_value) = scale(
          &alpha_2,
          self::ScaleArguments {
            bit_per_component: bpc,
            num_components: {
              let Ok(explicit_value) = cs.num_components();
              explicit_value
            },
            width,
            height,
          },
        );
        explicit_value
      });
    }
    ::core::option::Option::None => {}
  }

  Ok(Some(FilterResult {
    data: Cow::Owned(data_2),
    image_data: Some(ImageData {
      alpha,
      color_space: Some(cs),
      bits_per_component: bpc,
      width: (width),
      height: (height),
    }),
  }))
}

fn scale(
  data: &[u8],
  ScaleArguments {
    bit_per_component,
    num_components,
    width,
    height,
  }: ScaleArguments,
) -> Result<Option<Vec<u8>>, Never> {
  let components_per_row = u64::from(num_components).wrapping_mul(u64::from(width));

  match ::core::primitive::bool::then_some(
    bit_per_component == 0 || bit_per_component > 32 || components_per_row == 0,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let rows = match checked_index(components_per_row) {
    Ok(stride) => data.chunks_exact(stride.max(1)),
    Err(_row_too_wide) => return Ok(None),
  };
  let Ok(row_count) = fixed_index(rows.len());

  match ::core::primitive::bool::then_some(row_count > u64::from(height), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let row_bytes = components_per_row
    .wrapping_mul(u64::from(bit_per_component))
    .div_ceil(8);
  let maximum_output = row_bytes.saturating_mul(u64::from(height));

  let cursor_bits = row_bytes.saturating_mul(row_count).saturating_mul(8);
  let buffer_length = maximum_output.min(cursor_bits);
  let Ok(reserve) = allocate_scaled(data, buffer_length);
  let mut output = match reserve {
    Some(buffer) => buffer,
    None => return Ok(None),
  };

  let Ok(bit_writer) = BitWriter::new(&mut output, bit_per_component);
  let mut writer = match bit_writer {
    Some(writer) => writer,
    None => return Ok(None),
  };
  let Ok(maximum_sample) = console_core_number_conversion::nearest_f32_from_u64(u64::from(
    1_u32
      .wrapping_shl(u32::from(bit_per_component))
      .wrapping_sub(1),
  ));

  for row in rows {
    for byte in row {
      let Ok(rounded) = round_f32((f32::from(*byte) / 255.0) * maximum_sample);
      let Ok(scaled) = console_core_number_conversion::toward_zero_u32(f64::from(rounded));
      let Ok(written) = writer.write(scaled);

      match written {
        Some(_) => {}
        None => return Ok(None),
      }
    }

    let Ok(()) = writer.align();
  }

  Ok(Some(output))
}

#[expect(
  clippy::slow_vector_initialization,
  reason = "The fallible reservation reports allocation failure before initializing the bounded output"
)]
fn allocate_scaled(data: &[u8], byte_count: u64) -> Result<Option<Vec<u8>>, Never> {
  let mut output = Vec::new();

  match checked_index(byte_count) {
    Ok(length) => match output.try_reserve_exact(length.min(data.len().saturating_mul(32))) {
      Ok(()) => {
        output.resize(length.min(data.len().saturating_mul(32)), 0);
        Ok(Some(output))
      }
      Err(error) => {
        warn!("couldn't allocate scaled JPEG 2000 samples {:?}", error);
        Ok(None)
      }
    },
    Err(_output_too_large) => Ok(None),
  }
}

struct ScaleArguments {
  bit_per_component: u8,
  num_components: u8,
  width: u32,
  height: u32,
}

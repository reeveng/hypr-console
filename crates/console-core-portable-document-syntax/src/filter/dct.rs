use crate::object::Dict;
use crate::object::dict::keys::COLOR_TRANSFORM;
use crate::object::stream::{FilterResult, ImageColorSpace, ImageData, ImageDecodeParams};
use alloc::borrow::Cow;
use console_core_jpeg_files::{ColorSpace, decoded_components};
use console_core_never::Never;
use console_core_number_conversion::index;

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Only a PDF decode-parameter reference can reenter the stream reader, which enforces an indirect-object depth limit"
  )
)]
pub(crate) fn decode(
  data: &[u8],
  params: &Dict<'_>,
  image_params: &ImageDecodeParams,
) -> core::result::Result<Option<FilterResult<'static>>, console_core_never::Never> {
  let Ok(patched_data) = maybe_patch_jpeg_dimensions(data, image_params);
  let data_2 = match patched_data {
    Some(jpeg) => jpeg,
    None => return Ok(None),
  };

  let mut image = match decoded_components(&data_2) {
    Ok(image) => image,
    Err(_invalid_jpeg) => return Ok(None),
  };

  match (image_params.num_components, image.color) {
    (Some(2) | None, ColorSpace::TwoComponent) => {}
    (Some(0 | 2 | 5..), _) | (Some(_), ColorSpace::TwoComponent) => return Ok(None),
    _ => {}
  }

  let Ok(transform) = params.get::<u8>(COLOR_TRANSFORM);

  match transform {
    None | Some(1) => {
      let Ok(()) = image.convert_ycbcr_to_rgb();
    }
    Some(_) => {}
  }

  let mut decoded = image.bytes;

  match image.color {
    ColorSpace::Ycck => {
      for [first, second, third, _black] in decoded.as_chunks_mut::<4>().0 {
        let y = f32::from(*first);
        let cb = f32::from(*second);
        let cr = f32::from(*third);
        *first = {
          let Ok(numeric_value) =
            console_core_number_conversion::toward_zero_u8(f64::from(434.456 - y - 1.402 * cr));
          numeric_value
        };
        *second = {
          let Ok(numeric_value) = console_core_number_conversion::toward_zero_u8(f64::from(
            119.541 - y + 0.344 * cb + 0.714 * cr,
          ));
          numeric_value
        };
        *third = {
          let Ok(numeric_value) =
            console_core_number_conversion::toward_zero_u8(f64::from(481.816 - y - 1.772 * cb));
          numeric_value
        };
      }
    }
    ColorSpace::Rgb
    | ColorSpace::YCbCr
    | ColorSpace::Grey
    | ColorSpace::Cmyk
    | ColorSpace::TwoComponent => {}
  }

  let image_data = ImageData {
    alpha: None,
    color_space: match image.color {
      ColorSpace::Rgb | ColorSpace::YCbCr => Some(ImageColorSpace::Rgb),
      ColorSpace::Grey => Some(ImageColorSpace::Gray),
      ColorSpace::Cmyk | ColorSpace::Ycck => Some(ImageColorSpace::Cmyk),
      ColorSpace::TwoComponent => None,
    },
    bits_per_component: 8,
    width: image.size.width,
    height: image.size.height,
  };

  Ok(Some(FilterResult {
    data: Cow::Owned(decoded),
    image_data: Some(image_data),
  }))
}

#[must_use]
fn maybe_patch_jpeg_dimensions<'a>(
  data: &'a [u8],
  image: &ImageDecodeParams,
) -> Result<Option<Cow<'a, [u8]>>, Never> {
  let width = match u16::try_from(image.width) {
    Ok(value) => value,
    Err(_width_exceeds_jpeg_limit) => return Ok(None),
  };
  let height = match u16::try_from(image.height) {
    Ok(value) => value,
    Err(_height_exceeds_jpeg_limit) => return Ok(None),
  };
  let Ok(marker) = find_sof_marker(data);
  let sof = match marker {
    Some(offset) => offset,
    None => return Ok(None),
  };
  let Ok(dimensions_offset) = index(sof.wrapping_add(5));
  let (jpeg_height, jpeg_width) = match data.get(dimensions_offset..) {
    Some([height_hi, height_lo, width_hi, width_lo, ..]) => (
      u16::from_be_bytes([*height_hi, *height_lo]),
      u16::from_be_bytes([*width_hi, *width_lo]),
    ),
    Some(_) | None => return Ok(None),
  };
  let jpeg_area = u32::from(jpeg_width).wrapping_mul(u32::from(jpeg_height));
  let declared_area = image.width.wrapping_mul(image.height);

  match ::core::primitive::bool::then_some(jpeg_area <= declared_area, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(Some(Cow::Borrowed(data)));
    }
    ::core::option::Option::None => {}
  }

  let mut patched = data.to_vec();

  match patched.get_mut(dimensions_offset..) {
    Some([height_hi, height_lo, width_hi, width_lo, ..]) => {
      [*height_hi, *height_lo] = height.to_be_bytes();
      [*width_hi, *width_lo] = width.to_be_bytes();
      Ok(Some(Cow::Owned(patched)))
    }
    Some(_) | None => Ok(None),
  }
}

fn find_sof_marker(data: &[u8]) -> Result<Option<u64>, Never> {
  let mut offset = 0_u64;

  for _ in data {
    let Ok(position) = index(offset);
    let (prefix, marker, length) = match data.get(position..) {
      Some([prefix, marker, body @ ..]) => (*prefix, *marker, body),
      Some(_) | None => return Ok(None),
    };
    let width = match (prefix, marker) {
      (0xff, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) => {
        return Ok(Some(offset));
      }
      (0xff, 0xff) => 1,
      (0xff, 0xd8 | 0xd9 | 0x01 | 0x00) => 2,
      (0xff, _) => {
        let size = match length.first_chunk::<2>() {
          Some(size) => *size,
          None => return Ok(None),
        };
        u64::from(u16::from_be_bytes(size)).wrapping_add(2)
      }
      _ => 1,
    };
    offset = offset.wrapping_add(width);
  }

  Ok(None)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::reader::{Readable, Reader, ReaderContext};

  #[derive(Debug)]
  enum TestFailure {
    InvalidDictionary,
    RefusedFixture,
    MissingDimensions,
  }

  fn samples(
    jpeg: &[u8],
    settings: &[u8],
    components: u8,
    size: (u32, u32),
  ) -> Result<FilterResult<'static>, TestFailure> {
    let Ok(mut reader) = Reader::new(settings);
    let Ok(context) = ReaderContext::dummy();
    let dict = Dict::read(&mut reader, &context).ok_or(TestFailure::InvalidDictionary)?;
    let params = ImageDecodeParams {
      width: size.0,
      height: size.1,
      num_components: Some(components),
      ..Default::default()
    };
    let Ok(result) = decode(jpeg, &dict, &params);
    result.ok_or(TestFailure::RefusedFixture)
  }

  #[test]
  fn color_transform_zero_leaves_the_jpeg_triples_in_scan_order() -> Result<(), TestFailure> {
    let jpeg = include_bytes!("../../../console-core-jpeg-files/tests/pictures/full.jpg");
    let raw = samples(jpeg, b"<< /ColorTransform 0 >>", 3, (75, 53))?;
    let rgb = samples(jpeg, b"<< >>", 3, (75, 53))?;
    assert_eq!(raw.data.get(..3), Some([90, 77, 236].as_slice()));
    assert_eq!(rgb.data.get(..3), Some([241, 30, 0].as_slice()));
    assert_eq!(raw.data.len(), 11_925);
    Ok(())
  }

  #[test]
  fn print_inks_and_unnamed_components_survive_dct_decoding() -> Result<(), TestFailure> {
    let cmyk = samples(
      include_bytes!("../../../console-core-jpeg-files/tests/pictures/cmyk.jpg"),
      b"<< >>",
      4,
      (75, 53),
    )?;
    assert!(matches!(
      cmyk.image_data.and_then(|d| d.color_space),
      Some(ImageColorSpace::Cmyk)
    ));
    assert_eq!(cmyk.data.get(..4), Some([255, 29, 0, 242].as_slice()));
    let ycck = samples(
      include_bytes!("../../../console-core-jpeg-files/tests/pictures/ycck.jpg"),
      b"<< >>",
      4,
      (75, 53),
    )?;
    assert!(matches!(
      ycck.image_data.and_then(|d| d.color_space),
      Some(ImageColorSpace::Cmyk)
    ));
    assert_eq!(ycck.data.get(..4), Some([255, 30, 0, 241].as_slice()));
    let two = samples(
      include_bytes!("../../../console-core-jpeg-files/tests/pictures/two-component.jpg"),
      b"<< >>",
      2,
      (1, 1),
    )?;
    let dimensions = two.image_data.ok_or(TestFailure::MissingDimensions)?;
    assert!(dimensions.color_space.is_none());
    assert_eq!(two.data.as_ref(), [136, 120]);
    Ok(())
  }
}

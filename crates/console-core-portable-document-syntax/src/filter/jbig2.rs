use crate::object::Dict;
use crate::object::Stream;
use crate::object::dict::keys::JBIG2_GLOBALS;
use crate::object::stream::{
  FilterResult, ImageColorSpace, ImageData, ImageDecodeParams, ImageSamples,
};
use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::iter;

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "An indirect JBIG2Globals reference resolves with the existing object ancestry and depth limit"
  )
)]
pub(crate) fn decode(
  data: &[u8],
  params: &Dict<'_>,
  image_params: &ImageDecodeParams,
) -> core::result::Result<Option<FilterResult<'static>>, console_core_never::Never> {
  let Ok(param) = params.get::<Stream<'_>>(JBIG2_GLOBALS);
  let globals = match param {
    Some(stream) => match stream.decoded() {
      Ok(bytes) => Some(bytes),
      Err(error) => {
        warn!("couldn't decode JBIG2 globals {:?}", error);
        None
      }
    },
    None => None,
  };
  let image =
    match console_core_black_and_white_images::Image::new_embedded(data, globals.as_deref()) {
      Ok(image) => image,
      Err(error) => {
        warn!("invalid embedded JBIG2 image {:?}", error);
        return Ok(None);
      }
    };

  let Ok(size) = image.size();

  let (decoded, bpc) = match image_params.samples {
    ImageSamples::PaletteIndices => {
      struct BitPackDecoder {
        output: Vec<u8>,
        pending: u8,
        bit_count: u8,
      }

      impl console_core_black_and_white_images::Decoder for BitPackDecoder {
        fn push_pixel(
          &mut self,
          color: console_core_black_and_white_images::Color,
        ) -> Result<(), console_core_never::Never> {
          let white = color == console_core_black_and_white_images::Color::White;
          let shift = 7_u32.wrapping_sub(u32::from(self.bit_count));
          self.pending |= u8::from(white).wrapping_shl(shift);
          self.bit_count = self.bit_count.wrapping_add(1);

          match self.bit_count {
            8 => {
              let Ok(()) = self.next_line();
            }
            _ => {}
          }

          Ok(())
        }

        fn push_pixel_chunk(
          &mut self,
          color: console_core_black_and_white_images::Color,
          chunk_count: u32,
        ) -> Result<(), console_core_never::Never> {
          let byte = match color {
            console_core_black_and_white_images::Color::Black => 0,
            console_core_black_and_white_images::Color::White => 0xff,
          };
          let Ok(count) = console_core_number_conversion::index(chunk_count);
          self.output.extend(iter::repeat_n(byte, count));
          Ok(())
        }

        fn next_line(&mut self) -> Result<(), console_core_never::Never> {
          match self.bit_count {
            0 => {}
            _ => {
              self.output.push(self.pending);
              self.pending = 0;
              self.bit_count = 0;
            }
          }

          Ok(())
        }
      }

      let mut decoder = BitPackDecoder {
        output: Vec::new(),
        pending: 0,
        bit_count: 0,
      };

      match image.decode(&mut decoder) {
        Ok(()) => {}
        Err(error) => {
          warn!("couldn't decode packed JBIG2 image {:?}", error);
          return Ok(None);
        }
      }

      (decoder.output, 1)
    }
    ImageSamples::Direct => {
      struct Luma8Decoder {
        output: Vec<u8>,
      }

      impl console_core_black_and_white_images::Decoder for Luma8Decoder {
        fn push_pixel(
          &mut self,
          color: console_core_black_and_white_images::Color,
        ) -> Result<(), console_core_never::Never> {
          let black = color == console_core_black_and_white_images::Color::Black;
          self
            .output
            .push(match ::core::primitive::bool::then_some(black, ()) {
              ::core::option::Option::Some(()) => 0x00,
              ::core::option::Option::None => 0xFF,
            });
          Ok(())
        }

        fn push_pixel_chunk(
          &mut self,
          color: console_core_black_and_white_images::Color,
          chunk_count: u32,
        ) -> Result<(), console_core_never::Never> {
          let black = color == console_core_black_and_white_images::Color::Black;
          let byte = match ::core::primitive::bool::then_some(black, ()) {
            ::core::option::Option::Some(()) => 0x00,
            ::core::option::Option::None => 0xFF,
          };
          self.output.extend(iter::repeat_n(
            byte,
            ({
              let Ok(numeric_value) = console_core_number_conversion::wrapped_index(chunk_count);
              numeric_value
            })
            .wrapping_mul(8),
          ));
          Ok(())
        }

        fn next_line(&mut self) -> Result<(), console_core_never::Never> {
          Ok(())
        }
      }

      let mut decoder = Luma8Decoder { output: Vec::new() };

      match image.decode(&mut decoder) {
        Ok(()) => {}
        Err(error) => {
          warn!("couldn't decode JBIG2 samples {:?}", error);
          return Ok(None);
        }
      }

      (decoder.output, 8)
    }
  };

  Ok(Some(FilterResult {
    data: Cow::Owned(decoded),
    image_data: Some(ImageData {
      alpha: None,
      color_space: Some(ImageColorSpace::Gray),
      bits_per_component: bpc,
      width: size.width,
      height: size.height,
    }),
  }))
}

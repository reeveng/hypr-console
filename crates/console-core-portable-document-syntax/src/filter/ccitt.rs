use crate::object::dict::keys::{BLACK_IS_1, COLUMNS, ENCODED_BYTE_ALIGN, END_OF_BLOCK, K, ROWS};
use crate::object::stream::{
  FilterResult, ImageColorSpace, ImageData, ImageDecodeParams, ImageSamples,
};
use crate::object::{Dict, PdfBoolean};
use alloc::borrow::Cow;
use alloc::vec::Vec;
use console_core_fax_images::{
  Color, DecodeSettings, Decoder, DecoderContext, EncodingMode, EndOfBlock, RowAlignment,
};
use console_core_never::Never;
use core::iter;

enum FaxPolarity {
  BlackIsOne,
  WhiteIsOne,
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The fax dictionary's indirect values resolve with the existing object ancestry and depth limit"
  )
)]
pub(crate) fn decode(
  data: &[u8],
  params: &Dict<'_>,
  image_params: &ImageDecodeParams,
) -> core::result::Result<Option<FilterResult<'static>>, console_core_never::Never> {
  let Ok(param) = params.get::<i32>(K);
  let k = match param {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => 0,
  };

  let Ok(dictionary_rows) = params.get::<u32>(ROWS);
  let rows = match dictionary_rows {
    Some(rows) => rows,
    None => image_params.height,
  };
  let Ok(end_of_block) = params.get::<PdfBoolean>(END_OF_BLOCK);
  let Ok(black_is_one) = params.get::<PdfBoolean>(BLACK_IS_1);
  let Ok(byte_alignment) = params.get::<PdfBoolean>(ENCODED_BYTE_ALIGN);
  let polarity = match black_is_one {
    Some(PdfBoolean::True) => FaxPolarity::BlackIsOne,
    Some(PdfBoolean::False) | None => FaxPolarity::WhiteIsOne,
  };

  let Ok(dictionary_columns) = params.get::<u64>(COLUMNS);
  let declared_columns = match dictionary_columns {
    Some(columns) => columns,
    None => 1728,
  };
  let Ok(columns) = console_core_number_conversion::wrapped::<_, u32>(declared_columns);
  let settings = DecodeSettings {
    columns,
    rows,
    end_of_block: match end_of_block {
      Some(PdfBoolean::False) => EndOfBlock::NotExpected,
      Some(PdfBoolean::True) | None => EndOfBlock::Expected,
    },
    row_alignment: match byte_alignment {
      Some(PdfBoolean::True) => RowAlignment::Byte,
      Some(PdfBoolean::False) | None => RowAlignment::Bit,
    },
    encoding: match k.cmp(&0) {
      core::cmp::Ordering::Less => EncodingMode::Group4,
      core::cmp::Ordering::Equal => EncodingMode::Group3OneDimensional,
      core::cmp::Ordering::Greater => EncodingMode::Group3TwoDimensional,
    },
  };

  let Ok(ccitt_size_ok) = super::ccitt_size_ok(settings.columns, settings.rows);

  match ::core::primitive::bool::then_some(
    !matches!(ccitt_size_ok, crate::CcittSizeOkAnswer::Yes),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let (decoded, bpc) = match image_params.samples {
    ImageSamples::PaletteIndices => {
      struct BitPackDecoder {
        polarity: FaxPolarity,
        output: Vec<u8>,
        decoded_rows: u32,
        buffer: u8,
        bit_count: u8,
      }

      impl BitPackDecoder {
        fn push_bit(
          &mut self,
          sample: crate::IsOneAnswer,
        ) -> core::result::Result<(), console_core_never::Never> {
          let bit = match sample {
            crate::IsOneAnswer::Yes => 1,
            crate::IsOneAnswer::No => 0,
          };
          self.buffer = u8::wrapping_shl(self.buffer, 1_u32) | bit;

          {
            let numeric_amount = 1;
            let numeric_target = &mut (self.bit_count);
            *numeric_target = u8::wrapping_add(*numeric_target, numeric_amount);
          };

          match ::core::primitive::bool::then_some(self.bit_count == 8, ()) {
            ::core::option::Option::Some(()) => {
              self.output.push(self.buffer);
              self.buffer = 0;
              self.bit_count = 0;
            }
            ::core::option::Option::None => {}
          }

          Ok(())
        }

        fn flush(&mut self) -> core::result::Result<(), console_core_never::Never> {
          match ::core::primitive::bool::then_some(self.bit_count > 0, ()) {
            ::core::option::Option::Some(()) => {
              let padded = u8::wrapping_shl(self.buffer, {
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(
                  u8::wrapping_sub(8, self.bit_count),
                );
                numeric_value
              });
              self.output.push(padded);
              self.buffer = 0;
              self.bit_count = 0;
            }
            ::core::option::Option::None => {}
          }

          Ok(())
        }
      }

      impl Decoder for BitPackDecoder {
        fn push_pixel(&mut self, color: Color) -> Result<(), Never> {
          let Ok(sample) = is_one(color, &self.polarity);
          let Ok(()) = self.push_bit(sample);
          Ok(())
        }

        fn push_pixel_chunk(&mut self, color: Color, chunk_count: u32) -> Result<(), Never> {
          let Ok(byte) = sample_byte(color, &self.polarity);
          self.output.extend(iter::repeat_n(byte, {
            let Ok(numeric_value) = console_core_number_conversion::wrapped_index(chunk_count);
            numeric_value
          }));
          Ok(())
        }

        fn next_line(&mut self) -> Result<(), Never> {
          {
            let numeric_amount = 1;
            let numeric_target = &mut (self.decoded_rows);
            *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount);
          };

          let Ok(()) = self.flush();
          Ok(())
        }
      }

      let mut decoder = BitPackDecoder {
        polarity,
        output: Vec::new(),
        decoded_rows: 0,
        buffer: 0,
        bit_count: 0,
      };
      let Ok(mut context) = DecoderContext::new(settings);
      let result = console_core_fax_images::decode(data, &mut decoder, &mut context);

      match (result, decoder.decoded_rows) {
        (Err(error), 0) => {
          warn!("couldn't decode the first CCITT row {:?}", error);
          return Ok(None);
        }
        (Err(error), recovered_rows) => warn!(
          "fax decoding ended after {} rows {:?}",
          recovered_rows, error
        ),
        (Ok(_decoded_bits), _) => {}
      }

      (decoder.output, 1)
    }
    ImageSamples::Direct => {
      struct Luma8Decoder {
        polarity: FaxPolarity,
        output: Vec<u8>,
        decoded_rows: u32,
      }

      impl Decoder for Luma8Decoder {
        fn push_pixel(&mut self, color: Color) -> Result<(), Never> {
          let Ok(byte) = sample_byte(color, &self.polarity);
          self.output.push(byte);
          Ok(())
        }

        fn push_pixel_chunk(&mut self, color: Color, chunk_count: u32) -> Result<(), Never> {
          let Ok(byte) = sample_byte(color, &self.polarity);
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

        fn next_line(&mut self) -> Result<(), Never> {
          {
            let numeric_amount = 1;
            let numeric_target = &mut (self.decoded_rows);
            *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount);
          };

          Ok(())
        }
      }

      let mut decoder = Luma8Decoder {
        polarity,
        output: Vec::new(),
        decoded_rows: 0,
      };
      let Ok(mut context) = DecoderContext::new(settings);
      let result = console_core_fax_images::decode(data, &mut decoder, &mut context);

      match (result, decoder.decoded_rows) {
        (Err(error), 0) => {
          warn!("couldn't decode the first CCITT row {:?}", error);
          return Ok(None);
        }
        (Err(error), recovered_rows) => warn!(
          "fax decoding ended after {} rows {:?}",
          recovered_rows, error
        ),
        (Ok(_decoded_bits), _) => {}
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
      width: settings.columns,
      height: image_params.height,
    }),
  }))
}

fn is_one(
  color: Color,
  polarity: &FaxPolarity,
) -> core::result::Result<crate::IsOneAnswer, console_core_never::Never> {
  Ok(match (color, polarity) {
    (Color::White, FaxPolarity::WhiteIsOne) | (Color::Black, FaxPolarity::BlackIsOne) => {
      crate::IsOneAnswer::Yes
    }
    (Color::White, FaxPolarity::BlackIsOne) | (Color::Black, FaxPolarity::WhiteIsOne) => {
      crate::IsOneAnswer::No
    }
  })
}

fn sample_byte(color: Color, polarity: &FaxPolarity) -> Result<u8, Never> {
  let Ok(sample) = is_one(color, polarity);

  Ok(match sample {
    crate::IsOneAnswer::Yes => 0xff,
    crate::IsOneAnswer::No => 0x00,
  })
}

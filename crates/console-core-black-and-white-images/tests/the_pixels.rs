//! Embedded JBIG2 streams decoded to pixels: the four generic-region templates against recorded rows, and the streams that once went wrong.

use console_core_black_and_white_images::{Color, DecodeError, Decoder, Image};
use console_core_never::Never;

#[derive(Default)]
struct Pixels {
  pixels: Vec<u8>,
  rows: u32,
}

impl Decoder for Pixels {
  fn push_pixel(&mut self, color: Color) -> Result<(), Never> {
    self.pixels.push(u8::from(color == Color::Black));

    Ok(())
  }

  fn push_pixel_chunk(&mut self, color: Color, chunk_count: u32) -> Result<(), Never> {
    for _pixel in 0..chunk_count.saturating_mul(8) {
      let Ok(()) = self.push_pixel(color);
    }

    Ok(())
  }

  fn next_line(&mut self) -> Result<(), Never> {
    self.rows = self.rows.saturating_add(1);

    Ok(())
  }
}

fn decoded(bytes: &[u8]) -> Result<Pixels, DecodeError> {
  let image = Image::new_embedded(bytes, None)?;
  let mut output = Pixels::default();

  image.decode(&mut output)?;

  Ok(output)
}

fn from_file(bytes: &[u8]) -> Result<Pixels, DecodeError> {
  let embedded = bytes.get(13..).ok_or(DecodeError::EndOfData)?;

  decoded(embedded)
}

#[test]
fn all_arithmetic_templates_preserve_the_document_bitmap()
-> Result<(), Box<dyn std::error::Error>> {
  for (bytes, expected) in [
    (
      include_bytes!("fixtures/template-0.jb2").as_slice(),
      include_bytes!("fixtures/template-0.pixels").as_slice(),
    ),
    (
      include_bytes!("fixtures/template-1.jb2").as_slice(),
      include_bytes!("fixtures/template-1.pixels").as_slice(),
    ),
    (
      include_bytes!("fixtures/template-2.jb2").as_slice(),
      include_bytes!("fixtures/template-2.pixels").as_slice(),
    ),
    (
      include_bytes!("fixtures/template-3.jb2").as_slice(),
      include_bytes!("fixtures/template-3.pixels").as_slice(),
    ),
  ] {
    let image = Image::new_embedded(bytes, None)?;
    let Ok(size) = image.size();

    assert_eq!((size.width, size.height), (16, 16));

    let expected_pixels: Vec<u8> = expected
      .iter()
      .filter(|byte| **byte != b'\n')
      .map(|byte| byte.saturating_sub(b'0'))
      .collect();
    let first = decoded(bytes)?;
    let again = decoded(bytes)?;

    assert_eq!(first.rows, 16);
    assert_eq!(first.pixels, expected_pixels);
    assert_eq!(again.pixels, expected_pixels);
  }

  Ok(())
}

#[test]
fn a_huffman_text_region_refines_with_statistics_it_keeps_from_one_symbol_to_the_next()
-> Result<(), Box<dyn std::error::Error>> {
  let plain = from_file(include_bytes!("fixtures/symbol.jbig2"))?;

  for refined in [
    include_bytes!("fixtures/huffman_text_refine.jbig2").as_slice(),
    include_bytes!("fixtures/huffman_text_refine_large_offsets.jbig2").as_slice(),
  ] {
    let decoded = from_file(refined)?;

    assert_eq!(decoded.pixels, plain.pixels);
  }

  Ok(())
}

#[test]
fn a_symbol_dictionary_that_never_advances_is_refused_rather_than_waited_on() {
  for stream in [
    include_bytes!("fixtures/malformed_symbol_dictionary_empty_export_runs.jbig2").as_slice(),
    include_bytes!("fixtures/malformed_symbol_dictionary_empty_height_classes.jbig2").as_slice(),
  ] {
    let outcome = from_file(stream);

    assert!(matches!(outcome, Err(DecodeError::NoSymbols)));
  }
}

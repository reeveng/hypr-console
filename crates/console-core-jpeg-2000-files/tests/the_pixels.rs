//! Lossless OpenJPEG fixtures cover ordinary, RESET and RESET+RESTART coding.

use console_core_jpeg_2000_files::{DecodeSettings, Image};

#[test]
fn mq_coding_and_context_resets_preserve_every_pixel() -> Result<(), Box<dyn std::error::Error>> {
  let expected: Vec<u8> = (0u8..16)
    .flat_map(|y| {
      (0u8..16).map(move |x| {
        x.wrapping_mul(17)
          .wrapping_add(y.wrapping_mul(29))
          .wrapping_add(x.wrapping_mul(y).wrapping_mul(3))
      })
    })
    .collect();

  for bytes in [
    include_bytes!("fixtures/ordinary.j2k").as_slice(),
    include_bytes!("fixtures/reset.j2k").as_slice(),
    include_bytes!("fixtures/reset-restart.j2k").as_slice(),
  ] {
    let settings = DecodeSettings {
      validation: console_core_jpeg_2000_files::ValidationMode::Strict,
      ..DecodeSettings::default()
    };
    let image = Image::new(bytes, &settings)?;
    let Ok(width) = image.width();
    let Ok(height) = image.height();
    assert_eq!((width, height), (16, 16));
    let decode = image.decode()?;
    assert_eq!(decode, expected);
  }

  Ok(())
}

#[test]
fn short_destination_returns_an_error_without_writing_partial_pixels()
-> Result<(), Box<dyn std::error::Error>> {
  let source = include_bytes!("fixtures/ordinary.j2k");
  let image = Image::new(source, &DecodeSettings::default())?;
  let mut context = console_core_jpeg_2000_files::DecoderContext::default();
  let mut output = [0x55u8; 255];

  assert_eq!(
    image.decode_into(&mut output, &mut context),
    Err(console_core_jpeg_2000_files::DecodeError::Validation(
      console_core_jpeg_2000_files::ValidationError::InvalidDimensions,
    )),
  );
  assert_eq!(output, [0x55u8; 255]);
  Ok(())
}

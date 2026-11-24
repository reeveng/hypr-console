//! Video usability information fields with the same syntax in AVC and HEVC.

use console_core_number_conversion::{fitted, index};

use crate::{BitError, BitReader, FlagBit};

const SAMPLE_ASPECT_RATIOS: [(u16, u16); 17] = [
  (0, 0),
  (1, 1),
  (12, 11),
  (10, 11),
  (16, 11),
  (40, 33),
  (24, 11),
  (20, 11),
  (32, 11),
  (80, 33),
  (18, 11),
  (15, 11),
  (64, 33),
  (160, 99),
  (4, 3),
  (3, 2),
  (2, 1),
];

pub struct AspectRatio {
  pub identifier: u8,
  pub sample_aspect_ratio: (u16, u16),
}

pub fn read_aspect_ratio(reader: &mut BitReader<'_>) -> Result<AspectRatio, BitError> {
  let present = reader.read_flag()?;
  let identifier = match present {
    FlagBit::Zero => {
      return Ok(AspectRatio {
        identifier: 0,
        sample_aspect_ratio: (0, 0),
      });
    }
    FlagBit::One => {
      let identifier = reader.read_byte()?;

      identifier
    }
  };
  let sample_aspect_ratio = match identifier {
    255 => {
      let coded_width = reader.read_bits(16)?;
      let coded_height = reader.read_bits(16)?;
      let Ok(width) = fitted::<u32, u16>(coded_width);
      let Ok(height) = fitted::<u32, u16>(coded_height);

      (width, height)
    }
    _ => {
      let Ok(at) = index(identifier);

      match SAMPLE_ASPECT_RATIOS.get(at) {
        Some(&ratio) => ratio,
        None => (0, 0),
      }
    }
  };

  Ok(AspectRatio {
    identifier,
    sample_aspect_ratio,
  })
}

pub struct VideoSignal {
  pub format: u8,
  pub range: FlagBit,
  pub colour_primaries: u8,
  pub transfer_characteristics: u8,
  pub matrix_coefficients: u8,
}

impl VideoSignal {
  pub const UNSPECIFIED: Self = Self {
    format: 5,
    range: FlagBit::Zero,
    colour_primaries: 2,
    transfer_characteristics: 2,
    matrix_coefficients: 2,
  };
}

pub fn read_video_signal(reader: &mut BitReader<'_>) -> Result<VideoSignal, BitError> {
  let mut signal = VideoSignal::UNSPECIFIED;
  let present = reader.read_flag()?;

  match present {
    FlagBit::Zero => return Ok(signal),
    FlagBit::One => {}
  }

  let coded_format = reader.read_bits(3)?;
  let Ok(format) = fitted::<u32, u8>(coded_format);
  signal.format = format;
  let range = reader.read_flag()?;
  signal.range = range;
  let colour_description = reader.read_flag()?;

  match colour_description {
    FlagBit::Zero => {}
    FlagBit::One => {
      let colour_primaries = reader.read_byte()?;
      let transfer_characteristics = reader.read_byte()?;
      let matrix_coefficients = reader.read_byte()?;
      signal.colour_primaries = colour_primaries;
      signal.transfer_characteristics = transfer_characteristics;
      signal.matrix_coefficients = matrix_coefficients;
    }
  }

  Ok(signal)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reads_extended_aspect_ratios_and_preserves_reserved_identifiers() -> Result<(), BitError> {
    for (bytes, identifier, sample_aspect_ratio, end) in [
      (b"\x00".as_slice(), 0, (0, 0), 1),
      (b"\x80\x80".as_slice(), 1, (1, 1), 9),
      (b"\x88\x80".as_slice(), 17, (0, 0), 9),
      (b"\xff\x80\x20\x00\x16\x80".as_slice(), 255, (64, 45), 41),
    ] {
      let Ok(mut reader) = BitReader::new(bytes);
      let ratio = read_aspect_ratio(&mut reader)?;

      assert_eq!(
        (ratio.identifier, ratio.sample_aspect_ratio),
        (identifier, sample_aspect_ratio)
      );
      assert_eq!(reader.position(), Ok(end));
    }

    let Ok(mut truncated) = BitReader::new(b"\xff\x80");

    assert!(matches!(
      read_aspect_ratio(&mut truncated),
      Err(BitError::PastTheEnd)
    ));

    Ok(())
  }

  #[test]
  fn video_signal_defaults_colour_only_when_the_description_is_absent() -> Result<(), BitError> {
    for (bytes, format, range, colours, end) in [
      (b"\x00".as_slice(), 5, FlagBit::Zero, [2, 2, 2], 1),
      (b"\xb8".as_slice(), 3, FlagBit::One, [2, 2, 2], 6),
      (
        b"\xbc\x24\x40\x24".as_slice(),
        3,
        FlagBit::One,
        [9, 16, 9],
        30,
      ),
    ] {
      let Ok(mut reader) = BitReader::new(bytes);
      let signal = read_video_signal(&mut reader)?;

      assert_eq!((signal.format, signal.range), (format, range));
      assert_eq!(
        [
          signal.colour_primaries,
          signal.transfer_characteristics,
          signal.matrix_coefficients
        ],
        colours
      );
      assert_eq!(reader.position(), Ok(end));
    }

    Ok(())
  }
}

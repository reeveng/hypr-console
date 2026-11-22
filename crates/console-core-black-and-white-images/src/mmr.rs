use console_core_fax_images::{
  Color, DecodeSettings, Decoder, DecoderContext, EncodingMode, EndOfBlock, RowAlignment,
};
use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::bitmap::Bitmap;
use crate::error::DecodeError;

struct Writer<'a> {
  bitmap: &'a mut Bitmap,
  at: Point<u32>,
}

impl Decoder for Writer<'_> {
  fn push_pixel(&mut self, color: Color) -> Result<(), Never> {
    let bit = match color {
      Color::Black => 1,
      Color::White => 0,
    };
    let Ok(()) = self.bitmap.put(self.at, bit);

    self.at.x = self.at.x.saturating_add(1);

    Ok(())
  }

  fn push_pixel_chunk(&mut self, color: Color, chunk_count: u32) -> Result<(), Never> {
    for _pixel in 0..chunk_count.saturating_mul(8) {
      self.push_pixel(color)?;
    }

    Ok(())
  }

  fn next_line(&mut self) -> Result<(), Never> {
    self.at = Point {
      x: 0,
      y: self.at.y.saturating_add(1),
    };

    Ok(())
  }
}

pub(crate) fn decode(bitmap: &mut Bitmap, data: &[u8]) -> Result<u32, DecodeError> {
  let settings = DecodeSettings {
    columns: bitmap.size.width,
    rows: bitmap.size.height,
    end_of_block: EndOfBlock::Expected,
    row_alignment: RowAlignment::Bit,
    encoding: EncodingMode::Group4,
  };
  let Ok(mut context) = DecoderContext::new(settings);
  let mut writer = Writer {
    bitmap,
    at: Point { x: 0, y: 0 },
  };

  match console_core_fax_images::decode(data, &mut writer, &mut context) {
    Ok(read) => {
      let Ok(read_2) = fitted::<_, u32>(read);

      Ok(read_2)
    }
    Err(_fault) => Err(DecodeError::InvalidMmrData),
  }
}

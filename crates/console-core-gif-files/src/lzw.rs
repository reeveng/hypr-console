//! Decode the palette indices required for a GIF frame.

use console_core_geometry::Size;
use console_core_lzw::{BitOrder, End, Parameters, WidthGrowth};

use crate::GifError;

pub(crate) fn unpacked(packed: &[u8], least: u32, frame: Size<u32>) -> Result<Vec<u8>, GifError> {
  let pixels = u64::from(frame.width.saturating_mul(frame.height));
  let decoded = console_core_lzw::decode(
    packed,
    Parameters {
      bit_order: BitOrder::LeastSignificantFirst,
      literal_width: least,
      width_growth: WidthGrowth::AtCapacity,
      output_limit: Some(pixels),
    },
  )
  .map_err(|_| GifError::Corrupt)?;

  match decoded.end {
    End::OutputLimit => Ok(decoded.bytes),
    End::Code | End::Input => Err(GifError::Truncated),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_code_the_table_has_not_reached_is_damage() {
    let clear_then_seven = [0b0011_1100];

    assert_eq!(
      unpacked(
        &clear_then_seven,
        2,
        Size {
          width: 4,
          height: 1
        }
      ),
      Err(GifError::Corrupt)
    );
  }

  #[test]
  fn codes_that_end_before_the_frame_does_are_the_frame_cut_short() {
    let clear_then_one_index = [0b0000_0100];

    assert_eq!(
      unpacked(
        &clear_then_one_index,
        2,
        Size {
          width: 10,
          height: 10
        }
      ),
      Err(GifError::Truncated)
    );
  }

  #[test]
  fn what_the_codes_say_past_the_frame_is_not_read() -> Result<(), GifError> {
    let clear_one_two_three = [0b1000_1100, 0b0000_0110];

    let unpacked = unpacked(
      &clear_one_two_three,
      2,
      Size {
        width: 2,
        height: 1,
      },
    )?;

    assert_eq!(unpacked, [1, 2]);

    Ok(())
  }
}

//! Reads sparse-set nodes using branch factors represented by supported types.
//!
//! A packed node is a few bits of a byte, read from the lowest bits up. Where
//! it sits in its byte is a count of bits below eight, so moving past it is a
//! sum that is taken back below eight rather than one that can grow, and the
//! byte it sits in is a position that only moves forward while there is data
//! under it.

#[cfg(test)]
use core::cmp::Ordering;
use core::num::{NonZeroU8, NonZeroU32};

use super::sparse_bit_set::{BranchFactor, DecodingError};
use console_core_never::Never;
#[cfg(test)]
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
#[cfg(test)]
use core::num::NonZeroU64;

const BYTE_BITS: NonZeroU32 = match NonZeroU32::new(8) {
  Some(bits) => bits,
  None => NonZeroU32::MIN,
};

const PAST_THE_FACTOR: NonZeroU8 = match NonZeroU8::new(4) {
  Some(scale) => scale,
  None => NonZeroU8::MIN,
};

pub(crate) struct InputBitStream<'a, const BF: u8> {
  data: &'a [u8],
  byte_index: u64,
  sub_index: u32,
}

pub(crate) struct Factor<const BF: u8>;

pub(crate) enum Layout {
  Packed { mask: u32, width: u32 },
  Byte,
  Word,
}

pub(crate) trait SupportedFactor {
  const LAYOUT: Layout;
  const BRANCHES: NonZeroU32;
}

impl SupportedFactor for Factor<2> {
  const LAYOUT: Layout = Layout::Packed { mask: 3, width: 2 };
  const BRANCHES: NonZeroU32 = NonZeroU32::MIN.saturating_add(1);
}

impl SupportedFactor for Factor<4> {
  const LAYOUT: Layout = Layout::Packed { mask: 15, width: 4 };
  const BRANCHES: NonZeroU32 = NonZeroU32::MIN.saturating_add(3);
}

impl SupportedFactor for Factor<8> {
  const LAYOUT: Layout = Layout::Byte;
  const BRANCHES: NonZeroU32 = NonZeroU32::MIN.saturating_add(7);
}

impl SupportedFactor for Factor<32> {
  const LAYOUT: Layout = Layout::Word;
  const BRANCHES: NonZeroU32 = NonZeroU32::MIN.saturating_add(31);
}

impl<const BF: u8> Iterator for InputBitStream<'_, BF>
where
  Factor<BF>: SupportedFactor,
{
  type Item = u32;

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(position) = index(self.byte_index);

    match Factor::<BF>::LAYOUT {
      Layout::Packed { mask, width } => {
        let byte = self.data.get(position)?;
        let shifted = u32::from(*byte).checked_shr(self.sub_index)?;
        let following = self.byte_index.checked_add(1)?;

        self.sub_index = self.sub_index.saturating_add(width) % BYTE_BITS;

        match self.sub_index {
          0 => self.byte_index = following,
          _within_the_byte => {}
        }

        Some(shifted & mask)
      }
      Layout::Byte => {
        let byte = self.data.get(position)?;
        let following = self.byte_index.checked_add(1)?;

        self.byte_index = following;

        Some(u32::from(*byte))
      }
      Layout::Word => {
        let tail = self.data.get(position..)?;
        let bytes = tail.first_chunk::<4>()?;
        let following = self.byte_index.checked_add(4)?;

        self.byte_index = following;

        Some(u32::from_le_bytes(*bytes))
      }
    }
  }
}

impl<'a, const BF: u8> From<&'a [u8]> for InputBitStream<'a, BF> {
  fn from(data: &'a [u8]) -> Self {
    Self {
      data,
      byte_index: 1,
      sub_index: 0,
    }
  }
}

impl<'a, const BF: u8, const N: usize> From<&'a [u8; N]> for InputBitStream<'a, BF> {
  fn from(data: &'a [u8; N]) -> Self {
    Self::from(data.as_slice())
  }
}

#[allow(
  clippy::unusual_byte_groupings,
  reason = "bit groups follow the sparse set encoding"
)]
pub(crate) fn decode_header(data: &[u8]) -> Result<(BranchFactor, u8), DecodingError> {
  let first_byte = data.first().ok_or(DecodingError)?;
  let bf_bits = 0b0_00000_11 & first_byte;
  let depth_bits = (0b0_11111_00 & first_byte) / PAST_THE_FACTOR;
  let branch_factor = match bf_bits {
    0b00 => BranchFactor::Two,
    0b01 => BranchFactor::Four,
    0b10 => BranchFactor::Eight,
    _ => BranchFactor::ThirtyTwo,
  };
  Ok((branch_factor, depth_bits))
}

impl<'a, const BF: u8> InputBitStream<'a, BF> {
  pub(crate) fn bytes_consumed(&self) -> Result<u64, Never> {
    Ok(self.byte_index.saturating_add(match self.sub_index {
      0 => 0,
      _partial_byte => 1,
    }))
  }
}

impl<const BF: u8> InputBitStream<'_, BF>
where
  Factor<BF>: SupportedFactor,
{
  #[cfg(test)]
  pub(crate) fn skip_nodes(&mut self, count: u32) -> Result<(), DecodingError> {
    let byte_bits = NonZeroU64::from(BYTE_BITS);
    let skipped_bits = u64::from(count)
      .checked_mul(u64::from(BF))
      .ok_or(DecodingError)?;
    let bit_index = u64::from(self.sub_index)
      .checked_add(skipped_bits)
      .ok_or(DecodingError)?;
    let skipped_bytes = bit_index / byte_bits;
    let next_sub_index = match u32::try_from(bit_index % byte_bits) {
      Ok(index) => index,
      Err(_too_wide) => return Err(DecodingError),
    };
    let byte_index = self
      .byte_index
      .checked_add(skipped_bytes)
      .ok_or(DecodingError)?;

    self.byte_index = byte_index;
    self.sub_index = next_sub_index;

    let Ok(consumed) = self.bytes_consumed();
    let Ok(length) = fitted::<_, u64>(self.data.len());

    match consumed.cmp(&length) {
      Ordering::Less | Ordering::Equal => Ok(()),
      Ordering::Greater => Err(DecodingError),
    }
  }
}

#[cfg(test)]
#[allow(
  clippy::unusual_byte_groupings,
  reason = "bit groups follow the sparse set encoding"
)]
mod test {
  use super::*;

  #[test]
  fn huge_skips_report_end_of_data() -> Result<(), crate::TestFailure> {
    let mut packed = InputBitStream::<2>::from(&[0]);
    assert_eq!(packed.skip_nodes(u32::MAX), Err(DecodingError));
    let mut words = InputBitStream::<32>::from(&[0]);
    assert_eq!(words.skip_nodes(u32::MAX), Err(DecodingError));

    Ok(())
  }

  #[test]
  fn read_header() -> Result<(), crate::TestFailure> {
    assert_eq!(
      Ok((BranchFactor::Two, 25u8)),
      decode_header(&[0b1_11001_00u8])
    );

    assert_eq!(
      Ok((BranchFactor::Four, 0u8)),
      decode_header(&[0b1_00000_01u8])
    );

    assert_eq!(
      Ok((BranchFactor::Eight, 31u8)),
      decode_header(&[0b1_11111_10u8])
    );

    assert_eq!(
      Ok((BranchFactor::ThirtyTwo, 9u8)),
      decode_header(&[0b1_01001_11u8])
    );

    Ok(())
  }

  #[test]
  fn read_2() -> Result<(), crate::TestFailure> {
    let mut stream = InputBitStream::<2>::from(&[0b00000000, 0b11_10_01_00, 0b00_01_10_11]);
    assert_eq!(stream.bytes_consumed(), Ok(1));

    assert_eq!(stream.next(), Some(0b00));
    assert_eq!(stream.bytes_consumed(), Ok(2));
    assert_eq!(stream.next(), Some(0b01));
    assert_eq!(stream.next(), Some(0b10));
    assert_eq!(stream.next(), Some(0b11));
    assert_eq!(stream.bytes_consumed(), Ok(2));

    assert_eq!(stream.next(), Some(0b11));
    assert_eq!(stream.bytes_consumed(), Ok(3));
    assert_eq!(stream.next(), Some(0b10));
    assert_eq!(stream.next(), Some(0b01));
    assert_eq!(stream.next(), Some(0b00));
    assert_eq!(stream.bytes_consumed(), Ok(3));

    assert_eq!(stream.next(), None);

    let mut stream_2 = InputBitStream::<2>::from(&[]);
    assert_eq!(stream_2.next(), None);

    Ok(())
  }

  #[test]
  fn skip_2() -> Result<(), crate::TestFailure> {
    let mut stream = InputBitStream::<2>::from(&[0b00000000, 0b11_10_01_00, 0b00_01_10_11]);
    assert_eq!(stream.bytes_consumed(), Ok(1));

    assert_eq!(stream.skip_nodes(1), Ok(()));
    assert_eq!(stream.bytes_consumed(), Ok(2));
    assert_eq!(stream.skip_nodes(2), Ok(()));
    assert_eq!(stream.bytes_consumed(), Ok(2));
    assert_eq!(stream.skip_nodes(1), Ok(()));
    assert_eq!(stream.bytes_consumed(), Ok(2));

    assert_eq!(stream.skip_nodes(1), Ok(()));
    assert_eq!(stream.bytes_consumed(), Ok(3));
    assert_eq!(stream.skip_nodes(3), Ok(()));
    assert_eq!(stream.bytes_consumed(), Ok(3));
    assert_eq!(stream.skip_nodes(1), Err(DecodingError));

    Ok(())
  }

  #[test]
  fn skip_2_unaligned() -> Result<(), crate::TestFailure> {
    let mut stream = InputBitStream::<2>::from(&[0b00000000, 0b11_10_01_00, 0b00_01_10_11]);
    assert_eq!(stream.bytes_consumed(), Ok(1));

    assert_eq!(stream.skip_nodes(3), Ok(()));
    assert_eq!(stream.bytes_consumed(), Ok(2));

    assert_eq!(stream.skip_nodes(3), Ok(()));
    assert_eq!(stream.bytes_consumed(), Ok(3));

    assert_eq!(stream.skip_nodes(3), Err(DecodingError));

    Ok(())
  }

  #[test]
  fn read_4() -> Result<(), crate::TestFailure> {
    let mut stream = InputBitStream::<4>::from(&[0b00000000, 0b1110_0100, 0b0001_1011]);
    assert_eq!(stream.bytes_consumed(), Ok(1));

    assert_eq!(stream.next(), Some(0b0100));
    assert_eq!(stream.bytes_consumed(), Ok(2));
    assert_eq!(stream.next(), Some(0b1110));
    assert_eq!(stream.bytes_consumed(), Ok(2));

    assert_eq!(stream.next(), Some(0b1011));
    assert_eq!(stream.bytes_consumed(), Ok(3));
    assert_eq!(stream.next(), Some(0b0001));
    assert_eq!(stream.bytes_consumed(), Ok(3));

    assert_eq!(stream.next(), None);

    let mut stream_2 = InputBitStream::<4>::from(&[]);
    assert_eq!(stream_2.next(), None);

    Ok(())
  }

  #[test]
  fn skip_4() -> Result<(), crate::TestFailure> {
    let mut stream = InputBitStream::<4>::from(&[0b00000000, 0b1110_0100, 0b0001_1011]);
    assert_eq!(stream.bytes_consumed(), Ok(1));

    assert_eq!(stream.skip_nodes(1), Ok(()));
    assert_eq!(stream.bytes_consumed(), Ok(2));
    assert_eq!(stream.skip_nodes(1), Ok(()));
    assert_eq!(stream.bytes_consumed(), Ok(2));

    assert_eq!(stream.skip_nodes(2), Ok(()));
    assert_eq!(stream.bytes_consumed(), Ok(3));

    assert_eq!(stream.skip_nodes(1), Err(DecodingError));

    Ok(())
  }

  #[test]
  fn read_8() -> Result<(), crate::TestFailure> {
    let mut stream = InputBitStream::<8>::from(&[0b00000000, 0b11100100, 0b00011011]);
    assert_eq!(stream.bytes_consumed(), Ok(1));
    assert_eq!(stream.next(), Some(0b11100100));
    assert_eq!(stream.bytes_consumed(), Ok(2));
    assert_eq!(stream.next(), Some(0b00011011));
    assert_eq!(stream.bytes_consumed(), Ok(3));
    assert_eq!(stream.next(), None);

    let mut stream_2 = InputBitStream::<8>::from(&[]);
    assert_eq!(stream_2.next(), None);

    Ok(())
  }

  #[test]
  fn skip_8() -> Result<(), crate::TestFailure> {
    let mut stream = InputBitStream::<8>::from(&[0b00000000, 0b11100100, 0b00011011]);
    assert_eq!(stream.bytes_consumed(), Ok(1));

    assert_eq!(stream.skip_nodes(1), Ok(()));
    assert_eq!(stream.bytes_consumed(), Ok(2));
    assert_eq!(stream.skip_nodes(1), Ok(()));
    assert_eq!(stream.bytes_consumed(), Ok(3));
    assert_eq!(stream.skip_nodes(1), Err(DecodingError));

    let mut stream_2 = InputBitStream::<8>::from(&[0b00000000, 0b11100100, 0b00011011]);
    assert_eq!(stream_2.bytes_consumed(), Ok(1));

    assert_eq!(stream_2.skip_nodes(2), Ok(()));
    assert_eq!(stream_2.bytes_consumed(), Ok(3));
    assert_eq!(stream_2.skip_nodes(1), Err(DecodingError));

    Ok(())
  }

  #[test]
  fn read_32() -> Result<(), crate::TestFailure> {
    let mut stream =
      InputBitStream::<32>::from(&[0b00000000, 0b00000000, 0b11111111, 0b11100100, 0b00011011]);

    assert_eq!(stream.bytes_consumed(), Ok(1));
    assert_eq!(stream.next(), Some(0b00011011_11100100_11111111_00000000));
    assert_eq!(stream.bytes_consumed(), Ok(5));
    assert_eq!(stream.next(), None);

    let mut stream_2 = InputBitStream::<32>::from(&[
      0b00000000, 0b00000000, 0b11111111, 0b11100100, 0b00011011, 0b00000001,
    ]);
    assert_eq!(stream_2.next(), Some(0b00011011_11100100_11111111_00000000));
    assert_eq!(stream_2.next(), None);

    let mut stream_3 = InputBitStream::<32>::from(&[]);
    assert_eq!(stream_3.next(), None);

    Ok(())
  }

  #[test]
  fn skip_32() -> Result<(), crate::TestFailure> {
    let mut stream =
      InputBitStream::<32>::from(&[0b00000000, 0b00000000, 0b11111111, 0b11100100, 0b00011011]);

    assert_eq!(stream.bytes_consumed(), Ok(1));

    assert_eq!(stream.skip_nodes(1), Ok(()));
    assert_eq!(stream.bytes_consumed(), Ok(5));
    assert_eq!(stream.skip_nodes(1), Err(DecodingError));

    Ok(())
  }
}

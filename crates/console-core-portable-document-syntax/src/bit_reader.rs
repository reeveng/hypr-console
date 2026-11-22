//! A simple bit reader and writer.

use alloc::vec;
use alloc::vec::Vec;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use core::fmt::Debug;

#[derive(Debug, Clone)]
pub struct BitReader<'a> {
  pub data: &'a [u8],
  cur_pos: u64,
}

impl<'a> BitReader<'a> {
  #[inline]
  pub fn new(data: &'a [u8]) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(bit_reader) = Self::new_with(data, 0);

    Ok(bit_reader)
  }

  #[inline]
  pub fn new_with(
    data: &'a [u8],
    cur_pos: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(position) = fixed_index(cur_pos);
    Ok(Self {
      data,
      cur_pos: position,
    })
  }

  #[inline]
  pub fn align(&mut self) -> Result<(), Never> {
    let remainder = self.cur_pos.wrapping_rem(8);
    let padding = 8_u64.wrapping_sub(remainder).wrapping_rem(8);
    self.cur_pos = self.cur_pos.wrapping_add(padding);

    Ok(())
  }

  #[inline(always)]
  pub fn read(
    &mut self,
    bit_size: u8,
  ) -> core::result::Result<Option<u32>, console_core_never::Never> {
    let sample = match bit_size {
      0 | 33..=u8::MAX => return Ok(None),
      8 => {
        let Ok(position) = ::console_core_number_conversion::fixed_index(match self.byte_pos() {
          ::core::result::Result::Ok(position) => position,
        });

        match self
          .data
          .get(match ::console_core_number_conversion::index(position) {
            ::core::result::Result::Ok(native_position) => native_position,
          }) {
          Some(byte) => u32::from(*byte),
          None => return Ok(None),
        }
      }
      1..=32 => {
        let Ok(bits) = self.read_unaligned(bit_size);

        match bits {
          Some(bits_2) => bits_2,
          None => return Ok(None),
        }
      }
    };
    self.cur_pos = self.cur_pos.wrapping_add(u64::from(bit_size));

    Ok(Some(sample))
  }

  fn read_unaligned(&self, width: u8) -> Result<Option<u32>, Never> {
    let Ok(position) = ::console_core_number_conversion::fixed_index(match self.byte_pos() {
      ::core::result::Result::Ok(position) => position,
    });
    let Ok(bit_position) = ::console_core_number_conversion::fixed_index(match self.bit_pos() {
      ::core::result::Result::Ok(bit_position) => bit_position,
    });
    let Ok(bit_offset) = console_core_number_conversion::wrapped::<_, u8>(
      match ::console_core_number_conversion::index(bit_position) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    );
    let covered = bit_offset.saturating_add(width).div_ceil(8);
    let Ok(byte_count) = index(covered);
    let source = match self.data.get(
      match ::console_core_number_conversion::index(position) {
        ::core::result::Result::Ok(native_position) => native_position,
      }..(match ::console_core_number_conversion::index(position) {
        ::core::result::Result::Ok(native_position) => native_position,
      })
      .saturating_add(byte_count),
    ) {
      Some(bytes) => bytes,
      None => return Ok(None),
    };
    let mut bytes = [0_u8; 8];

    for (target, value) in bytes.iter_mut().zip(source) {
      *target = *value;
    }

    let shift = 64_u32
      .saturating_sub(u32::from(bit_offset))
      .saturating_sub(u32::from(width));
    let raw = u64::from_be_bytes(bytes).wrapping_shr(shift);
    let Ok(sample) = console_core_number_conversion::wrapped::<_, u32>(raw);
    let Ok(mask) = bit_mask(width);

    Ok(Some(sample & mask))
  }

  pub fn peak(
    &mut self,
    bit_size: u8,
  ) -> core::result::Result<Option<u32>, console_core_never::Never> {
    let Ok(read) = self.clone().read(bit_size);

    Ok(read)
  }

  pub fn at_end(&self) -> core::result::Result<crate::End, console_core_never::Never> {
    let Ok(byte_pos) = ::console_core_number_conversion::fixed_index(match self.byte_pos() {
      ::core::result::Result::Ok(byte_pos) => byte_pos,
    });

    Ok(
      match ::core::primitive::bool::then_some(
        (match ::console_core_number_conversion::index(byte_pos) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) >= self.data.len(),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::End::Reached,
        ::core::option::Option::None => crate::End::Pending,
      },
    )
  }

  #[inline]
  pub fn byte_pos(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    index(self.cur_pos.wrapping_div(8))
  }

  #[inline]
  pub fn bit_pos(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    index(self.cur_pos.wrapping_rem(8))
  }

  #[inline]
  pub fn cur_pos(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    index(self.cur_pos)
  }

  #[inline]
  pub fn tail(&self) -> Result<&'a [u8], Never> {
    let Ok(position) = ::console_core_number_conversion::fixed_index(match self.byte_pos() {
      ::core::result::Result::Ok(position) => position,
    });

    Ok(
      match self.data.get(
        match ::console_core_number_conversion::index(position) {
          ::core::result::Result::Ok(native_position) => native_position,
        }..,
      ) {
        Some(bytes) => bytes,
        None => &[],
      },
    )
  }

  #[inline]
  pub fn cur_byte(&self) -> core::result::Result<Option<u8>, console_core_never::Never> {
    Ok(
      self
        .data
        .get(match self.byte_pos() {
          ::core::result::Result::Ok(checked_value_2) => checked_value_2,
        })
        .copied(),
    )
  }
}

pub fn bit_mask(bit_size: u8) -> core::result::Result<u32, console_core_never::Never> {
  Ok(match bit_size {
    0 => 0,
    1..=31 => u32::wrapping_sub(
      u32::wrapping_shl(1_u32, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(bit_size);
        numeric_value
      }),
      1,
    ),
    _ => u32::MAX,
  })
}

#[derive(Debug)]
pub struct BitWriter<'a> {
  data: &'a mut [u8],
  cur_pos: u64,
  bit_size: u8,
}

impl<'a> BitWriter<'a> {
  #[inline]
  pub fn new(
    data: &'a mut [u8],
    bit_size: u8,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    match ::core::primitive::bool::then_some(!(1..=32).contains(&bit_size), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    Ok(Some(Self {
      data,
      bit_size,
      cur_pos: 0,
    }))
  }

  #[inline]
  pub fn split_off(self) -> core::result::Result<(&'a [u8], Self), console_core_never::Never> {
    let Ok(position) = index(self.cur_pos.wrapping_div(8));
    let (left, right) = self.data.split_at_mut(position);

    Ok((
      left,
      BitWriter {
        data: right,
        cur_pos: 0,
        bit_size: self.bit_size,
      },
    ))
  }

  #[inline]
  pub fn align(&mut self) -> Result<(), Never> {
    let remainder = self.cur_pos.wrapping_rem(8);
    let padding = 8_u64.wrapping_sub(remainder).wrapping_rem(8);
    self.cur_pos = self.cur_pos.wrapping_add(padding);

    Ok(())
  }

  #[inline]
  pub fn cur_pos(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    index(self.cur_pos)
  }

  #[inline]
  pub fn get_data(&self) -> core::result::Result<&[u8], console_core_never::Never> {
    Ok(self.data)
  }

  #[inline]
  pub fn write(&mut self, val: u32) -> Result<Option<()>, Never> {
    let Ok(mask) = bit_mask(self.bit_size);
    let sample = val & mask;
    let mut left = self.bit_size;

    for _ in 0..self.bit_size {
      match left {
        0 => break,
        1.. => {}
      }

      let consumed = self.bit_size.saturating_sub(left);
      let position = self.cur_pos.wrapping_add(u64::from(consumed));
      let Ok(byte_position) = index(position.wrapping_div(8));
      let Ok(bit_offset) =
        console_core_number_conversion::wrapped::<_, u8>(position.wrapping_rem(8));
      let byte_bits = 8_u8.saturating_sub(bit_offset);
      let width = left.min(byte_bits);
      let remaining = left.saturating_sub(width);
      let shift = byte_bits.saturating_sub(width);
      let Ok(chunk_mask) = bit_mask(width);
      let portion = sample.wrapping_shr(u32::from(remaining)) & chunk_mask;
      let Ok(data) = console_core_number_conversion::wrapped::<_, u8>(portion);
      let Ok(mask_byte) = console_core_number_conversion::wrapped::<_, u8>(chunk_mask);
      let byte = match self.data.get_mut(byte_position) {
        Some(byte) => byte,
        None => return Ok(None),
      };
      let placed_mask = mask_byte.wrapping_shl(u32::from(shift));
      *byte = (*byte & !placed_mask) | (data.wrapping_shl(u32::from(shift)) & placed_mask);
      left = remaining;
    }

    self.cur_pos = self.cur_pos.wrapping_add(u64::from(self.bit_size));

    Ok(Some(()))
  }

  #[inline]
  pub fn fill_bytes(
    &mut self,
    byte: u8,
    count: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    match self.cur_pos.wrapping_rem(8) {
      0 => {}
      _unaligned => return Ok(None),
    }

    let Ok(start) = index(self.cur_pos.wrapping_div(8));
    let ::core::result::Result::Ok(end) =
      ::console_core_number_conversion::fixed_index(match start.checked_add(count) {
        Some(end) => end,
        None => return Ok(None),
      });

    match self.data.get_mut(
      start..match ::console_core_number_conversion::index(end) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    ) {
      Some(data) => data.fill(byte),
      None => return Ok(None),
    }

    let Ok(written) = fixed_index(count);
    self.cur_pos = self.cur_pos.wrapping_add(written.wrapping_mul(8));

    Ok(Some(()))
  }

  #[inline]
  pub fn write_bits(
    &mut self,
    bits: impl IntoIterator<Item = u32>,
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    for bit in bits {
      let value: u32 = bit;
      let Ok(explicit_value) = self.write(value);

      match explicit_value {
        Some(value_2) => value_2,
        None => return Ok(None),
      };
    }

    Ok(Some(()))
  }
}

pub struct BitChunks<'a> {
  reader: BitReader<'a>,
  bit_size: u8,
  chunk_len: u64,
}

impl<'a> BitChunks<'a> {
  pub fn new(
    data: &'a [u8],
    bit_size: u8,
    chunk_len: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    match ::core::primitive::bool::then_some(!(1..=16).contains(&bit_size) || chunk_len == 0, ())
    {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let Ok(reader) = BitReader::new(data);
    let Ok(samples) = fixed_index(chunk_len);

    Ok(Some(Self {
      reader,
      bit_size,
      chunk_len: samples,
    }))
  }
}

impl Iterator for BitChunks<'_> {
  type Item = BitChunk;

  fn next(&mut self) -> Option<Self::Item> {
    let mut bits = Vec::new();

    for _ in 0..self.chunk_len {
      let Ok(read) = self.reader.read(self.bit_size);
      let checked_value = read?;
      let Ok(own_read) = console_core_number_conversion::wrapped::<_, u16>(checked_value);
      bits.push(own_read);
    }

    Some(BitChunk { bits })
  }
}

#[derive(Debug, Clone)]
pub struct BitChunk {
  bits: Vec<u16>,
}

impl BitChunk {
  pub fn iter(
    &self,
  ) -> core::result::Result<impl Iterator<Item = u16> + '_, console_core_never::Never> {
    Ok(self.bits.iter().copied())
  }

  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "This public constructor creates exactly the sample count requested by the caller and PDF row counts are bounded before it is called"
    )
  )]
  pub fn new(
    val: u8,
    count: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      bits: vec![
        {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(val);
          numeric_value
        };
        count
      ],
    })
  }

  pub fn from_reader(
    bit_reader: &mut BitReader<'_>,
    bit_size: u8,
    chunk_len: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    match ::core::primitive::bool::then_some(!(1..=16).contains(&bit_size) || chunk_len == 0, ())
    {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let mut bits = Vec::new();

    for _ in 0..chunk_len {
      let Ok(read) = bit_reader.read(bit_size);
      bits.push({
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u16>(match read {
          Some(value) => value,
          None => return Ok(None),
        });
        numeric_value
      });
    }

    Ok(Some(Self { bits }))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_support::{Required, TestFailure};

  #[test]
  fn bit_reader_16() {
    let data = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
    let Ok(mut reader) = BitReader::new(&data);
    assert_eq!(
      reader.read(16),
      Ok(Some(u32::from(u16::from_be_bytes([0x01, 0x02]))))
    );
    assert_eq!(
      reader.read(16),
      Ok(Some(u32::from(u16::from_be_bytes([0x03, 0x04]))))
    );
    assert_eq!(
      reader.read(16),
      Ok(Some(u32::from(u16::from_be_bytes([0x05, 0x06]))))
    );
  }

  #[test]
  fn bit_writer_16() -> Result<(), TestFailure> {
    let mut buf = vec![0_u8; 6];
    let Ok(made) = BitWriter::new(&mut buf, 16);
    let mut writer = made.required()?;
    assert_eq!(
      writer.write(u32::from(u16::from_be_bytes([0x01, 0x02]))),
      Ok(Some(()))
    );
    assert_eq!(
      writer.write(u32::from(u16::from_be_bytes([0x03, 0x04]))),
      Ok(Some(()))
    );
    assert_eq!(
      writer.write(u32::from(u16::from_be_bytes([0x05, 0x06]))),
      Ok(Some(()))
    );

    assert_eq!(buf, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);

    Ok(())
  }

  #[test]
  fn bit_reader_12() {
    let data = [0b10011000, 0b00011111, 0b10101001, 0b11101001, 0b00011010];
    let Ok(mut reader) = BitReader::new(&data);
    assert_eq!(reader.read(12), Ok(Some(0b100110000001)));
    assert_eq!(reader.read(12), Ok(Some(0b111110101001)));
    assert_eq!(reader.read(12), Ok(Some(0b111010010001)));
  }

  #[test]
  fn bit_reader_9() {
    let data = [0b10011000, 0b00011111, 0b10101001, 0b11101001, 0b00011010];
    let Ok(mut reader) = BitReader::new(&data);
    assert_eq!(reader.read(9), Ok(Some(0b100110000)));
    assert_eq!(reader.read(9), Ok(Some(0b001111110)));
    assert_eq!(reader.read(9), Ok(Some(0b101001111)));
    assert_eq!(reader.read(9), Ok(Some(0b010010001)));
  }

  #[test]
  fn bit_writer_8() -> Result<(), TestFailure> {
    let mut buf = vec![0_u8; 3];
    let Ok(made) = BitWriter::new(&mut buf, 8);
    let mut writer = made.required()?;
    assert_eq!(writer.write(0x01), Ok(Some(())));
    assert_eq!(writer.write(0x02), Ok(Some(())));
    assert_eq!(writer.write(0x03), Ok(Some(())));

    assert_eq!(buf, [0x01, 0x02, 0x03]);

    Ok(())
  }

  #[test]
  fn bit_reader_8() {
    let data = [0x01, 0x02, 0x03];
    let Ok(mut reader) = BitReader::new(&data);
    assert_eq!(reader.read(8), Ok(Some(0x01)));
    assert_eq!(reader.read(8), Ok(Some(0x02)));
    assert_eq!(reader.read(8), Ok(Some(0x03)));
  }

  #[test]
  fn bit_writer_4() -> Result<(), TestFailure> {
    let mut buf = vec![0_u8; 3];
    let Ok(made) = BitWriter::new(&mut buf, 4);
    let mut writer = made.required()?;

    for value in [0b1001, 0b1000, 0b0001, 0b1111, 0b1010, 0b1001] {
      assert_eq!(writer.write(value), Ok(Some(())));
    }

    assert_eq!(buf, [0b10011000, 0b00011111, 0b10101001]);

    Ok(())
  }

  #[test]
  fn bit_reader_4() {
    let data = [0b10011000, 0b00011111, 0b10101001];
    let Ok(mut reader) = BitReader::new(&data);

    for expected in [0b1001, 0b1000, 0b0001, 0b1111, 0b1010, 0b1001] {
      assert_eq!(reader.read(4), Ok(Some(expected)));
    }
  }

  #[test]
  fn bit_writer_2() -> Result<(), TestFailure> {
    let mut buf = vec![0_u8; 2];
    let Ok(made) = BitWriter::new(&mut buf, 2);
    let mut writer = made.required()?;

    for value in [0b10, 0b01, 0b10, 0b00, 0b00, 0b01, 0b00, 0b00] {
      assert_eq!(writer.write(value), Ok(Some(())));
    }

    assert_eq!(buf, [0b10011000, 0b00010000]);

    Ok(())
  }

  #[test]
  fn bit_reader_2() {
    let data = [0b10011000, 0b00010000];
    let Ok(mut reader) = BitReader::new(&data);

    for expected in [0b10, 0b01, 0b10, 0b00, 0b00, 0b01, 0b00, 0b00] {
      assert_eq!(reader.read(2), Ok(Some(expected)));
    }
  }

  #[test]
  fn bit_writer_1() -> Result<(), TestFailure> {
    let mut buf = vec![0_u8; 2];
    let Ok(made) = BitWriter::new(&mut buf, 1);
    let mut writer = made.required()?;

    for value in [
      0b1, 0b0, 0b0, 0b1, 0b1, 0b0, 0b0, 0b0, 0b0, 0b0, 0b0, 0b1, 0b0, 0b0, 0b0, 0b0,
    ] {
      assert_eq!(writer.write(value), Ok(Some(())));
    }

    assert_eq!(buf, [0b10011000, 0b00010000]);

    Ok(())
  }

  #[test]
  fn bit_reader_1() {
    let data = [0b10011000, 0b00010000];
    let Ok(mut reader) = BitReader::new(&data);

    for expected in [
      0b1, 0b0, 0b0, 0b1, 0b1, 0b0, 0b0, 0b0, 0b0, 0b0, 0b0, 0b1, 0b0, 0b0, 0b0, 0b0,
    ] {
      assert_eq!(reader.read(1), Ok(Some(expected)));
    }
  }

  #[test]
  fn bit_reader_align() {
    let data = [0b10011000, 0b00010000];
    let Ok(mut reader) = BitReader::new(&data);

    for expected in [0b1, 0b0, 0b0, 0b1] {
      assert_eq!(reader.read(1), Ok(Some(expected)));
    }

    let Ok(()) = reader.align();

    for expected in [0b0, 0b0, 0b0, 0b1, 0b0, 0b0, 0b0, 0b0] {
      assert_eq!(reader.read(1), Ok(Some(expected)));
    }
  }

  #[test]
  fn bit_reader_chunks() -> Result<(), TestFailure> {
    let data = [0b10011000, 0b00010000];
    let Ok(made) = BitChunks::new(&data, 1, 3);
    let mut reader = made.required()?;
    assert_eq!(
      reader.next().map(|chunk| chunk.bits),
      Some(vec![0b1, 0b0, 0b0])
    );
    assert_eq!(
      reader.next().map(|chunk| chunk.bits),
      Some(vec![0b1, 0b1, 0b0])
    );
    assert_eq!(
      reader.next().map(|chunk| chunk.bits),
      Some(vec![0b0, 0b0, 0b0])
    );
    assert_eq!(
      reader.next().map(|chunk| chunk.bits),
      Some(vec![0b0, 0b0, 0b1])
    );
    assert_eq!(
      reader.next().map(|chunk| chunk.bits),
      Some(vec![0b0, 0b0, 0b0])
    );

    Ok(())
  }

  #[test]
  fn bit_reader_varying_bit_sizes() {
    let data = [0b10011000, 0b00011111, 0b10101001];
    let Ok(mut reader) = BitReader::new(&data);
    assert_eq!(reader.read(4), Ok(Some(0b1001)));
    assert_eq!(reader.read(1), Ok(Some(0b1)));
    assert_eq!(reader.read(4), Ok(Some(0b0000)));
    assert_eq!(reader.read(5), Ok(Some(0b00111)));
    assert_eq!(reader.read(1), Ok(Some(0b1)));
    assert_eq!(reader.read(2), Ok(Some(0b11)));
    assert_eq!(reader.read(7), Ok(Some(0b0101001)));
  }

  #[test]
  fn bit_writer_rejects_invalid_sizes() {
    let mut buf = [0_u8; 4];
    assert!(matches!(BitWriter::new(&mut buf, 0), Ok(None)));
    assert!(matches!(BitWriter::new(&mut buf, 33), Ok(None)));
  }

  #[test]
  fn bit_reader_rejects_invalid_sizes() {
    let data = [0xff];
    let Ok(mut reader) = BitReader::new(&data);
    assert_eq!(reader.read(0), Ok(None));
    assert_eq!(reader.read(33), Ok(None));
  }

  fn round_trip(bit_size: u8, values: &[u32]) -> Result<(), TestFailure> {
    let Ok(count) = console_core_number_conversion::fitted::<_, u32>(values.len());
    let total_bits = u32::from(bit_size).wrapping_mul(count);
    let Ok(bytes) = console_core_number_conversion::index(total_bits.div_ceil(8));
    let mut buf = vec![0_u8; bytes.min(128)];
    let Ok(made) = BitWriter::new(&mut buf, bit_size);
    let mut writer = made.required()?;

    for value in values {
      assert_eq!(writer.write(*value), Ok(Some(())));
    }

    let Ok(mut reader) = BitReader::new(&buf);

    for expected in values {
      assert_eq!(
        reader.read(bit_size),
        Ok(Some(*expected)),
        "round-trip failed for bit size {bit_size}"
      );
    }

    Ok(())
  }

  #[test]
  fn bit_writer_round_trip_all_bit_sizes() -> Result<(), TestFailure> {
    for bit_size in 1_u8..=32 {
      let Ok(mask) = bit_mask(bit_size);
      let sample_count: u32 = match ::core::primitive::bool::then_some(bit_size > 16, ()) {
        ::core::option::Option::Some(()) => 4,
        ::core::option::Option::None => 6,
      };
      let values: Vec<u32> = (0..sample_count)
        .map(|i| {
          (0x9E37_u32.wrapping_mul(i.wrapping_add(1)) ^ u32::from(bit_size).wrapping_mul(0x45))
            & mask
        })
        .collect();

      round_trip(bit_size, &values)?;
    }

    Ok(())
  }

  #[test]
  fn bit_writer_round_trip_12_and_25_bits() -> Result<(), TestFailure> {
    for bit_size in [12_u8, 25_u8] {
      let Ok(mask) = bit_mask(bit_size);
      let values: Vec<u32> = (0_u32..5)
        .map(|i| (0xABCDE_u32.wrapping_mul(i.wrapping_add(1)) ^ u32::from(bit_size)) & mask)
        .collect();

      round_trip(bit_size, &values)?;
    }

    Ok(())
  }
}

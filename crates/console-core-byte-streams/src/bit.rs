//! Bits read out of bytes, most significant first or least significant first.
//!
//! A reader keeps up to a word of bits ahead of what it has handed out, refilling it a word
//! at a time from a byte slice or a byte at a time from a stream. Reading most significant
//! first, the next bit is the top of that word and a read shifts it left; reading least
//! significant first, it is the bottom and a read shifts it right. A read wider than what is
//! held takes what is held, refills, and gathers the rest out of the new word.
//!
//! A codebook read needs the longest codeword's worth of bits ahead at once, which a stream
//! read a byte at a time cannot promise, so it is refused there rather than stopping the
//! program as upstream did.

use std::{cmp::Ordering, io};

use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use super::vlc::{Codebook, CodebookEntry, EntryKind};
use super::{ReadBytes, ahead, copied, finished};

fn end_of_bitstream<T>() -> io::Result<T> {
  Err(io::Error::other("unexpected end of bitstream"))
}

mod private {
  use std::io;

  pub trait FetchBits {
    fn fetch_bits(&mut self) -> io::Result<()>;

    fn fetch_bits_partial(&mut self) -> io::Result<()>;

    fn get_bits(&self) -> u64;

    fn num_bits_left(&self) -> u32;

    fn consume_bits(&mut self, num: u32);
  }
}

use private::FetchBits;

#[derive(Clone, Copy)]
enum Order {
  MostFirst,
  LeastFirst,
}

enum BufferedBits {
  Sufficient,
  NeedsRefill,
}

impl From<bool> for BufferedBits {
  fn from(sufficient: bool) -> Self {
    match bool::then_some(sufficient, ()) {
      Some(()) => Self::Sufficient,
      None => Self::NeedsRefill,
    }
  }
}

struct Window {
  bits: u64,
  available: u32,
}

struct Taken<'a> {
  word: [u8; 8],
  bits: u32,
  rest: &'a [u8],
}

fn taken_word(buf: &[u8], most: u64) -> Result<Taken<'_>, Never> {
  let Ok((taken, rest)) = ahead(buf, most.min(8));
  let mut word = [0; 8];
  let Ok(count) = copied(taken, &mut word);
  let Ok(narrow) = fitted::<u64, u32>(count);

  Ok(Taken {
    word,
    bits: narrow.wrapping_shl(3),
    rest,
  })
}

fn peeked(order: Order, bits: u64, width: u32) -> Result<u64, Never> {
  Ok(match order {
    Order::MostFirst => bits.wrapping_shr(u64::BITS.wrapping_sub(width)),
    Order::LeastFirst => bits & 1_u64.wrapping_shl(width).wrapping_sub(1),
  })
}

fn skipped(order: Order, bits: u64, width: u32) -> Result<u64, Never> {
  Ok(match order {
    Order::MostFirst => bits.wrapping_shl(width),
    Order::LeastFirst => bits.wrapping_shr(width),
  })
}

fn entry_at<E: CodebookEntry>(codebook: &Codebook<E>, at: u64) -> io::Result<E> {
  let Ok(slot) = index(at);

  match codebook.table.get(slot) {
    Some(entry) => Ok(*entry),
    None => Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "core (io): codeword is not in the codebook",
    )),
  }
}

fn decoded<E: CodebookEntry>(
  codebook: &Codebook<E>,
  order: Order,
  window: Window,
) -> io::Result<(E::ValueType, u32)> {
  let Ok(first) = peeked(order, window.bits, codebook.init_block_len);
  let entry = entry_at(codebook, first.saturating_add(1))?;

  let stepped = iterate(
    (window.bits, codebook.init_block_len, entry, 0_u32),
    |(bits, width, entry_2, consumed)| {
      Ok(match entry_2.kind() {
        EntryKind::Value => Step::Halt(Ok((entry_2, consumed))),
        EntryKind::Jump => {
          let spent = consumed.saturating_add(width);
          let Ok(rest) = skipped(order, bits, width);

          match BufferedBits::from(spent <= window.available) {
            BufferedBits::NeedsRefill => Step::Halt(end_of_bitstream()),
            BufferedBits::Sufficient => {
              let beneath = entry_2.jump_len();
              let Ok(at) = peeked(order, rest, beneath);

              match entry_at(
                codebook,
                u64::from(entry_2.jump_offset()).saturating_add(at),
              ) {
                Ok(next) => Step::Again((rest, beneath, next, spent)),
                Err(fault) => Step::Halt(Err(fault)),
              }
            }
          }
        }
      })
    },
  );

  let (found, consumed) = finished(stepped)?;
  let spent = consumed.saturating_add(found.value_len());

  match BufferedBits::from(spent <= window.available) {
    BufferedBits::NeedsRefill => end_of_bitstream(),
    BufferedBits::Sufficient => Ok((found.value(), spent)),
  }
}

fn ready<B: FetchBits + ?Sized>(reader: &mut B) -> io::Result<()> {
  match reader.num_bits_left() {
    0 => reader.fetch_bits(),
    1.. => Ok(()),
  }
}

fn topped<B: FetchBits + ?Sized>(reader: &mut B, wanted: u32) -> io::Result<()> {
  match BufferedBits::from(reader.num_bits_left() >= wanted) {
    BufferedBits::NeedsRefill => reader.fetch_bits_partial(),
    BufferedBits::Sufficient => Ok(()),
  }
}

fn gather<B: FetchBits + ?Sized>(
  reader: &mut B,
  order: Order,
  width: u32,
  first: u64,
) -> io::Result<(u32, u64)> {
  let stepped = iterate((reader, width, first), |(reader_2, wanted, gathered)| {
    Ok(match BufferedBits::from(wanted <= reader_2.num_bits_left()) {
      BufferedBits::Sufficient => Step::Halt(Ok((wanted, gathered))),
      BufferedBits::NeedsRefill => {
        let rest = wanted.saturating_sub(reader_2.num_bits_left());

        match reader_2.fetch_bits() {
          Ok(()) => {
            let word = reader_2.get_bits();

            let more = match order {
              Order::MostFirst => word.wrapping_shr(u64::BITS.wrapping_sub(rest)),
              Order::LeastFirst => word.wrapping_shl(width.wrapping_sub(rest)),
            };

            Step::Again((reader_2, rest, gathered | more))
          }
          Err(fault) => Step::Halt(Err(fault)),
        }
      }
    })
  });

  finished(stepped)
}

fn unary_ltr<B: FetchBits + ?Sized>(reader: &mut B, run: impl Fn(u64) -> u32) -> io::Result<u32> {
  let stepped = iterate((reader, 0_u32), |(reader_2, counted)| {
    let length = run(reader_2.get_bits());
    let left = reader_2.num_bits_left();

    Ok(match BufferedBits::from(length < left) {
      BufferedBits::NeedsRefill => match reader_2.fetch_bits() {
        Ok(()) => Step::Again((reader_2, counted.saturating_add(left))),
        Err(fault) => Step::Halt(Err(fault)),
      },
      BufferedBits::Sufficient => {
        reader_2.consume_bits(length);
        reader_2.consume_bits(1);

        Step::Halt(Ok(counted.saturating_add(length)))
      }
    })
  });

  finished(stepped)
}

fn capped_ltr<B: FetchBits + ?Sized>(
  reader: &mut B,
  limit: u32,
  run: impl Fn(u64) -> u32,
) -> io::Result<u32> {
  let stepped = iterate((reader, limit, 0_u32), |(reader_2, remaining, counted)| {
    let left = reader_2.num_bits_left();
    let length = run(reader_2.get_bits()).min(left);

    Ok(match length.cmp(&remaining) {
      Ordering::Equal | Ordering::Greater => {
        reader_2.consume_bits(remaining);

        Step::Halt(Ok(counted.saturating_add(remaining)))
      }
      Ordering::Less => match BufferedBits::from(length < left) {
        BufferedBits::Sufficient => {
          reader_2.consume_bits(length);
          reader_2.consume_bits(1);

          Step::Halt(Ok(counted.saturating_add(length)))
        }
        BufferedBits::NeedsRefill => match reader_2.fetch_bits() {
          Ok(()) => Step::Again((
            reader_2,
            remaining.saturating_sub(length),
            counted.saturating_add(length),
          )),
          Err(fault) => Step::Halt(Err(fault)),
        },
      },
    })
  });

  finished(stepped)
}

pub trait FiniteBitStream {
  fn bits_left(&self) -> u64;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlagBit {
  Zero,
  One,
}

impl From<bool> for FlagBit {
  fn from(set: bool) -> Self {
    match bool::then_some(set, ()) {
      Some(()) => Self::One,
      None => Self::Zero,
    }
  }
}

pub trait ReadBitsLtr: FetchBits {
  #[inline(always)]
  fn realign(&mut self) -> Result<(), Never> {
    let skip = self.num_bits_left() & 0x7;

    self.consume_bits(skip);

    Ok(())
  }

  #[inline(always)]
  fn ignore_bits(&mut self, num_bits: u32) -> io::Result<()> {
    let (rest, _gathered) = gather(self, Order::MostFirst, num_bits, 0)?;

    self.consume_bits(rest.saturating_sub(1));
    self.consume_bits(rest.min(1));

    Ok(())
  }

  #[inline(always)]
  fn ignore_bit(&mut self) -> io::Result<()> {
    self.ignore_bits(1)
  }

  #[inline(always)]
  fn read_flag(&mut self) -> io::Result<FlagBit> {
    self.read_bit().map(|bit| FlagBit::from(bit == 1))
  }

  #[inline(always)]
  fn read_bit(&mut self) -> io::Result<u32> {
    ready(self)?;

    let Ok(bit) = fitted::<u64, u32>(self.get_bits().wrapping_shr(63));

    self.consume_bits(1);

    Ok(bit)
  }

  #[inline(always)]
  fn read_bits_leq32(&mut self, bit_width: u32) -> io::Result<u32> {
    let first = self
      .get_bits()
      .wrapping_shr(u32::BITS)
      .wrapping_shr(u32::BITS.wrapping_sub(bit_width));
    let (rest, gathered) = gather(self, Order::MostFirst, bit_width, first)?;

    self.consume_bits(rest);

    let Ok(bits) = fitted::<u64, u32>(gathered & u64::from(u32::MAX));

    Ok(bits)
  }

  #[inline(always)]
  fn read_bits_leq32_signed(&mut self, bit_width: u32) -> io::Result<i32> {
    let value = self.read_bits_leq32(bit_width)?;
    let shift = u32::BITS.wrapping_sub(bit_width);

    Ok(value.wrapping_shl(shift).cast_signed().wrapping_shr(shift))
  }

  #[inline(always)]
  fn read_bits_leq64(&mut self, bit_width: u32) -> io::Result<u64> {
    match bit_width {
      0 => Ok(0),
      _ => {
        let first = self
          .get_bits()
          .wrapping_shr(u64::BITS.wrapping_sub(bit_width));
        let (rest, gathered) = gather(self, Order::MostFirst, bit_width, first)?;

        self.consume_bits(rest.saturating_sub(1));
        self.consume_bits(1);

        Ok(gathered)
      }
    }
  }

  #[inline(always)]
  fn read_unary_zeros(&mut self) -> io::Result<u32> {
    unary_ltr(self, u64::leading_zeros)
  }

  #[inline(always)]
  fn read_unary_ones(&mut self) -> io::Result<u32> {
    unary_ltr(self, u64::leading_ones)
  }

  #[inline(always)]
  fn read_unary_ones_capped(&mut self, limit: u32) -> io::Result<u32> {
    capped_ltr(self, limit, u64::leading_ones)
  }

  #[inline(always)]
  fn read_codebook<E: CodebookEntry>(
    &mut self,
    codebook: &Codebook<E>,
  ) -> io::Result<(E::ValueType, u32)> {
    topped(self, codebook.max_code_len)?;

    let window = Window {
      bits: self.get_bits(),
      available: self.num_bits_left(),
    };
    let (value, consumed) = decoded(codebook, Order::MostFirst, window)?;

    self.consume_bits(consumed);

    Ok((value, consumed))
  }
}

pub trait ReadBitsRtl: FetchBits {
  #[inline(always)]
  fn ignore_bits(&mut self, num_bits: u32) -> io::Result<()> {
    let (rest, _gathered) = gather(self, Order::LeastFirst, num_bits, 0)?;

    self.consume_bits(rest.saturating_sub(1));
    self.consume_bits(rest.min(1));

    Ok(())
  }

  #[inline(always)]
  fn read_flag(&mut self) -> io::Result<FlagBit> {
    ready(self)?;

    let bit = FlagBit::from(self.get_bits() & 1 == 1);

    self.consume_bits(1);

    Ok(bit)
  }

  #[inline(always)]
  fn read_bits_leq32(&mut self, bit_width: u32) -> io::Result<u32> {
    let first = self.get_bits();
    let (rest, gathered) = gather(self, Order::LeastFirst, bit_width, first)?;

    self.consume_bits(rest);

    let mask = !u64::MAX.wrapping_shl(bit_width);
    let Ok(bits) = fitted::<u64, u32>(gathered & mask & u64::from(u32::MAX));

    Ok(bits)
  }

  #[inline(always)]
  fn read_bits_leq64(&mut self, bit_width: u32) -> io::Result<u64> {
    match bit_width {
      0 => Ok(0),
      _ => {
        let first = self.get_bits();
        let (rest, gathered) = gather(self, Order::LeastFirst, bit_width, first)?;

        self.consume_bits(rest.saturating_sub(1));
        self.consume_bits(1);

        Ok(
          gathered
            & !u64::MAX
              .wrapping_shl(bit_width.wrapping_sub(1))
              .wrapping_shl(1),
        )
      }
    }
  }

  #[inline(always)]
  fn read_codebook<E: CodebookEntry>(
    &mut self,
    codebook: &Codebook<E>,
  ) -> io::Result<(E::ValueType, u32)> {
    topped(self, codebook.max_code_len)?;

    let window = Window {
      bits: self.get_bits(),
      available: self.num_bits_left(),
    };
    let (value, consumed) = decoded(codebook, Order::LeastFirst, window)?;

    self.consume_bits(consumed);

    Ok((value, consumed))
  }
}

pub struct BitStreamLtr<'a, B: ReadBytes> {
  reader: &'a mut B,
  bits: u64,
  n_bits_left: u32,
}

impl<'a, B: ReadBytes> BitStreamLtr<'a, B> {
  pub fn new(reader: &'a mut B) -> Result<Self, Never> {
    Ok(BitStreamLtr {
      reader,
      bits: 0,
      n_bits_left: 0,
    })
  }
}

impl<B: ReadBytes> FetchBits for BitStreamLtr<'_, B> {
  #[inline(always)]
  fn fetch_bits(&mut self) -> io::Result<()> {
    let byte = self.reader.read_u8()?;

    self.bits = u64::from(byte).wrapping_shl(56);
    self.n_bits_left = u8::BITS;

    Ok(())
  }

  #[inline(always)]
  fn fetch_bits_partial(&mut self) -> io::Result<()> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      "a codebook is read from a byte slice rather than a byte stream",
    ))
  }

  #[inline(always)]
  fn get_bits(&self) -> u64 {
    self.bits
  }

  #[inline(always)]
  fn num_bits_left(&self) -> u32 {
    self.n_bits_left
  }

  #[inline(always)]
  fn consume_bits(&mut self, num: u32) {
    self.n_bits_left = self.n_bits_left.saturating_sub(num);

    self.bits = match self.bits.checked_shl(num) {
      Some(bits) => bits,
      None => 0,
    };
  }
}

impl<B: ReadBytes> ReadBitsLtr for BitStreamLtr<'_, B> {}

pub struct BitReaderLtr<'a> {
  buf: &'a [u8],
  bits: u64,
  n_bits_left: u32,
}

impl<'a> BitReaderLtr<'a> {
  pub fn new(buf: &'a [u8]) -> Result<Self, Never> {
    Ok(BitReaderLtr {
      buf,
      bits: 0,
      n_bits_left: 0,
    })
  }
}

impl FetchBits for BitReaderLtr<'_> {
  #[inline]
  fn fetch_bits_partial(&mut self) -> io::Result<()> {
    let room = u64::from(u64::BITS.saturating_sub(self.n_bits_left).wrapping_shr(3));
    let Ok(taken) = taken_word(self.buf, room);

    self.buf = taken.rest;
    self.bits |= u64::from_be_bytes(taken.word).wrapping_shr(self.n_bits_left);
    self.n_bits_left = self.n_bits_left.saturating_add(taken.bits);

    Ok(())
  }

  fn fetch_bits(&mut self) -> io::Result<()> {
    let Ok(taken) = taken_word(self.buf, 8);

    match taken.bits {
      0 => end_of_bitstream(),
      _ => {
        self.buf = taken.rest;
        self.bits = u64::from_be_bytes(taken.word);
        self.n_bits_left = taken.bits;

        Ok(())
      }
    }
  }

  #[inline(always)]
  fn get_bits(&self) -> u64 {
    self.bits
  }

  #[inline(always)]
  fn num_bits_left(&self) -> u32 {
    self.n_bits_left
  }

  #[inline(always)]
  fn consume_bits(&mut self, num: u32) {
    self.n_bits_left = self.n_bits_left.saturating_sub(num);

    self.bits = match self.bits.checked_shl(num) {
      Some(bits) => bits,
      None => 0,
    };
  }
}

impl ReadBitsLtr for BitReaderLtr<'_> {}

impl FiniteBitStream for BitReaderLtr<'_> {
  fn bits_left(&self) -> u64 {
    let Ok(bytes) = fitted::<_, u64>(self.buf.len());

    bytes
      .saturating_mul(8)
      .saturating_add(u64::from(self.n_bits_left))
  }
}

pub struct BitReaderRtl<'a> {
  buf: &'a [u8],
  bits: u64,
  n_bits_left: u32,
}

impl<'a> BitReaderRtl<'a> {
  pub fn new(buf: &'a [u8]) -> Result<Self, Never> {
    Ok(BitReaderRtl {
      buf,
      bits: 0,
      n_bits_left: 0,
    })
  }
}

impl FetchBits for BitReaderRtl<'_> {
  #[inline]
  fn fetch_bits_partial(&mut self) -> io::Result<()> {
    let room = u64::from(u64::BITS.saturating_sub(self.n_bits_left).wrapping_shr(3));
    let Ok(taken) = taken_word(self.buf, room);

    self.buf = taken.rest;
    self.bits |= u64::from_le_bytes(taken.word).wrapping_shl(self.n_bits_left);
    self.n_bits_left = self.n_bits_left.saturating_add(taken.bits);

    Ok(())
  }

  fn fetch_bits(&mut self) -> io::Result<()> {
    let Ok(taken) = taken_word(self.buf, 8);

    match taken.bits {
      0 => end_of_bitstream(),
      _ => {
        self.buf = taken.rest;
        self.bits = u64::from_le_bytes(taken.word);
        self.n_bits_left = taken.bits;

        Ok(())
      }
    }
  }

  #[inline(always)]
  fn get_bits(&self) -> u64 {
    self.bits
  }

  #[inline(always)]
  fn num_bits_left(&self) -> u32 {
    self.n_bits_left
  }

  #[inline(always)]
  fn consume_bits(&mut self, num: u32) {
    self.n_bits_left = self.n_bits_left.saturating_sub(num);

    self.bits = match self.bits.checked_shr(num) {
      Some(bits) => bits,
      None => 0,
    };
  }
}

impl ReadBitsRtl for BitReaderRtl<'_> {}

impl FiniteBitStream for BitReaderRtl<'_> {
  fn bits_left(&self) -> u64 {
    let Ok(bytes) = fitted::<_, u64>(self.buf.len());

    bytes
      .saturating_mul(8)
      .saturating_add(u64::from(self.n_bits_left))
  }
}

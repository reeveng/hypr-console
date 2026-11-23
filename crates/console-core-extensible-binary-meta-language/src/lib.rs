//! Extensible Binary Meta Language variable integers, element values and borrowed child elements.
//!
//! Extracted from the clean-room Matroska reader carried in this workspace.
//! MIT and Apache-2.0 licenses are retained beside this crate.
//!
//! A variable integer is one to eight bytes whose leading zeros give its width and whose first set
//! bit is a marker that is not part of the value; `read_vint` also says whether every value bit was
//! set, which is the reserved "unknown" of an element size and a reserved element id. An id keeps
//! its marker and is at most four bytes, and the one id whose value bits are all zero is `0x80`,
//! because Matroska's `ChapterDisplay` is spelled that way. A signed vint is the value less
//! `2^(7 * width - 1) - 1`, which is what lacing deltas use.
//!
//! `write_id` and `write_size` give the shortest spelling, and a size that would come out as all
//! ones takes one byte more, because that spelling already means "unknown". `write_unknown_size`
//! takes a width of one to eight and clamps what is outside it.
//!
//! `uint`, `int`, `float` and `string` read the four kinds of element value. An integer takes at
//! most eight bytes, a float is four or eight bytes and zero otherwise, and a string ends at its
//! first NUL and is repaired where it is not UTF-8.
//!
//! `Children` walks the elements inside a master element that is already in memory. An element
//! whose size overruns its parent is cut at the parent's end and says so, an unknown size runs to
//! the parent's end, and a header that cannot be decoded ends the walk with `Condition::Damaged`.
//!
//! A width is a `u32` and an offset a `u64`, because a width is a count of bytes in a number and an
//! offset is a place in a file, and neither is the machine's; the one meeting with `usize` is the
//! slice a width is taken from. What may be absent comes back as `Result<Option<_>, Never>`.
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;
use alloc::{string::String, vec::Vec};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const LONGEST_VINT: u32 = 8;
const LONGEST_ID: u32 = 4;
const VALUE_BITS_PER_BYTE: u32 = 7;

pub fn vint_len(first: u8) -> Result<Option<u32>, Never> {
  Ok(match first {
    0 => None,
    _ => first.leading_zeros().checked_add(1),
  })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
  Known(u64),
  Unknown,
}

impl Size {
  pub fn known(self) -> Result<Option<u64>, Never> {
    Ok(match self {
      Size::Known(size) => Some(size),
      Size::Unknown => None,
    })
  }
}

fn value_mask(width: u32) -> Result<u64, Never> {
  Ok(
    1u64
      .wrapping_shl(width.saturating_mul(VALUE_BITS_PER_BYTE))
      .wrapping_sub(1),
  )
}

fn leading_width(data: &[u8]) -> Result<Option<u32>, Never> {
  Ok(data.first().and_then(|first| {
    let Ok(width) = vint_len(*first);

    width
  }))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VintBits {
  AllOnes,
  ContainsZero,
}

impl From<bool> for VintBits {
  fn from(all_ones: bool) -> Self {
    match bool::then_some(all_ones, ()) {
      Some(()) => Self::AllOnes,
      None => Self::ContainsZero,
    }
  }
}

pub fn read_vint(data: &[u8]) -> Result<Option<(u64, u32, VintBits)>, Never> {
  let Ok(leading) = leading_width(data);

  Ok(leading.and_then(|width| {
    let Ok(count) = index(width);
    let bytes = data.get(..count)?;
    let Ok(mask) = value_mask(width);
    let raw = bytes.iter().fold(0u64, |so_far, byte| {
      so_far.wrapping_shl(8) | u64::from(*byte)
    });
    let value = raw & mask;

    Some((value, width, VintBits::from(value == mask)))
  }))
}

pub fn read_size(data: &[u8]) -> Result<Option<(Size, u32)>, Never> {
  let Ok(read) = read_vint(data);

  Ok(read.map(|(value, width, bits)| match bits {
    VintBits::AllOnes => (Size::Unknown, width),
    VintBits::ContainsZero => (Size::Known(value), width),
  }))
}

pub fn read_id(data: &[u8]) -> Result<Option<(u32, u32)>, Never> {
  let Ok(leading) = leading_width(data);

  Ok(
    leading
      .filter(|width| *width <= LONGEST_ID)
      .and_then(|width| {
        let Ok(count) = index(width);
        let bytes = data.get(..count)?;
        let Ok(read) = read_vint(bytes);
        let (value, _, bits) = read?;
        let id = bytes.iter().fold(0u32, |so_far, byte| {
          so_far.wrapping_shl(8) | u32::from(*byte)
        });

        match (bits, value, width) {
          (VintBits::AllOnes, _, _) | (VintBits::ContainsZero, 0, 2..) => None,
          (VintBits::ContainsZero, 0, 0 | 1) | (VintBits::ContainsZero, 1.., _) => {
            Some((id, width))
          }
        }
      }),
  )
}

pub fn read_svint(data: &[u8]) -> Result<Option<(i64, u32)>, Never> {
  let Ok(read) = read_vint(data);

  Ok(read.map(|(value, width, _)| {
    let Ok(mask) = value_mask(width);
    let Ok(bias) = fitted::<u64, i64>(mask.wrapping_shr(1));
    let Ok(signed) = fitted::<u64, i64>(value);

    (signed.wrapping_sub(bias), width)
  }))
}

pub fn write_id(out: &mut Vec<u8>, id: u32) -> Result<(), Never> {
  let [a, b, c, d] = id.to_be_bytes();

  match id {
    0x100_0000.. => out.extend_from_slice(&[a, b, c, d]),
    0x1_0000..=0xFF_FFFF => out.extend_from_slice(&[b, c, d]),
    0x100..=0xFFFF => out.extend_from_slice(&[c, d]),
    0..=0xFF => out.push(d),
  }

  Ok(())
}

pub fn write_size(out: &mut Vec<u8>, size: u64, min_len: u32) -> Result<(), Never> {
  let shortest = (1..LONGEST_VINT).find(|width| {
    let Ok(mask) = value_mask(*width);

    size < mask
  });
  let width = match shortest {
    Some(width) => width,
    None => LONGEST_VINT,
  }
  .max(min_len)
  .min(LONGEST_VINT);
  let marked = size | 1u64.wrapping_shl(width.saturating_mul(VALUE_BITS_PER_BYTE));
  let Ok(skip) = index(LONGEST_VINT.saturating_sub(width));

  match marked.to_be_bytes().get(skip..) {
    Some(tail) => out.extend_from_slice(tail),
    None => {}
  }

  Ok(())
}

pub fn write_unknown_size(out: &mut Vec<u8>, len: u32) -> Result<(), Never> {
  let rest = len.clamp(1, LONGEST_VINT).saturating_sub(1);
  let Ok(more) = index(rest);

  out.push(0xFFu8.wrapping_shr(rest));
  out.extend(core::iter::repeat_n(0xFF, more));

  Ok(())
}

pub fn uint(data: &[u8]) -> Result<u64, Never> {
  let Ok(eight) = index(LONGEST_VINT);

  Ok(data.iter().take(eight).fold(0u64, |so_far, byte| {
    so_far.wrapping_shl(8) | u64::from(*byte)
  }))
}

pub fn int(data: &[u8]) -> Result<i64, Never> {
  let Ok(eight) = index(LONGEST_VINT);
  let taken = match data.get(..eight) {
    Some(first_eight) => first_eight,
    None => data,
  };
  let Ok(value) = uint(taken);
  let Ok(count) = fitted::<_, u32>(taken.len());
  let shift = 64u32.saturating_sub(count.saturating_mul(8));

  Ok(i64::from_be_bytes(value.wrapping_shl(shift).to_be_bytes()).wrapping_shr(shift))
}

pub fn float(data: &[u8]) -> Result<f64, Never> {
  Ok(match data {
    [a, b, c, d] => f64::from(f32::from_be_bytes([*a, *b, *c, *d])),
    [a, b, c, d, e, f, g, h] => f64::from_be_bytes([*a, *b, *c, *d, *e, *f, *g, *h]),
    _ => 0.0,
  })
}

pub fn string(data: &[u8]) -> Result<String, Never> {
  Ok(match data.split(|byte| *byte == 0).next() {
    #[cfg_attr(
      dylint_lib = "explicit089_no_lossy_text",
      expect(
        explicit089_no_lossy_text,
        reason = "EBML text recovery deliberately preserves valid text around malformed bytes with U+FFFD, as specified by this reader and covered by its tests"
      )
    )]
    Some(head) => String::from_utf8_lossy(head).into_owned(),
    None => String::new(),
  })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extent {
  Declared,
  Unknown,
  Truncated,
}

#[derive(Clone, Copy, Debug)]
pub struct Elem<'a> {
  pub id: u32,
  pub offset: u64,
  pub header_len: u32,
  pub data: &'a [u8],
  pub extent: Extent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
  Intact,
  Damaged,
}

pub struct Children<'a> {
  data: &'a [u8],
  pos: u64,
  condition: Condition,
}

impl<'a> From<&'a [u8]> for Children<'a> {
  fn from(data: &'a [u8]) -> Self {
    Self {
      data,
      pos: 0,
      condition: Condition::Intact,
    }
  }
}

fn header_of(rest: &[u8]) -> Result<Option<(u32, Size, u32)>, Never> {
  let Ok(identified) = read_id(rest);

  Ok(identified.and_then(|(id, id_len)| {
    let Ok(skip) = index(id_len);
    let tail = rest.get(skip..)?;
    let Ok(sized) = read_size(tail);
    let (size, size_len) = sized?;

    Some((id, size, id_len.saturating_add(size_len)))
  }))
}

impl<'a> Children<'a> {
  pub fn condition(&self) -> Result<Condition, Never> {
    Ok(self.condition)
  }

  fn damaged(&mut self) -> Result<Option<Elem<'a>>, Never> {
    let Ok(end) = fitted::<_, u64>(self.data.len());

    self.condition = Condition::Damaged;
    self.pos = end;

    Ok(None)
  }

  fn element(
    &mut self,
    rest: &'a [u8],
    header: (u32, Size, u32),
  ) -> Result<Option<Elem<'a>>, Never> {
    let (id, size, header_len) = header;
    let Ok(skip) = index(header_len);
    let body = match rest.get(skip..) {
      Some(body) => body,
      None => return self.damaged(),
    };
    let Ok(available) = fitted::<_, u64>(body.len());
    let (wanted, extent) = match size {
      Size::Known(declared) => match declared.cmp(&available) {
        core::cmp::Ordering::Less | core::cmp::Ordering::Equal => (declared, Extent::Declared),
        core::cmp::Ordering::Greater => (available, Extent::Truncated),
      },
      Size::Unknown => (available, Extent::Unknown),
    };
    let Ok(len) = index(wanted);
    let data = match body.get(..len) {
      Some(data) => data,
      None => return self.damaged(),
    };
    let offset = self.pos;

    self.pos = offset
      .saturating_add(u64::from(header_len))
      .saturating_add(wanted);

    Ok(Some(Elem {
      id,
      offset,
      header_len,
      data,
      extent,
    }))
  }
}

impl<'a> Iterator for Children<'a> {
  type Item = Elem<'a>;

  fn next(&mut self) -> Option<Elem<'a>> {
    let Ok(at) = index(self.pos);
    let rest = self.data.get(at..)?;

    match rest {
      [] => None,
      [_first, ..] => {
        let Ok(header) = header_of(rest);

        match header {
          Some(found) => {
            let Ok(next) = self.element(rest, found);

            next
          }
          None => {
            let Ok(next) = self.damaged();

            next
          }
        }
      }
    }
  }
}

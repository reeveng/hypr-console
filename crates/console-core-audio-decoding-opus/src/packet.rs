//! The table-of-contents byte and the framing of a packet (RFC 6716 section 3).
//!
//! A packet is a TOC byte and one to forty-eight frames, and the byte says how
//! the frames are counted: one, two of the same size, two of different sizes,
//! or an arbitrary number behind a count byte that may announce padding. All
//! the rules of section 3.4 that a malformed packet breaks (R1 to R7) are the
//! one error, [`Error::MalformedFraming`]. The last stream of a multistream
//! packet is framed like any packet and every stream before it is
//! self-delimited (appendix B): it carries the size of its last frame, which is
//! how a decoder knows where the next stream begins.
//!
//! A size is at most 1275 and a packet at most a few thousand bytes, so the
//! sums here are far from the ends of their widths; they wrap rather than
//! saturate only because a malformed size is refused by the comparison that
//! follows it either way. A byte asked for past the end of the packet reads as
//! nothing, and the length checks after it are what refuse the packet.

use std::cmp::Ordering;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::Error;

const LONGEST_FRAME: i32 = 1275;
const LONGEST_PACKET_SAMPLES: u32 = 5760;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
  SilkOnly,
  Hybrid,
  CeltOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Bandwidth {
  Narrowband,
  Mediumband,
  Wideband,
  SuperWideband,
  Fullband,
}

impl Bandwidth {
  pub(crate) fn celt_end_band(self) -> Result<u32, Never> {
    Ok(match self {
      Bandwidth::Narrowband => 13,
      Bandwidth::Mediumband | Bandwidth::Wideband => 17,
      Bandwidth::SuperWideband => 19,
      Bandwidth::Fullband => 21,
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Channels {
  Mono,
  Stereo,
}

impl Channels {
  pub(crate) fn count(self) -> Result<u32, Never> {
    Ok(match self {
      Channels::Mono => 1,
      Channels::Stereo => 2,
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Toc {
  pub config: u8,
  pub mode: Mode,
  pub bandwidth: Bandwidth,
  pub layout: Channels,
  pub code: u8,
}

impl Toc {
  pub(crate) fn parse(byte: u8) -> Result<Toc, Never> {
    let config = byte.wrapping_shr(3);

    let (mode, bandwidth) = match config {
      0..=3 => (Mode::SilkOnly, Bandwidth::Narrowband),
      4..=7 => (Mode::SilkOnly, Bandwidth::Mediumband),
      8..=11 => (Mode::SilkOnly, Bandwidth::Wideband),
      12..=13 => (Mode::Hybrid, Bandwidth::SuperWideband),
      14..=15 => (Mode::Hybrid, Bandwidth::Fullband),
      16..=19 => (Mode::CeltOnly, Bandwidth::Narrowband),
      20..=23 => (Mode::CeltOnly, Bandwidth::Wideband),
      24..=27 => (Mode::CeltOnly, Bandwidth::SuperWideband),
      _fullband => (Mode::CeltOnly, Bandwidth::Fullband),
    };
    let layout = match byte & 4 {
      0 => Channels::Mono,
      _stereo => Channels::Stereo,
    };

    Ok(Toc {
      config,
      mode,
      bandwidth,
      layout,
      code: byte & 3,
    })
  }

  pub(crate) fn frame_samples(&self) -> Result<u32, Never> {
    let config = u32::from(self.config);
    let step = config & 3;

    Ok(match self.mode {
      Mode::CeltOnly => 120u32.wrapping_shl(step),
      Mode::Hybrid => match config & 1 {
        0 => 480,
        _twenty_milliseconds => 960,
      },
      Mode::SilkOnly => match step {
        3 => 2880,
        shorter => 480u32.wrapping_shl(shorter),
      },
    })
  }
}

#[derive(Clone, Debug)]
pub(crate) struct Packet<'a> {
  pub toc: Toc,
  pub frames: Vec<&'a [u8]>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Framing {
  Regular,
  SelfDelimited,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Spacing {
  Constant,
  Varying,
}

struct Cursor<'a> {
  data: &'a [u8],
  at: u32,
  remaining: i32,
}

struct Layout {
  count: u32,
  sizes: Vec<i32>,
  last: i32,
  spacing: Spacing,
  padding: u32,
}

pub(crate) fn stretch(
  data: &[u8],
  crate::quantities::StartOffset(from): crate::quantities::StartOffset,
  crate::quantities::ByteLength(length): crate::quantities::ByteLength,
) -> Result<&[u8], Never> {
  let Ok(start) = index(from);
  let Ok(span) = index(length);

  Ok(match data.get(start..start.saturating_add(span)) {
    Some(found) => found,
    None => &[],
  })
}

impl Cursor<'_> {
  fn byte(&mut self) -> Result<u8, Error> {
    match self.remaining.cmp(&1) {
      Ordering::Less => Err(Error::MalformedFraming),
      Ordering::Equal | Ordering::Greater => {
        let Ok(place) = index(self.at);

        let byte = match self.data.get(place) {
          Some(byte) => *byte,
          None => 0,
        };

        self.at = self.at.wrapping_add(1);
        self.remaining = self.remaining.wrapping_sub(1);

        Ok(byte)
      }
    }
  }

  fn size(&mut self) -> Result<i32, Error> {
    let Ok(room) = fitted::<_, u32>(self.remaining.max(0));
    let Ok(available) = stretch(
      self.data,
      crate::quantities::StartOffset(self.at),
      crate::quantities::ByteLength(room),
    );
    let (size, used) = read_size(available)?;
    let Ok(spent) = fitted::<_, i32>(used);

    self.at = self.at.wrapping_add(used);
    self.remaining = self.remaining.wrapping_sub(spent);

    Ok(size)
  }
}

fn read_size(available: &[u8]) -> Result<(i32, u32), Error> {
  match available {
    [] => Err(Error::MalformedFraming),
    [first, rest @ ..] => match ((*first).cmp(&252), rest.first()) {
      (Ordering::Less, _) => Ok((i32::from(*first), 1)),
      (Ordering::Equal | Ordering::Greater, Some(second)) => Ok((
        i32::from(*second)
          .wrapping_mul(4)
          .wrapping_add(i32::from(*first)),
        2,
      )),
      (Ordering::Equal | Ordering::Greater, None) => Err(Error::MalformedFraming),
    },
  }
}

fn equal_pair(cursor: &Cursor<'_>, framing: Framing) -> Result<Layout, Error> {
  let mut layout = Layout {
    count: 2,
    sizes: Vec::new(),
    last: cursor.remaining,
    spacing: Spacing::Constant,
    padding: 0,
  };

  match framing {
    Framing::SelfDelimited => Ok(layout),
    Framing::Regular => match cursor.remaining & 1 {
      0 => {
        layout.last = cursor.remaining.wrapping_shr(1);
        layout.sizes.push(layout.last);

        Ok(layout)
      }
      _odd => Err(Error::MalformedFraming),
    },
  }
}

fn unequal_pair(cursor: &mut Cursor<'_>) -> Result<Layout, Error> {
  let first = cursor.size()?;

  match first.cmp(&cursor.remaining) {
    Ordering::Greater => Err(Error::MalformedFraming),
    Ordering::Less | Ordering::Equal => Ok(Layout {
      count: 2,
      sizes: vec![first],
      last: cursor.remaining.wrapping_sub(first),
      spacing: Spacing::Varying,
      padding: 0,
    }),
  }
}

fn padding_of(cursor: &mut Cursor<'_>) -> Result<u32, Error> {
  let mut padding = 0u32;

  for _byte in 0..cursor.remaining.max(0) {
    let byte = cursor.byte()?;
    let step = i32::from(byte).min(254);
    let Ok(counted) = fitted::<_, u32>(step);

    cursor.remaining = cursor.remaining.wrapping_sub(step);
    padding = padding.wrapping_add(counted);

    match byte {
      255 => {}
      _last => return Ok(padding),
    }
  }

  Err(Error::MalformedFraming)
}

fn counted(cursor: &mut Cursor<'_>, samples: u32, framing: Framing) -> Result<Layout, Error> {
  let header = cursor.byte()?;
  let count = u32::from(header & 0x3F);

  match (
    count,
    samples.wrapping_mul(count).cmp(&LONGEST_PACKET_SAMPLES),
  ) {
    (0, _) | (_, Ordering::Greater) => return Err(Error::MalformedFraming),
    (1.., Ordering::Less | Ordering::Equal) => {}
  }

  let padding = match header & 0x40 {
    0 => 0,
    _padded => padding_of(cursor)?,
  };

  match cursor.remaining.cmp(&0) {
    Ordering::Less => return Err(Error::MalformedFraming),
    Ordering::Equal | Ordering::Greater => {}
  }

  match header & 0x80 {
    0 => constant_frames(
      cursor,
      crate::quantities::FrameCount(count),
      crate::quantities::Padding(padding),
      framing,
    ),
    _varying => varying_frames(
      cursor,
      crate::quantities::FrameCount(count),
      crate::quantities::Padding(padding),
    ),
  }
}

fn varying_frames(
  cursor: &mut Cursor<'_>,
  crate::quantities::FrameCount(count): crate::quantities::FrameCount,
  crate::quantities::Padding(padding): crate::quantities::Padding,
) -> Result<Layout, Error> {
  let mut last = cursor.remaining;
  let mut sizes = Vec::new();

  for _frame in 1..count {
    let before = cursor.remaining;
    let size = cursor.size()?;

    match size.cmp(&cursor.remaining) {
      Ordering::Greater => return Err(Error::MalformedFraming),
      Ordering::Less | Ordering::Equal => {}
    }

    sizes.push(size);
    last = last.wrapping_sub(before.wrapping_sub(cursor.remaining).wrapping_add(size));
  }

  match last.cmp(&0) {
    Ordering::Less => Err(Error::MalformedFraming),
    Ordering::Equal | Ordering::Greater => Ok(Layout {
      count,
      sizes,
      last,
      spacing: Spacing::Varying,
      padding,
    }),
  }
}

fn copies(count: u32, size: i32) -> Result<Vec<i32>, Never> {
  Ok((1..count).map(|_frame| size).collect())
}

fn constant_frames(
  cursor: &Cursor<'_>,
  crate::quantities::FrameCount(count): crate::quantities::FrameCount,
  crate::quantities::Padding(padding): crate::quantities::Padding,
  framing: Framing,
) -> Result<Layout, Error> {
  let Ok(many) = fitted::<_, i32>(count);
  let mut layout = Layout {
    count,
    sizes: Vec::new(),
    last: cursor.remaining,
    spacing: Spacing::Constant,
    padding,
  };

  match framing {
    Framing::SelfDelimited => Ok(layout),
    Framing::Regular => {
      layout.last = match cursor.remaining.checked_div(many) {
        Some(each) => each,
        None => 0,
      };

      match layout.last.wrapping_mul(many).cmp(&cursor.remaining) {
        Ordering::Less | Ordering::Greater => Err(Error::MalformedFraming),
        Ordering::Equal => {
          let Ok(sizes) = copies(count, layout.last);

          layout.sizes = sizes;

          Ok(layout)
        }
      }
    }
  }
}

fn delimited(cursor: &mut Cursor<'_>, layout: &mut Layout) -> Result<(), Error> {
  let before = cursor.remaining;
  let size = cursor.size()?;

  match size.cmp(&cursor.remaining) {
    Ordering::Greater => return Err(Error::MalformedFraming),
    Ordering::Less | Ordering::Equal => {}
  }

  match layout.spacing {
    Spacing::Constant => {
      let Ok(many) = fitted::<_, i32>(layout.count);

      match size.wrapping_mul(many).cmp(&cursor.remaining) {
        Ordering::Greater => return Err(Error::MalformedFraming),
        Ordering::Less | Ordering::Equal => {
          let Ok(sizes) = copies(layout.count, size);

          layout.sizes = sizes;
        }
      }
    }
    Spacing::Varying => {
      match before
        .wrapping_sub(cursor.remaining)
        .wrapping_add(size)
        .cmp(&layout.last)
      {
        Ordering::Greater => return Err(Error::MalformedFraming),
        Ordering::Less | Ordering::Equal => {}
      }
    }
  }

  layout.last = size;

  Ok(())
}

fn framed(cursor: &mut Cursor<'_>, toc: Toc, framing: Framing) -> Result<Layout, Error> {
  let Ok(samples) = toc.frame_samples();

  match toc.code {
    0 => Ok(Layout {
      count: 1,
      sizes: Vec::new(),
      last: cursor.remaining,
      spacing: Spacing::Varying,
      padding: 0,
    }),
    1 => equal_pair(cursor, framing),
    2 => unequal_pair(cursor),
    _counted => counted(cursor, samples, framing),
  }
}

fn cut<'a>(data: &'a [u8], sizes: &[i32], from: u32) -> Result<(Vec<&'a [u8]>, u32), Error> {
  let mut frames = Vec::new();
  let mut at = from;

  for size in sizes {
    let Ok(size_2) = fitted::<_, u32>(*size);
    let Ok(frame) = stretch(
      data,
      crate::quantities::StartOffset(at),
      crate::quantities::ByteLength(size_2),
    );
    let Ok(held) = fitted::<_, u32>(frame.len());

    match held.cmp(&size_2) {
      Ordering::Less | Ordering::Greater => return Err(Error::MalformedFraming),
      Ordering::Equal => frames.push(frame),
    }

    at = at.wrapping_add(size_2);
  }

  Ok((frames, at))
}

fn parse_with<'a>(data: &'a [u8], framing: Framing) -> Result<(Packet<'a>, u32), Error> {
  let first = match data.first() {
    Some(first) => *first,
    None => return Err(Error::EmptyPacket),
  };
  let Ok(toc) = Toc::parse(first);
  let Ok(length) = fitted::<_, i32>(data.len());
  let mut cursor = Cursor {
    data,
    at: 1,
    remaining: length.wrapping_sub(1),
  };
  let mut layout = framed(&mut cursor, toc, framing)?;

  match framing {
    Framing::SelfDelimited => delimited(&mut cursor, &mut layout)?,
    Framing::Regular => {}
  }

  match layout
    .sizes
    .iter()
    .chain(std::iter::once(&layout.last))
    .find(|&&size| size > LONGEST_FRAME)
  {
    Some(_oversized) => return Err(Error::MalformedFraming),
    None => layout.sizes.push(layout.last),
  }

  let (frames, at) = cut(data, &layout.sizes, cursor.at)?;
  let Ok(held) = fitted::<_, u32>(data.len());

  let total = match framing {
    Framing::SelfDelimited => at.wrapping_add(layout.padding),
    Framing::Regular => held,
  };

  match total.cmp(&held) {
    Ordering::Greater => Err(Error::MalformedFraming),
    Ordering::Less | Ordering::Equal => Ok((Packet { toc, frames }, total)),
  }
}

impl<'a> Packet<'a> {
  pub(crate) fn parse(data: &'a [u8]) -> Result<Packet<'a>, Error> {
    let (packet, _used) = parse_with(data, Framing::Regular)?;

    Ok(packet)
  }

  pub(crate) fn parse_self_delimited(data: &'a [u8]) -> Result<(Packet<'a>, u32), Error> {
    parse_with(data, Framing::SelfDelimited)
  }
}

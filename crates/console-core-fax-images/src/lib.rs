//! A picture sent the way a fax sends one, decoded into its black and white
//! pixels.
//!
//! ITU-T T.4 and T.6 code a picture a line at a time. A one-dimensional line
//! is the length of each run of one colour, white first, each length a code
//! from a table for its colour. A two-dimensional line is said against the
//! line above it: where each change of colour falls relative to the one above,
//! or two runs where nothing above is near enough. Group 3 is either kind of
//! line, a tag bit before each saying which when both are allowed, and Group
//! 4 is every line two-dimensional against an imaginary white one above the
//! first. PDF's CCITTFax filter and JBIG2's MMR regions are both written in
//! them, and both hand [`decode`] a [`Decoder`] that is given every pixel as it
//! is made, in runs of eight wherever a run starts on a byte. What a pixel is
//! told is its colour; which bit a colour is stored as is the caller's, because
//! PDF says it in BlackIs1 and JBIG2 says black is one, and neither is a fax
//! picture's to know.
//!
//! The crate began as hayro-ccitt, by Laurenz Stampfl under MIT or
//! Apache-2.0, carried in `forks/` for the PDF reader and the JBIG2 decoder,
//! and it came into the tree under the rules. Every walk now says where it
//! stops: a run code at the bits there are, a line at its width or the bits
//! there are, and a picture at its rows, its end-of-block marker or its last
//! bit, where the fork's loops ended only when the stream said so, and went
//! round for ever on a picture no columns wide. A pixel is a [`Color`] rather
//! than a bool, the code tables are built when a [`DecoderContext`] is made
//! rather than by a const fn walking them a bit at a time, and a branch of a
//! code tree that leads nowhere says so rather than being a number no branch
//! can reach. The end-of-line flag and the K parameter, which the fork kept
//! and never read, were left behind: an end of line is always looked for,
//! because PDF writers set the flag carelessly, and only the sign of K says
//! anything a decoder needs. The fork's flag for inverting black went to the
//! callers, where it had handed a decoder white for every black pixel, and
//! the two flags left are each named for the two things they can say.

mod codes;
mod lines;
mod reading;

pub use crate::lines::DecoderContext;

use console_core_never::Never;
use std::fmt;

use crate::codes::Mode;
use crate::reading::BitReader;

const END_OF_BLOCK: u32 = 0x1001;

const END_OF_BLOCK_LENGTH: u32 = 24;

const RETURN_TO_CONTROL: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
  UnexpectedEof,
  InvalidCode,
  LineLengthMismatch,
  Overflow,
}

impl fmt::Display for DecodeError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      DecodeError::UnexpectedEof => write!(to, "the picture ended before the line it was in"),
      DecodeError::InvalidCode => {
        write!(to, "these bits are no code a fax picture is written in")
      }
      DecodeError::LineLengthMismatch => {
        write!(to, "a line came out a different width from the picture")
      }
      DecodeError::Overflow => write!(to, "a change of colour fell outside any line"),
    }
  }
}

impl std::error::Error for DecodeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingMode {
  Group4,
  Group3OneDimensional,
  Group3TwoDimensional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndOfBlock {
  Expected,
  NotExpected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAlignment {
  Bit,
  Byte,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeSettings {
  pub columns: u32,
  pub rows: u32,
  pub end_of_block: EndOfBlock,
  pub row_alignment: RowAlignment,
  pub encoding: EncodingMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
  White,
  Black,
}

impl Color {
  fn opposite(self) -> Result<Color, Never> {
    Ok(match self {
      Color::White => Color::Black,
      Color::Black => Color::White,
    })
  }
}

pub trait Decoder {
  fn push_pixel(&mut self, color: Color) -> Result<(), Never>;

  fn push_pixel_chunk(&mut self, color: Color, chunk_count: u32) -> Result<(), Never>;

  fn next_line(&mut self) -> Result<(), Never>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lines {
  OneDimensional,
  Tagged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
  Continues,
  Ended,
}

pub fn decode(
  data: &[u8],
  decoder: &mut impl Decoder,
  context: &mut DecoderContext,
) -> Result<u64, DecodeError> {
  let Ok(()) = context.reset();
  let Ok(mut reader) = BitReader::new(data);

  let decoded = match context.settings.encoding {
    EncodingMode::Group4 => group4(context, &mut reader, decoder),
    EncodingMode::Group3OneDimensional => {
      group3(context, &mut reader, decoder, Lines::OneDimensional)
    }
    EncodingMode::Group3TwoDimensional => group3(context, &mut reader, decoder, Lines::Tagged),
  };

  decoded?;

  let Ok(()) = reader.align();
  let Ok(read) = reader.byte_pos();

  Ok(read)
}

fn most_lines(context: &DecoderContext, reader: &BitReader<'_>) -> Result<u64, Never> {
  let Ok(left) = reader.bits_left();

  Ok(left.saturating_add(1).max(u64::from(context.settings.rows)))
}

fn group3(
  context: &mut DecoderContext,
  reader: &mut BitReader<'_>,
  decoder: &mut impl Decoder,
  lines: Lines,
) -> Result<(), DecodeError> {
  let Ok(_leading) = reader.read_eols();
  let Ok(most) = most_lines(context, reader);

  for _line in 0..most {
    let line = match lines {
      Lines::Tagged => tagged_line(context, reader, decoder),
      Lines::OneDimensional => one_dimensional_line(context, reader, decoder),
    };

    line?;
    context.next_line(reader, decoder)?;

    let Ok(block) = group3_block(context, reader);

    match block {
      Block::Ended => return Ok(()),
      Block::Continues => {}
    }
  }

  Ok(())
}

fn tagged_line(
  context: &mut DecoderContext,
  reader: &mut BitReader<'_>,
  decoder: &mut impl Decoder,
) -> Result<(), DecodeError> {
  let tag = reader.read_bit()?;

  match ::core::primitive::bool::then_some(tag == 1, ()) {
    ::core::option::Option::Some(()) => one_dimensional_line(context, reader, decoder),
    ::core::option::Option::None => two_dimensional_line(context, reader, decoder),
  }
}

fn group3_block(context: &DecoderContext, reader: &mut BitReader<'_>) -> Result<Block, Never> {
  let Ok(ends) = reader.read_eols();
  let Ok(left) = reader.bits_left();
  let returned =
    context.settings.end_of_block == EndOfBlock::Expected && ends >= RETURN_TO_CONTROL;
  let counted = context.decoded_rows == context.settings.rows;

  Ok(
    match ::core::primitive::bool::then_some(returned || counted || left == 0, ()) {
      ::core::option::Option::Some(()) => Block::Ended,
      ::core::option::Option::None => Block::Continues,
    },
  )
}

fn group4(
  context: &mut DecoderContext,
  reader: &mut BitReader<'_>,
  decoder: &mut impl Decoder,
) -> Result<(), DecodeError> {
  let Ok(lines) = most_lines(context, reader);

  for _line in 0..lines {
    let ahead = reader.peek_bits(END_OF_BLOCK_LENGTH);

    match ::core::primitive::bool::then_some(
      context.settings.end_of_block == EndOfBlock::Expected && ahead == Ok(END_OF_BLOCK),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let _marker = reader.read_bits(END_OF_BLOCK_LENGTH)?;

        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(left) = reader.bits_left();

    match ::core::primitive::bool::then_some(
      context.decoded_rows == context.settings.rows || left == 0,
      (),
    ) {
      ::core::option::Option::Some(()) => return Ok(()),
      ::core::option::Option::None => {}
    }

    two_dimensional_line(context, reader, decoder)?;
    context.next_line(reader, decoder)?;
  }

  Ok(())
}

fn one_dimensional_line(
  context: &mut DecoderContext,
  reader: &mut BitReader<'_>,
  decoder: &mut impl Decoder,
) -> Result<(), DecodeError> {
  let Ok(bound) = reader.bits_left();

  for _run in 0..=bound {
    match context.at_end_of_line() {
      Ok(Block::Ended) => return Ok(()),
      Ok(Block::Continues) => {}
    }

    let run = context.codes.run(reader, context.color)?;
    let Ok(()) = context.push_pixels(decoder, run);
    let Ok(()) = context.turn();
  }

  Err(DecodeError::UnexpectedEof)
}

fn two_dimensional_line(
  context: &mut DecoderContext,
  reader: &mut BitReader<'_>,
  decoder: &mut impl Decoder,
) -> Result<(), DecodeError> {
  let Ok(bound) = reader.bits_left();

  for _mode in 0..=bound {
    match context.at_end_of_line() {
      Ok(Block::Ended) => return Ok(()),
      Ok(Block::Continues) => {}
    }

    let mode = context.codes.mode(reader)?;

    match mode {
      Mode::Pass => {
        let Ok(()) = context.pass(decoder);
      }
      Mode::Vertical(offset) => {
        context.vertical(decoder, offset)?;
      }
      Mode::Horizontal => {
        let first = context.codes.run(reader, context.color)?;
        let Ok(()) = context.push_pixels(decoder, first);
        let Ok(()) = context.turn();
        let second = context.codes.run(reader, context.color)?;
        let Ok(()) = context.push_pixels(decoder, second);
        let Ok(()) = context.turn();
        let Ok(()) = context.update_b();
      }
    }
  }

  Err(DecodeError::UnexpectedEof)
}

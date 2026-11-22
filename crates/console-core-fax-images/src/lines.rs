//! The line being decoded and the line above it, kept as where each changes
//! colour.
//!
//! T.6 names the places a two-dimensional code is read against: a0, where
//! the line being decoded has got to; b1, the first change above and to the
//! right of a0 that turns to the colour a0 is not; and b2, the change after
//! b1. The changes above are walked from wherever the last b1 was found, so a
//! line is one walk over the line above it rather than one per code. Where
//! nothing above is far enough right, b1 and b2 are the end of the line.
//!
//! A run is cut at the end of its line, which is where a picture that says
//! more than its width is made to fit. A pass that would go back past a0,
//! which only a damaged picture asks for, takes the rest of the line: the fork
//! subtracted and wrapped to the same answer, and this says it.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::codes::Codes;
use crate::reading::BitReader;
use crate::{Block, Color, DecodeError, DecodeSettings, Decoder, RowAlignment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Change {
  at: u32,
  color: Color,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderContext {
  pub(crate) codes: Codes,
  above: Vec<Change>,
  walked_from: u32,
  b1: Option<u32>,
  here: Vec<Change>,
  decoded: u32,
  pub(crate) color: Color,
  pub(crate) decoded_rows: u32,
  pub(crate) settings: DecodeSettings,
}

impl DecoderContext {
  pub fn new(settings: DecodeSettings) -> Result<DecoderContext, Never> {
    let Ok(codes) = Codes::new();

    Ok(DecoderContext {
      codes,
      above: Vec::new(),
      walked_from: 0,
      b1: Some(0),
      here: Vec::new(),
      decoded: 0,
      color: Color::White,
      decoded_rows: 0,
      settings,
    })
  }

  pub(crate) fn reset(&mut self) -> Result<(), Never> {
    self.above.clear();
    self.walked_from = 0;
    self.b1 = Some(0);
    self.here.clear();
    self.decoded = 0;
    self.color = Color::White;
    self.decoded_rows = 0;

    Ok(())
  }

  pub(crate) fn turn(&mut self) -> Result<(), Never> {
    let Ok(opposite) = self.color.opposite();

    self.color = opposite;

    Ok(())
  }

  pub(crate) fn at_end_of_line(&self) -> Result<Block, Never> {
    Ok(match ::core::primitive::bool::then_some(self.decoded == self.settings.columns, ()) {
      ::core::option::Option::Some(()) => Block::Ended,
      ::core::option::Option::None => Block::Continues,
    })
  }

  pub(crate) fn pass(&mut self, decoder: &mut impl Decoder) -> Result<(), Never> {
    let Ok(b2) = self.b2();

    let run = match b2.checked_sub(self.decoded) {
      Some(run) => run,
      None => self.settings.columns.saturating_sub(self.decoded),
    };

    let Ok(()) = self.push_pixels(decoder, run);

    self.update_b()
  }

  pub(crate) fn vertical(
    &mut self,
    decoder: &mut impl Decoder,
    offset: i8,
  ) -> Result<(), DecodeError> {
    let Ok(b1) = self.b1();

    let a1 = match b1.checked_add_signed(i32::from(offset)) {
      Some(a1) => a1,
      None => return Err(DecodeError::Overflow),
    };

    let run = match a1.checked_sub(self.decoded) {
      Some(run) => run,
      None => return Err(DecodeError::Overflow),
    };

    let Ok(()) = self.push_pixels(decoder, run);
    let Ok(()) = self.turn();
    let Ok(()) = self.update_b();

    Ok(())
  }

  fn b1(&self) -> Result<u32, Never> {
    self.change_above(self.b1)
  }

  fn b2(&self) -> Result<u32, Never> {
    self.change_above(self.b1.map(|b1| b1.saturating_add(1)))
  }

  fn change_above(&self, which: Option<u32>) -> Result<u32, Never> {
    let found = match which {
      Some(place) => {
        let Ok(slot) = index(place);

        self.above.get(slot)
      }
      None => None,
    };

    Ok(match found {
      Some(change) => change.at,
      None => self.settings.columns,
    })
  }

  pub(crate) fn update_b(&mut self) -> Result<(), Never> {
    let Ok(wanted) = self.color.opposite();
    let Ok(from) = index(self.walked_from);

    let least = match self.decoded {
      0 => 0,
      decoded => decoded.saturating_add(1),
    };

    self.b1 = None;

    for (at, change) in self.above.iter().enumerate().skip(from) {
      let Ok(place) = fitted::<_, u32>(at);

      match ::core::primitive::bool::then_some(change.at < least, ()) {
        ::core::option::Option::Some(()) => self.walked_from = place.saturating_add(1),
        ::core::option::Option::None => match ::core::primitive::bool::then_some(change.color == wanted, ()) {
          ::core::option::Option::Some(()) => {
            self.b1 = Some(place);

            return Ok(());
          }
          ::core::option::Option::None => {}
        },
      }
    }

    Ok(())
  }

  pub(crate) fn push_pixels(
    &mut self,
    decoder: &mut impl Decoder,
    wanted: u32,
  ) -> Result<(), Never> {
    let count = wanted.min(self.settings.columns.saturating_sub(self.decoded));
    let color = self.color;
    let unaligned = count.min(self.decoded.wrapping_neg() & 7);
    let aligned = count.saturating_sub(unaligned);

    for _pixel in 0..unaligned {
      let Ok(()) = decoder.push_pixel(color);
    }

    match aligned.wrapping_shr(3) {
      0 => {}
      chunks => {
        let Ok(()) = decoder.push_pixel_chunk(color, chunks);
      }
    }

    for _pixel in 0..aligned & 7 {
      let Ok(()) = decoder.push_pixel(color);
    }

    match count {
      0 => Ok(()),
      _pushed => self.changed(count),
    }
  }

  fn changed(&mut self, count: u32) -> Result<(), Never> {
    let turned = match self.here.last() {
      Some(last) => last.color != self.color,
      None => self.color == Color::Black,
    };

    match ::core::primitive::bool::then_some(turned, ()) {
      ::core::option::Option::Some(()) => self.here.push(Change {
        at: self.decoded,
        color: self.color,
      }),
      ::core::option::Option::None => {}
    }

    self.decoded = self.decoded.saturating_add(count);

    Ok(())
  }

  pub(crate) fn next_line(
    &mut self,
    reader: &mut BitReader<'_>,
    decoder: &mut impl Decoder,
  ) -> Result<(), DecodeError> {
    match ::core::primitive::bool::then_some(self.decoded == self.settings.columns, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(DecodeError::LineLengthMismatch),
    }

    std::mem::swap(&mut self.above, &mut self.here);
    self.here.clear();
    self.decoded = 0;
    self.walked_from = 0;
    self.b1 = Some(0);
    self.color = Color::White;
    self.decoded_rows = self.decoded_rows.saturating_add(1);

    let Ok(()) = decoder.next_line();

    match self.settings.row_alignment {
      RowAlignment::Byte => {
        let Ok(()) = reader.align();
      }
      RowAlignment::Bit => {}
    }

    let Ok(()) = self.update_b();

    Ok(())
  }
}

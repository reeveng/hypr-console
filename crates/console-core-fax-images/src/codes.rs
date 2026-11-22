//! The codes a run's length and a line's mode are written in, and the trees
//! they are read through.
//!
//! Every code in T.4 and T.6 is a string of bits no other code begins with,
//! so each table is a tree: a branch for a zero and one for a one at every
//! bit, and a length or a mode at the end of each code. A run is read a bit at
//! a time down the tree for its colour. A length of sixty-four or more is a
//! make-up code, which is added on and followed by more of the run, and a
//! length below that ends it. A mode is read down the tree of modes the same
//! way and is always one code.
//!
//! A run the tree for its colour cannot read is read again down the other
//! colour's tree, from wherever the first attempt stopped, because some PDF
//! writers swap the two. That is what the fork did, and a picture it read is
//! a picture this reads.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::reading::BitReader;
use crate::{Color, DecodeError};

const MAKE_UP_FROM: u16 = 64;

type Code = (u16, u8, u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
  Pass,
  Horizontal,
  Vertical(i8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Branch {
  Nowhere,
  Next(u16),
  Ends(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bit {
  Zero,
  One,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grow {
  End,
  Fresh,
  Follow(u16),
  Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fork {
  zero: Branch,
  one: Branch,
}

impl Fork {
  fn taken(self, bit: Bit) -> Result<Branch, Never> {
    Ok(match bit {
      Bit::Zero => self.zero,
      Bit::One => self.one,
    })
  }
}

const BARE: Fork = Fork {
  zero: Branch::Nowhere,
  one: Branch::Nowhere,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Codes {
  white: Vec<Fork>,
  black: Vec<Fork>,
  modes: Vec<Fork>,
}

impl Codes {
  pub(crate) fn new() -> Result<Codes, Never> {
    let Ok(white) = tree(&[&WHITE_TERMINATING, &WHITE_MAKEUP, &COMMON_MAKEUP]);
    let Ok(black) = tree(&[&BLACK_TERMINATING, &BLACK_MAKEUP, &COMMON_MAKEUP]);
    let Ok(modes) = tree(&[&MODE_CODES]);

    Ok(Codes {
      white,
      black,
      modes,
    })
  }

  pub(crate) fn run(&self, reader: &mut BitReader<'_>, color: Color) -> Result<u32, DecodeError> {
    let (first, other) = match color {
      Color::White => (&self.white, &self.black),
      Color::Black => (&self.black, &self.white),
    };

    match read_down(first, reader) {
      Ok(run) => Ok(run),
      Err(_the_other_color) => read_down(other, reader),
    }
  }

  pub(crate) fn mode(&self, reader: &mut BitReader<'_>) -> Result<Mode, DecodeError> {
    let read = read_down(&self.modes, reader)?;

    match read {
      0 => Ok(Mode::Pass),
      1 => Ok(Mode::Horizontal),
      2 => Ok(Mode::Vertical(0)),
      3 => Ok(Mode::Vertical(1)),
      4 => Ok(Mode::Vertical(2)),
      5 => Ok(Mode::Vertical(3)),
      6 => Ok(Mode::Vertical(-1)),
      7 => Ok(Mode::Vertical(-2)),
      8 => Ok(Mode::Vertical(-3)),
      _unknown => Err(DecodeError::InvalidCode),
    }
  }
}

fn read_down(forks: &[Fork], reader: &mut BitReader<'_>) -> Result<u32, DecodeError> {
  let Ok(left) = reader.bits_left();
  let mut total = 0u32;
  let mut at = 0u16;

  for _bit in 0..=left {
    let read = reader.read_bit()?;
    let Ok(bit) = bit_of(read);
    let Ok(slot) = index(at);

    let taken = match forks.get(slot) {
      Some(fork) => fork.taken(bit),
      None => return Err(DecodeError::InvalidCode),
    };

    match taken {
      Ok(Branch::Nowhere) => return Err(DecodeError::InvalidCode),
      Ok(Branch::Next(next)) => at = next,
      Ok(Branch::Ends(length)) => {
        total = match total.checked_add(u32::from(length)) {
          Some(longer) => longer,
          None => return Err(DecodeError::Overflow),
        };

        match ::core::primitive::bool::then_some(length < MAKE_UP_FROM, ()) {
          ::core::option::Option::Some(()) => return Ok(total),
          ::core::option::Option::None => at = 0,
        }
      }
    }
  }

  Err(DecodeError::UnexpectedEof)
}

fn tree(tables: &[&[Code]]) -> Result<Vec<Fork>, Never> {
  let mut forks = vec![BARE];

  for code in tables.iter().flat_map(|table| table.iter()) {
    let Ok(()) = grown(&mut forks, *code);
  }

  Ok(forks)
}

fn grown(forks: &mut Vec<Fork>, (length, width, bits): Code) -> Result<(), Never> {
  let mut at = 0u16;

  for place in (0..width).rev() {
    let Ok(bit) = bit_of(u32::from(bits.wrapping_shr(u32::from(place)) & 1));
    let Ok(fresh) = fitted::<_, u16>(forks.len());
    let Ok(slot) = index(at);

    let taken = match forks.get(slot) {
      Some(fork) => fork.taken(bit),
      None => return Ok(()),
    };

    let grow = match place {
      0 => Grow::End,
      _inner => match taken {
        Ok(Branch::Nowhere) => Grow::Fresh,
        Ok(Branch::Next(next)) => Grow::Follow(next),
        Ok(Branch::Ends(_shorter)) => Grow::Stop,
      },
    };

    match grow {
      Grow::End => {
        let Ok(()) = branched(forks, at, bit, Branch::Ends(length));

        return Ok(());
      }
      Grow::Fresh => {
        let Ok(()) = branched(forks, at, bit, Branch::Next(fresh));

        forks.push(BARE);
        at = fresh;
      }
      Grow::Follow(next) => at = next,
      Grow::Stop => return Ok(()),
    }
  }

  Ok(())
}

fn branched(forks: &mut [Fork], at: u16, bit: Bit, branch: Branch) -> Result<(), Never> {
  let Ok(slot) = index(at);

  match forks.get_mut(slot) {
    Some(fork) => match bit {
      Bit::Zero => fork.zero = branch,
      Bit::One => fork.one = branch,
    },
    None => {}
  }

  Ok(())
}

fn bit_of(read: u32) -> Result<Bit, Never> {
  Ok(match read {
    0 => Bit::Zero,
    _set => Bit::One,
  })
}

const WHITE_TERMINATING: [Code; 64] = [
  (0, 8, 0b00110101),
  (1, 6, 0b000111),
  (2, 4, 0b0111),
  (3, 4, 0b1000),
  (4, 4, 0b1011),
  (5, 4, 0b1100),
  (6, 4, 0b1110),
  (7, 4, 0b1111),
  (8, 5, 0b10011),
  (9, 5, 0b10100),
  (10, 5, 0b00111),
  (11, 5, 0b01000),
  (12, 6, 0b001000),
  (13, 6, 0b000011),
  (14, 6, 0b110100),
  (15, 6, 0b110101),
  (16, 6, 0b101010),
  (17, 6, 0b101011),
  (18, 7, 0b0100111),
  (19, 7, 0b0001100),
  (20, 7, 0b0001000),
  (21, 7, 0b0010111),
  (22, 7, 0b0000011),
  (23, 7, 0b0000100),
  (24, 7, 0b0101000),
  (25, 7, 0b0101011),
  (26, 7, 0b0010011),
  (27, 7, 0b0100100),
  (28, 7, 0b0011000),
  (29, 8, 0b00000010),
  (30, 8, 0b00000011),
  (31, 8, 0b00011010),
  (32, 8, 0b00011011),
  (33, 8, 0b00010010),
  (34, 8, 0b00010011),
  (35, 8, 0b00010100),
  (36, 8, 0b00010101),
  (37, 8, 0b00010110),
  (38, 8, 0b00010111),
  (39, 8, 0b00101000),
  (40, 8, 0b00101001),
  (41, 8, 0b00101010),
  (42, 8, 0b00101011),
  (43, 8, 0b00101100),
  (44, 8, 0b00101101),
  (45, 8, 0b00000100),
  (46, 8, 0b00000101),
  (47, 8, 0b00001010),
  (48, 8, 0b00001011),
  (49, 8, 0b01010010),
  (50, 8, 0b01010011),
  (51, 8, 0b01010100),
  (52, 8, 0b01010101),
  (53, 8, 0b00100100),
  (54, 8, 0b00100101),
  (55, 8, 0b01011000),
  (56, 8, 0b01011001),
  (57, 8, 0b01011010),
  (58, 8, 0b01011011),
  (59, 8, 0b01001010),
  (60, 8, 0b01001011),
  (61, 8, 0b00110010),
  (62, 8, 0b00110011),
  (63, 8, 0b00110100),
];

const WHITE_MAKEUP: [Code; 27] = [
  (64, 5, 0b11011),
  (128, 5, 0b10010),
  (192, 6, 0b010111),
  (256, 7, 0b0110111),
  (320, 8, 0b00110110),
  (384, 8, 0b00110111),
  (448, 8, 0b01100100),
  (512, 8, 0b01100101),
  (576, 8, 0b01101000),
  (640, 8, 0b01100111),
  (704, 9, 0b011001100),
  (768, 9, 0b011001101),
  (832, 9, 0b011010010),
  (896, 9, 0b011010011),
  (960, 9, 0b011010100),
  (1024, 9, 0b011010101),
  (1088, 9, 0b011010110),
  (1152, 9, 0b011010111),
  (1216, 9, 0b011011000),
  (1280, 9, 0b011011001),
  (1344, 9, 0b011011010),
  (1408, 9, 0b011011011),
  (1472, 9, 0b010011000),
  (1536, 9, 0b010011001),
  (1600, 9, 0b010011010),
  (1664, 6, 0b011000),
  (1728, 9, 0b010011011),
];

const BLACK_TERMINATING: [Code; 64] = [
  (0, 10, 0b0000110111),
  (1, 3, 0b010),
  (2, 2, 0b11),
  (3, 2, 0b10),
  (4, 3, 0b011),
  (5, 4, 0b0011),
  (6, 4, 0b0010),
  (7, 5, 0b00011),
  (8, 6, 0b000101),
  (9, 6, 0b000100),
  (10, 7, 0b0000100),
  (11, 7, 0b0000101),
  (12, 7, 0b0000111),
  (13, 8, 0b00000100),
  (14, 8, 0b00000111),
  (15, 9, 0b000011000),
  (16, 10, 0b0000010111),
  (17, 10, 0b0000011000),
  (18, 10, 0b0000001000),
  (19, 11, 0b00001100111),
  (20, 11, 0b00001101000),
  (21, 11, 0b00001101100),
  (22, 11, 0b00000110111),
  (23, 11, 0b00000101000),
  (24, 11, 0b00000010111),
  (25, 11, 0b00000011000),
  (26, 12, 0b000011001010),
  (27, 12, 0b000011001011),
  (28, 12, 0b000011001100),
  (29, 12, 0b000011001101),
  (30, 12, 0b000001101000),
  (31, 12, 0b000001101001),
  (32, 12, 0b000001101010),
  (33, 12, 0b000001101011),
  (34, 12, 0b000011010010),
  (35, 12, 0b000011010011),
  (36, 12, 0b000011010100),
  (37, 12, 0b000011010101),
  (38, 12, 0b000011010110),
  (39, 12, 0b000011010111),
  (40, 12, 0b000001101100),
  (41, 12, 0b000001101101),
  (42, 12, 0b000011011010),
  (43, 12, 0b000011011011),
  (44, 12, 0b000001010100),
  (45, 12, 0b000001010101),
  (46, 12, 0b000001010110),
  (47, 12, 0b000001010111),
  (48, 12, 0b000001100100),
  (49, 12, 0b000001100101),
  (50, 12, 0b000001010010),
  (51, 12, 0b000001010011),
  (52, 12, 0b000000100100),
  (53, 12, 0b000000110111),
  (54, 12, 0b000000111000),
  (55, 12, 0b000000100111),
  (56, 12, 0b000000101000),
  (57, 12, 0b000001011000),
  (58, 12, 0b000001011001),
  (59, 12, 0b000000101011),
  (60, 12, 0b000000101100),
  (61, 12, 0b000001011010),
  (62, 12, 0b000001100110),
  (63, 12, 0b000001100111),
];

const BLACK_MAKEUP: [Code; 27] = [
  (64, 10, 0b0000001111),
  (128, 12, 0b000011001000),
  (192, 12, 0b000011001001),
  (256, 12, 0b000001011011),
  (320, 12, 0b000000110011),
  (384, 12, 0b000000110100),
  (448, 12, 0b000000110101),
  (512, 13, 0b0000001101100),
  (576, 13, 0b0000001101101),
  (640, 13, 0b0000001001010),
  (704, 13, 0b0000001001011),
  (768, 13, 0b0000001001100),
  (832, 13, 0b0000001001101),
  (896, 13, 0b0000001110010),
  (960, 13, 0b0000001110011),
  (1024, 13, 0b0000001110100),
  (1088, 13, 0b0000001110101),
  (1152, 13, 0b0000001110110),
  (1216, 13, 0b0000001110111),
  (1280, 13, 0b0000001010010),
  (1344, 13, 0b0000001010011),
  (1408, 13, 0b0000001010100),
  (1472, 13, 0b0000001010101),
  (1536, 13, 0b0000001011010),
  (1600, 13, 0b0000001011011),
  (1664, 13, 0b0000001100100),
  (1728, 13, 0b0000001100101),
];

const COMMON_MAKEUP: [Code; 13] = [
  (1792, 11, 0b00000001000),
  (1856, 11, 0b00000001100),
  (1920, 11, 0b00000001101),
  (1984, 12, 0b000000010010),
  (2048, 12, 0b000000010011),
  (2112, 12, 0b000000010100),
  (2176, 12, 0b000000010101),
  (2240, 12, 0b000000010110),
  (2304, 12, 0b000000010111),
  (2368, 12, 0b000000011100),
  (2432, 12, 0b000000011101),
  (2496, 12, 0b000000011110),
  (2560, 12, 0b000000011111),
];

const MODE_CODES: [Code; 9] = [
  (0, 4, 0b0001),
  (1, 3, 0b001),
  (2, 1, 0b1),
  (3, 3, 0b011),
  (4, 6, 0b000011),
  (5, 7, 0b0000011),
  (6, 3, 0b010),
  (7, 6, 0b000010),
  (8, 7, 0b0000010),
];

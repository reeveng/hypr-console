//! The fifteen standard Huffman tables of T.88 Annex B, each line in the order its code is assigned.

use console_core_never::Never;

use crate::huffman::{Kind, Line};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Standard {
  B1,
  B2,
  B3,
  B4,
  B5,
  B6,
  B7,
  B8,
  B9,
  B10,
  B11,
  B12,
  B13,
  B14,
  B15,
}

impl Standard {
  pub(crate) fn lines(self) -> Result<&'static [Line], Never> {
    Ok(match self {
      Standard::B1 => &B1,
      Standard::B2 => &B2,
      Standard::B3 => &B3,
      Standard::B4 => &B4,
      Standard::B5 => &B5,
      Standard::B6 => &B6,
      Standard::B7 => &B7,
      Standard::B8 => &B8,
      Standard::B9 => &B9,
      Standard::B10 => &B10,
      Standard::B11 => &B11,
      Standard::B12 => &B12,
      Standard::B13 => &B13,
      Standard::B14 => &B14,
      Standard::B15 => &B15,
    })
  }
}

const B1: [Line; 4] = [
  Line {
    low: 0,
    prefix: 1,
    range: 4,
    kind: Kind::Range,
  },
  Line {
    low: 16,
    prefix: 2,
    range: 8,
    kind: Kind::Range,
  },
  Line {
    low: 272,
    prefix: 3,
    range: 16,
    kind: Kind::Range,
  },
  Line {
    low: 65808,
    prefix: 3,
    range: 32,
    kind: Kind::Range,
  },
];

const B2: [Line; 7] = [
  Line {
    low: 0,
    prefix: 1,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 1,
    prefix: 2,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 2,
    prefix: 3,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 3,
    prefix: 4,
    range: 3,
    kind: Kind::Range,
  },
  Line {
    low: 11,
    prefix: 5,
    range: 6,
    kind: Kind::Range,
  },
  Line {
    low: 75,
    prefix: 6,
    range: 32,
    kind: Kind::Range,
  },
  Line {
    low: 0,
    prefix: 6,
    range: 0,
    kind: Kind::OutOfBand,
  },
];

const B3: [Line; 9] = [
  Line {
    low: 0,
    prefix: 1,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 1,
    prefix: 2,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 2,
    prefix: 3,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 3,
    prefix: 4,
    range: 3,
    kind: Kind::Range,
  },
  Line {
    low: 11,
    prefix: 5,
    range: 6,
    kind: Kind::Range,
  },
  Line {
    low: 0,
    prefix: 6,
    range: 0,
    kind: Kind::OutOfBand,
  },
  Line {
    low: 75,
    prefix: 7,
    range: 32,
    kind: Kind::Range,
  },
  Line {
    low: -256,
    prefix: 8,
    range: 8,
    kind: Kind::Range,
  },
  Line {
    low: -257,
    prefix: 8,
    range: 32,
    kind: Kind::Lower,
  },
];

const B4: [Line; 6] = [
  Line {
    low: 1,
    prefix: 1,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 2,
    prefix: 2,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 3,
    prefix: 3,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 4,
    prefix: 4,
    range: 3,
    kind: Kind::Range,
  },
  Line {
    low: 12,
    prefix: 5,
    range: 6,
    kind: Kind::Range,
  },
  Line {
    low: 76,
    prefix: 5,
    range: 32,
    kind: Kind::Range,
  },
];

const B5: [Line; 8] = [
  Line {
    low: 1,
    prefix: 1,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 2,
    prefix: 2,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 3,
    prefix: 3,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 4,
    prefix: 4,
    range: 3,
    kind: Kind::Range,
  },
  Line {
    low: 12,
    prefix: 5,
    range: 6,
    kind: Kind::Range,
  },
  Line {
    low: 76,
    prefix: 6,
    range: 32,
    kind: Kind::Range,
  },
  Line {
    low: -255,
    prefix: 7,
    range: 8,
    kind: Kind::Range,
  },
  Line {
    low: -256,
    prefix: 7,
    range: 32,
    kind: Kind::Lower,
  },
];

const B6: [Line; 14] = [
  Line {
    low: 0,
    prefix: 2,
    range: 7,
    kind: Kind::Range,
  },
  Line {
    low: 128,
    prefix: 3,
    range: 7,
    kind: Kind::Range,
  },
  Line {
    low: 256,
    prefix: 3,
    range: 8,
    kind: Kind::Range,
  },
  Line {
    low: -1024,
    prefix: 4,
    range: 9,
    kind: Kind::Range,
  },
  Line {
    low: -512,
    prefix: 4,
    range: 8,
    kind: Kind::Range,
  },
  Line {
    low: -256,
    prefix: 4,
    range: 7,
    kind: Kind::Range,
  },
  Line {
    low: -32,
    prefix: 4,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: 512,
    prefix: 4,
    range: 9,
    kind: Kind::Range,
  },
  Line {
    low: 1024,
    prefix: 4,
    range: 10,
    kind: Kind::Range,
  },
  Line {
    low: -2048,
    prefix: 5,
    range: 10,
    kind: Kind::Range,
  },
  Line {
    low: -128,
    prefix: 5,
    range: 6,
    kind: Kind::Range,
  },
  Line {
    low: -64,
    prefix: 5,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: -2049,
    prefix: 6,
    range: 32,
    kind: Kind::Lower,
  },
  Line {
    low: 2048,
    prefix: 6,
    range: 32,
    kind: Kind::Range,
  },
];

const B7: [Line; 15] = [
  Line {
    low: -512,
    prefix: 3,
    range: 8,
    kind: Kind::Range,
  },
  Line {
    low: 256,
    prefix: 3,
    range: 8,
    kind: Kind::Range,
  },
  Line {
    low: 512,
    prefix: 3,
    range: 9,
    kind: Kind::Range,
  },
  Line {
    low: 1024,
    prefix: 3,
    range: 10,
    kind: Kind::Range,
  },
  Line {
    low: -1024,
    prefix: 4,
    range: 9,
    kind: Kind::Range,
  },
  Line {
    low: -256,
    prefix: 4,
    range: 7,
    kind: Kind::Range,
  },
  Line {
    low: -32,
    prefix: 4,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: 0,
    prefix: 4,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: 128,
    prefix: 4,
    range: 7,
    kind: Kind::Range,
  },
  Line {
    low: -128,
    prefix: 5,
    range: 6,
    kind: Kind::Range,
  },
  Line {
    low: -64,
    prefix: 5,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: 32,
    prefix: 5,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: 64,
    prefix: 5,
    range: 6,
    kind: Kind::Range,
  },
  Line {
    low: -1025,
    prefix: 5,
    range: 32,
    kind: Kind::Lower,
  },
  Line {
    low: 2048,
    prefix: 5,
    range: 32,
    kind: Kind::Range,
  },
];

const B8: [Line; 21] = [
  Line {
    low: 0,
    prefix: 2,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 0,
    prefix: 2,
    range: 0,
    kind: Kind::OutOfBand,
  },
  Line {
    low: 4,
    prefix: 3,
    range: 4,
    kind: Kind::Range,
  },
  Line {
    low: -1,
    prefix: 4,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 22,
    prefix: 4,
    range: 4,
    kind: Kind::Range,
  },
  Line {
    low: 38,
    prefix: 4,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: 2,
    prefix: 5,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 70,
    prefix: 5,
    range: 6,
    kind: Kind::Range,
  },
  Line {
    low: 134,
    prefix: 5,
    range: 7,
    kind: Kind::Range,
  },
  Line {
    low: 3,
    prefix: 6,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 20,
    prefix: 6,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 262,
    prefix: 6,
    range: 7,
    kind: Kind::Range,
  },
  Line {
    low: 646,
    prefix: 6,
    range: 10,
    kind: Kind::Range,
  },
  Line {
    low: -2,
    prefix: 7,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 390,
    prefix: 7,
    range: 8,
    kind: Kind::Range,
  },
  Line {
    low: -15,
    prefix: 8,
    range: 3,
    kind: Kind::Range,
  },
  Line {
    low: -5,
    prefix: 8,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: -7,
    prefix: 9,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: -3,
    prefix: 9,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: -16,
    prefix: 9,
    range: 32,
    kind: Kind::Lower,
  },
  Line {
    low: 1670,
    prefix: 9,
    range: 32,
    kind: Kind::Range,
  },
];

const B9: [Line; 22] = [
  Line {
    low: 0,
    prefix: 2,
    range: 0,
    kind: Kind::OutOfBand,
  },
  Line {
    low: -1,
    prefix: 3,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 1,
    prefix: 3,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 7,
    prefix: 3,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: -3,
    prefix: 4,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 43,
    prefix: 4,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: 75,
    prefix: 4,
    range: 6,
    kind: Kind::Range,
  },
  Line {
    low: 3,
    prefix: 5,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 139,
    prefix: 5,
    range: 7,
    kind: Kind::Range,
  },
  Line {
    low: 267,
    prefix: 5,
    range: 8,
    kind: Kind::Range,
  },
  Line {
    low: 5,
    prefix: 6,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 39,
    prefix: 6,
    range: 2,
    kind: Kind::Range,
  },
  Line {
    low: 523,
    prefix: 6,
    range: 8,
    kind: Kind::Range,
  },
  Line {
    low: 1291,
    prefix: 6,
    range: 11,
    kind: Kind::Range,
  },
  Line {
    low: -5,
    prefix: 7,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 779,
    prefix: 7,
    range: 9,
    kind: Kind::Range,
  },
  Line {
    low: -31,
    prefix: 8,
    range: 4,
    kind: Kind::Range,
  },
  Line {
    low: -11,
    prefix: 8,
    range: 2,
    kind: Kind::Range,
  },
  Line {
    low: -15,
    prefix: 9,
    range: 2,
    kind: Kind::Range,
  },
  Line {
    low: -7,
    prefix: 9,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: -32,
    prefix: 9,
    range: 32,
    kind: Kind::Lower,
  },
  Line {
    low: 3339,
    prefix: 9,
    range: 32,
    kind: Kind::Range,
  },
];

const B10: [Line; 21] = [
  Line {
    low: -2,
    prefix: 2,
    range: 2,
    kind: Kind::Range,
  },
  Line {
    low: 6,
    prefix: 2,
    range: 6,
    kind: Kind::Range,
  },
  Line {
    low: 0,
    prefix: 2,
    range: 0,
    kind: Kind::OutOfBand,
  },
  Line {
    low: -3,
    prefix: 5,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 2,
    prefix: 5,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 70,
    prefix: 5,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: 3,
    prefix: 6,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 102,
    prefix: 6,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: 134,
    prefix: 6,
    range: 6,
    kind: Kind::Range,
  },
  Line {
    low: 198,
    prefix: 6,
    range: 7,
    kind: Kind::Range,
  },
  Line {
    low: 326,
    prefix: 6,
    range: 8,
    kind: Kind::Range,
  },
  Line {
    low: 582,
    prefix: 6,
    range: 9,
    kind: Kind::Range,
  },
  Line {
    low: 1094,
    prefix: 6,
    range: 10,
    kind: Kind::Range,
  },
  Line {
    low: -21,
    prefix: 7,
    range: 4,
    kind: Kind::Range,
  },
  Line {
    low: -4,
    prefix: 7,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 4,
    prefix: 7,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 2118,
    prefix: 7,
    range: 11,
    kind: Kind::Range,
  },
  Line {
    low: -5,
    prefix: 8,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 5,
    prefix: 8,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: -22,
    prefix: 8,
    range: 32,
    kind: Kind::Lower,
  },
  Line {
    low: 4166,
    prefix: 8,
    range: 32,
    kind: Kind::Range,
  },
];

const B11: [Line; 13] = [
  Line {
    low: 1,
    prefix: 1,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 2,
    prefix: 2,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 4,
    prefix: 4,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 5,
    prefix: 4,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 7,
    prefix: 5,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 9,
    prefix: 5,
    range: 2,
    kind: Kind::Range,
  },
  Line {
    low: 13,
    prefix: 6,
    range: 2,
    kind: Kind::Range,
  },
  Line {
    low: 17,
    prefix: 7,
    range: 2,
    kind: Kind::Range,
  },
  Line {
    low: 21,
    prefix: 7,
    range: 3,
    kind: Kind::Range,
  },
  Line {
    low: 29,
    prefix: 7,
    range: 4,
    kind: Kind::Range,
  },
  Line {
    low: 45,
    prefix: 7,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: 77,
    prefix: 7,
    range: 6,
    kind: Kind::Range,
  },
  Line {
    low: 141,
    prefix: 7,
    range: 32,
    kind: Kind::Range,
  },
];

const B12: [Line; 13] = [
  Line {
    low: 1,
    prefix: 1,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 2,
    prefix: 2,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 3,
    prefix: 3,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 5,
    prefix: 5,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 6,
    prefix: 5,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 8,
    prefix: 6,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 10,
    prefix: 7,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 11,
    prefix: 7,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 13,
    prefix: 7,
    range: 2,
    kind: Kind::Range,
  },
  Line {
    low: 17,
    prefix: 7,
    range: 3,
    kind: Kind::Range,
  },
  Line {
    low: 25,
    prefix: 7,
    range: 4,
    kind: Kind::Range,
  },
  Line {
    low: 41,
    prefix: 8,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: 73,
    prefix: 8,
    range: 32,
    kind: Kind::Range,
  },
];

const B13: [Line; 13] = [
  Line {
    low: 1,
    prefix: 1,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 2,
    prefix: 3,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 7,
    prefix: 3,
    range: 3,
    kind: Kind::Range,
  },
  Line {
    low: 3,
    prefix: 4,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 5,
    prefix: 4,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 4,
    prefix: 5,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 15,
    prefix: 6,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 17,
    prefix: 6,
    range: 2,
    kind: Kind::Range,
  },
  Line {
    low: 21,
    prefix: 6,
    range: 3,
    kind: Kind::Range,
  },
  Line {
    low: 29,
    prefix: 6,
    range: 4,
    kind: Kind::Range,
  },
  Line {
    low: 45,
    prefix: 6,
    range: 5,
    kind: Kind::Range,
  },
  Line {
    low: 77,
    prefix: 7,
    range: 6,
    kind: Kind::Range,
  },
  Line {
    low: 141,
    prefix: 7,
    range: 32,
    kind: Kind::Range,
  },
];

const B14: [Line; 5] = [
  Line {
    low: 0,
    prefix: 1,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: -2,
    prefix: 3,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: -1,
    prefix: 3,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 1,
    prefix: 3,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 2,
    prefix: 3,
    range: 0,
    kind: Kind::Range,
  },
];

const B15: [Line; 13] = [
  Line {
    low: 0,
    prefix: 1,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: -1,
    prefix: 3,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 1,
    prefix: 3,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: -2,
    prefix: 4,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: 2,
    prefix: 4,
    range: 0,
    kind: Kind::Range,
  },
  Line {
    low: -4,
    prefix: 5,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: 3,
    prefix: 5,
    range: 1,
    kind: Kind::Range,
  },
  Line {
    low: -8,
    prefix: 6,
    range: 2,
    kind: Kind::Range,
  },
  Line {
    low: 5,
    prefix: 6,
    range: 2,
    kind: Kind::Range,
  },
  Line {
    low: -24,
    prefix: 7,
    range: 4,
    kind: Kind::Range,
  },
  Line {
    low: 9,
    prefix: 7,
    range: 4,
    kind: Kind::Range,
  },
  Line {
    low: -25,
    prefix: 7,
    range: 32,
    kind: Kind::Lower,
  },
  Line {
    low: 25,
    prefix: 7,
    range: 32,
    kind: Kind::Range,
  },
];

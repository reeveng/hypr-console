//! Small constant tables from the H.264 specification.

use console_core_never::Never;
use console_core_number_conversion::index;

pub const ZIGZAG4: [u8; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];

#[rustfmt::skip]
pub const ZIGZAG8: [u8; 64] = [
  0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5,
  12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28,
  35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51,
  58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

pub const DEFAULT_4X4_INTRA: [u8; 16] = [
  6, 13, 13, 20, 20, 20, 28, 28, 28, 28, 32, 32, 32, 37, 37, 42,
];
pub const DEFAULT_4X4_INTER: [u8; 16] = [
  10, 14, 14, 20, 20, 20, 24, 24, 24, 24, 27, 27, 27, 30, 30, 34,
];

#[rustfmt::skip]
pub const DEFAULT_8X8_INTRA: [u8; 64] = [
    6, 10, 10, 13, 11, 13, 16, 16, 16, 16, 18, 18, 18, 18, 18, 23,
    23, 23, 23, 23, 23, 25, 25, 25, 25, 25, 25, 25, 27, 27, 27, 27,
    27, 27, 27, 27, 29, 29, 29, 29, 29, 29, 29, 31, 31, 31, 31, 31,
    31, 33, 33, 33, 33, 33, 36, 36, 36, 36, 38, 38, 38, 40, 40, 42,
];
#[rustfmt::skip]
pub const DEFAULT_8X8_INTER: [u8; 64] = [
    9, 13, 13, 15, 13, 15, 17, 17, 17, 17, 19, 19, 19, 19, 19, 21,
    21, 21, 21, 21, 21, 22, 22, 22, 22, 22, 22, 22, 24, 24, 24, 24,
    24, 24, 24, 24, 25, 25, 25, 25, 25, 25, 25, 27, 27, 27, 27, 27,
    27, 28, 28, 28, 28, 28, 30, 30, 30, 30, 32, 32, 32, 33, 33, 35,
];

#[rustfmt::skip]
pub const QPC_TABLE: [u8; 52] = [
  0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
  16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 29, 30,
  31, 32, 32, 33, 34, 34, 35, 35, 36, 36, 37, 37, 37, 38, 38, 38,
  39, 39, 39, 39,
];

pub const NORM_ADJUST4: [[u16; 3]; 6] = [
  [10, 16, 13],
  [11, 18, 14],
  [13, 20, 16],
  [14, 23, 18],
  [16, 25, 20],
  [18, 29, 23],
];

pub const NORM_ADJUST8: [[u16; 6]; 6] = [
  [20, 18, 32, 19, 25, 24],
  [22, 19, 35, 21, 28, 26],
  [26, 23, 42, 24, 33, 31],
  [28, 25, 45, 26, 35, 33],
  [32, 28, 51, 30, 40, 38],
  [36, 32, 58, 34, 46, 43],
];

pub struct QuantizerRemainder(pub u8);

pub fn norm_adjust4(remainder: QuantizerRemainder, coefficient: u8) -> Result<u16, Never> {
  let Ok(position) = index(remainder.0);
  let (row, column) = (coefficient.wrapping_div(4), coefficient.wrapping_rem(4));
  Ok(match (NORM_ADJUST4.get(position), row & 1, column & 1) {
    (Some([even, _, _]), 0, 0) => *even,
    (Some([_, odd, _]), 1, 1) => *odd,
    (Some([_, _, mixed]), _, _) => *mixed,
    (None, _, _) => 0,
  })
}

pub fn norm_adjust8(remainder: QuantizerRemainder, coefficient: u8) -> Result<u16, Never> {
  let Ok(position) = index(remainder.0);
  let (row, column) = (coefficient.wrapping_div(8) & 3, coefficient & 3);
  let [a, b, c, d, e, f] = match NORM_ADJUST8.get(position) {
    Some(scale) => scale,
    None => return Ok(0),
  };
  Ok(
    *(match (row, column) {
      (0, 0) => a,
      (1 | 3, 1 | 3) => b,
      (2, 2) => c,
      (0, 1 | 3) | (1 | 3, 0) => d,
      (0, 2) | (2, 0) => e,
      _ => f,
    }),
  )
}

#[rustfmt::skip]
pub const ALPHA: [u8; 52] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 4, 5, 6, 7, 8, 9, 10, 12, 13,
    15, 17, 20, 22, 25, 28, 32, 36, 40, 45, 50, 56, 63, 71, 80, 90, 101, 113, 127, 144, 162, 182, 203, 226, 255, 255,
];

#[rustfmt::skip]
pub const BETA: [u8; 52] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4,
    6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 16, 16, 17, 17, 18, 18,
];

#[rustfmt::skip]
pub const TC0: [[u8; 3]; 52] = [
  [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
  [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
  [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
  [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
  [0, 0, 0], [0, 0, 1], [0, 0, 1], [0, 0, 1],
  [0, 0, 1], [0, 1, 1], [0, 1, 1], [1, 1, 1],
  [1, 1, 1], [1, 1, 1], [1, 1, 1], [1, 1, 2],
  [1, 1, 2], [1, 1, 2], [1, 1, 2], [1, 2, 3],
  [1, 2, 3], [2, 2, 3], [2, 2, 4], [2, 3, 4],
  [2, 3, 4], [3, 3, 5], [3, 4, 6], [3, 4, 6],
  [4, 5, 7], [4, 5, 8], [4, 6, 9], [5, 7, 10],
  [6, 8, 11], [6, 8, 13], [7, 10, 14], [8, 11, 16],
  [9, 12, 18], [10, 13, 20], [11, 15, 23], [13, 17, 25],
];

pub const BLK4_XY: [(u8, u8); 16] = [
  (0, 0),
  (1, 0),
  (0, 1),
  (1, 1),
  (2, 0),
  (3, 0),
  (2, 1),
  (3, 1),
  (0, 2),
  (1, 2),
  (0, 3),
  (1, 3),
  (2, 2),
  (3, 2),
  (2, 3),
  (3, 3),
];

pub const RASTER_TO_BLK4: [u8; 16] = [0, 1, 4, 5, 2, 3, 6, 7, 8, 9, 12, 13, 10, 11, 14, 15];

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn zigzag8_known_prefix() {
    assert_eq!(
      ZIGZAG8.get(..16),
      Some([0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5].as_slice())
    );
    assert_eq!(
      ZIGZAG8.get(48..),
      Some(
        [
          58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63
        ]
        .as_slice()
      )
    );
    assert!((0..64).all(|value| ZIGZAG8.contains(&value)));
  }

  #[test]
  fn blk4_maps_consistent() {
    for (id, &(x, y)) in (0_u8..16).zip(BLK4_XY.iter()) {
      let Ok(raster) = index(y.wrapping_mul(4).wrapping_add(x));
      assert_eq!(RASTER_TO_BLK4.get(raster), Some(&id));
    }
  }

  #[test]
  fn qpc() {
    assert_eq!(QPC_TABLE.get(29), Some(&29));
    assert_eq!(QPC_TABLE.get(30), Some(&29));
    assert_eq!(QPC_TABLE.get(51), Some(&39));
  }
}

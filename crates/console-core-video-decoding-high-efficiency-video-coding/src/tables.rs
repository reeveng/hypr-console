//! Derived tables: scan orders (6.5.3 - 6.5.5), chroma QP mapping and small helper tables.

pub use crate::spec_tables::*;

use console_core_never::Never;
use console_core_number_conversion::wrapped;

use crate::places::{read, table_row};


#[rustfmt::skip]
pub static SCAN_1: [[(u8, u8); 1]; 3] = [
  [
    (0, 0),
  ],
  [
    (0, 0),
  ],
  [
    (0, 0),
  ],
];

#[rustfmt::skip]
pub static SCAN_2: [[(u8, u8); 4]; 3] = [
  [
    (0, 0), (0, 1), (1, 0), (1, 1),
  ],
  [
    (0, 0), (1, 0), (0, 1), (1, 1),
  ],
  [
    (0, 0), (0, 1), (1, 0), (1, 1),
  ],
];

#[rustfmt::skip]
pub static SCAN_4: [[(u8, u8); 16]; 3] = [
  [
    (0, 0), (0, 1), (1, 0), (0, 2), (1, 1), (2, 0), (0, 3), (1, 2),
    (2, 1), (3, 0), (1, 3), (2, 2), (3, 1), (2, 3), (3, 2), (3, 3),
  ],
  [
    (0, 0), (1, 0), (2, 0), (3, 0), (0, 1), (1, 1), (2, 1), (3, 1),
    (0, 2), (1, 2), (2, 2), (3, 2), (0, 3), (1, 3), (2, 3), (3, 3),
  ],
  [
    (0, 0), (0, 1), (0, 2), (0, 3), (1, 0), (1, 1), (1, 2), (1, 3),
    (2, 0), (2, 1), (2, 2), (2, 3), (3, 0), (3, 1), (3, 2), (3, 3),
  ],
];

#[rustfmt::skip]
pub static SCAN_8: [[(u8, u8); 64]; 3] = [
  [
    (0, 0), (0, 1), (1, 0), (0, 2), (1, 1), (2, 0), (0, 3), (1, 2),
    (2, 1), (3, 0), (0, 4), (1, 3), (2, 2), (3, 1), (4, 0), (0, 5),
    (1, 4), (2, 3), (3, 2), (4, 1), (5, 0), (0, 6), (1, 5), (2, 4),
    (3, 3), (4, 2), (5, 1), (6, 0), (0, 7), (1, 6), (2, 5), (3, 4),
    (4, 3), (5, 2), (6, 1), (7, 0), (1, 7), (2, 6), (3, 5), (4, 4),
    (5, 3), (6, 2), (7, 1), (2, 7), (3, 6), (4, 5), (5, 4), (6, 3),
    (7, 2), (3, 7), (4, 6), (5, 5), (6, 4), (7, 3), (4, 7), (5, 6),
    (6, 5), (7, 4), (5, 7), (6, 6), (7, 5), (6, 7), (7, 6), (7, 7),
  ],
  [
    (0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0),
    (0, 1), (1, 1), (2, 1), (3, 1), (4, 1), (5, 1), (6, 1), (7, 1),
    (0, 2), (1, 2), (2, 2), (3, 2), (4, 2), (5, 2), (6, 2), (7, 2),
    (0, 3), (1, 3), (2, 3), (3, 3), (4, 3), (5, 3), (6, 3), (7, 3),
    (0, 4), (1, 4), (2, 4), (3, 4), (4, 4), (5, 4), (6, 4), (7, 4),
    (0, 5), (1, 5), (2, 5), (3, 5), (4, 5), (5, 5), (6, 5), (7, 5),
    (0, 6), (1, 6), (2, 6), (3, 6), (4, 6), (5, 6), (6, 6), (7, 6),
    (0, 7), (1, 7), (2, 7), (3, 7), (4, 7), (5, 7), (6, 7), (7, 7),
  ],
  [
    (0, 0), (0, 1), (0, 2), (0, 3), (0, 4), (0, 5), (0, 6), (0, 7),
    (1, 0), (1, 1), (1, 2), (1, 3), (1, 4), (1, 5), (1, 6), (1, 7),
    (2, 0), (2, 1), (2, 2), (2, 3), (2, 4), (2, 5), (2, 6), (2, 7),
    (3, 0), (3, 1), (3, 2), (3, 3), (3, 4), (3, 5), (3, 6), (3, 7),
    (4, 0), (4, 1), (4, 2), (4, 3), (4, 4), (4, 5), (4, 6), (4, 7),
    (5, 0), (5, 1), (5, 2), (5, 3), (5, 4), (5, 5), (5, 6), (5, 7),
    (6, 0), (6, 1), (6, 2), (6, 3), (6, 4), (6, 5), (6, 6), (6, 7),
    (7, 0), (7, 1), (7, 2), (7, 3), (7, 4), (7, 5), (7, 6), (7, 7),
  ],
];

#[inline]
pub fn scan_order(log2: u32, scan: u64) -> Result<&'static [(u8, u8)], Never> {
  match log2 {
    0 => table_row(&SCAN_1, scan),
    1 => table_row(&SCAN_2, scan),
    2 => table_row(&SCAN_4, scan),
    _ => table_row(&SCAN_8, scan),
  }
}

pub fn scan_diag(log2: u32) -> Result<&'static [(u8, u8)], Never> {
  scan_order(log2, 0)
}

#[inline]
pub fn qpc_420(qpi: i32) -> Result<i32, Never> {
  match qpi {
    ..=29 => Ok(qpi),
    44.. => Ok(qpi.wrapping_sub(6)),
    _ => {
      let Ok(at) = wrapped::<_, u64>(qpi.wrapping_sub(30));
      let Ok(mapped) = read(&QPC_30_43, at);
      Ok(i32::from(mapped))
    }
  }
}

pub static CTX_IDX_MAP: [u8; 16] = [0, 1, 4, 5, 2, 3, 4, 5, 6, 6, 8, 8, 7, 7, 8, 8];

pub static LEVEL_SCALE: [i32; 6] = [40, 45, 51, 57, 64, 72];

pub static DST_MATRIX: [[i8; 4]; 4] = [
  [29, 55, 74, 84],
  [74, 74, 0, -74],
  [84, -29, -74, 55],
  [55, -84, 74, -29],
];

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn diagonal_scan_4x4() {
    let s = &*match &SCAN_4 {
      [element, ..] => element,
    };
    assert_eq!(s.first_chunk::<6>(), Some(&[(0, 0), (0, 1), (1, 0), (0, 2), (1, 1), (2, 0)]));
    assert_eq!(
      *match &*s {
        [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, element, ..] => element,
      },
      (3, 3)
    );
    assert_eq!(
      *match &SCAN_2 {
        [element, ..] => element,
      },
      [(0, 0), (0, 1), (1, 0), (1, 1)]
    );
    assert_eq!(
      *match &(*match &SCAN_4 {
        [_, _, element, ..] => element,
      }) {
        [_, element_2, ..] => element_2,
      },
      (0, 1)
    );
    assert_eq!(
      *match &(*match &SCAN_4 {
        [_, element, ..] => element,
      }) {
        [_, element_2, ..] => element_2,
      },
      (1, 0)
    );
  }

  #[test]
  fn dct_matrix_properties() {
    for (a, row_a) in TRANS_MATRIX.iter().enumerate() {
      for (b, row_b) in TRANS_MATRIX.iter().enumerate() {
        let d: i64 = row_a
          .iter()
          .zip(row_b)
          .map(|(&left, &right)| i64::wrapping_mul(i64::from(left), i64::from(right)))
          .sum();

        match ::core::primitive::bool::then_some(a == b, ()) {
          ::core::option::Option::Some(()) => {
            assert!(
              ::core::primitive::i64::wrapping_sub(d, 131072).abs() < 1000,
              "row {a} norm {d}"
            );
          }
          ::core::option::Option::None => {
            assert!(d.abs() < 1000, "rows {a},{b}: {d}");
          }
        }
      }
    }

    for (n, row) in TRANS_MATRIX.iter().enumerate() {
      for (m, (&a, &b)) in row.iter().zip(row.iter().rev()).take(16).enumerate() {
        let (a_2, b_2) = (i32::from(a), i32::from(b));
        assert_eq!(
          a_2,
          match n.wrapping_rem(2) {
            0 => b_2,
            _ => b_2.wrapping_neg(),
          },
          "{n} {m}"
        );
      }
    }
  }

  #[test]
  fn qpc_table() {
    let Ok(below_transition) = qpc_420(29);
    let Ok(at_transition) = qpc_420(30);
    let Ok(upper) = qpc_420(43);
    let Ok(highest) = qpc_420(51);
    assert_eq!(below_transition, 29);
    assert_eq!(at_transition, 29);
    assert_eq!(upper, 37);
    assert_eq!(highest, 45);
  }
}

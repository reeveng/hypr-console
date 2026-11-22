//! Constants, small lookup tables and decode trees of the VP9 specification (sections 3, 6, 9.3
//! and 10.2). The large tables are extracted from the specification text into `spec_tables`.
//!
//! Two tables the decoder this came from built once and kept for the whole process, the Pareto
//! rows of every probability and the two neighbours of every scan position, are worked out where
//! they are asked for instead. Each is a few additions over a table that is already here, and a
//! table filled on first use is a value the whole process shares without anybody having handed
//! it over.

use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::frame::Subsampling;
use crate::places::{read, row};

pub use crate::spec_tables::*;

pub const BLOCK_8X8: u8 = 3;
pub const BLOCK_16X16: u8 = 6;
pub const BLOCK_64X64: u8 = 12;
pub const BLOCK_INVALID: u8 = 14;

pub const TX_4X4: u8 = 0;
pub const TX_8X8: u8 = 1;
pub const TX_16X16: u8 = 2;
pub const TX_32X32: u8 = 3;
pub const TX_MODE_SELECT: u8 = 4;

pub const DC_PRED: u8 = 0;
pub const V_PRED: u8 = 1;
pub const H_PRED: u8 = 2;
pub const D45_PRED: u8 = 3;
pub const D135_PRED: u8 = 4;
pub const D117_PRED: u8 = 5;
pub const D153_PRED: u8 = 6;
pub const D207_PRED: u8 = 7;
pub const D63_PRED: u8 = 8;
pub const TM_PRED: u8 = 9;
pub const NEARESTMV: u8 = 10;
pub const NEARMV: u8 = 11;
pub const ZEROMV: u8 = 12;
pub const NEWMV: u8 = 13;

pub const NONE: i8 = -1;
pub const INTRA_FRAME: i8 = 0;
pub const LAST_FRAME: i8 = 1;
pub const GOLDEN_FRAME: i8 = 2;
pub const ALTREF_FRAME: i8 = 3;

pub const DCT_DCT: u8 = 0;
pub const ADST_DCT: u8 = 1;
pub const DCT_ADST: u8 = 2;
pub const ADST_ADST: u8 = 3;

pub const SWITCHABLE: u8 = 4;

pub const SEG_LVL_ALT_Q: u64 = 0;
pub const SEG_LVL_ALT_L: u64 = 1;
pub const SEG_LVL_REF_FRAME: u64 = 2;
pub const SEG_LVL_SKIP: u64 = 3;
pub const MAX_FRAME_SIDE: u32 = 16384;
pub const MAX_MI_COUNT: u64 = 4194304;

pub const MAX_LOOP_FILTER: i32 = 63;
pub const MIN_TILE_WIDTH_B64: u32 = 4;
pub const MAX_TILE_WIDTH_B64: u32 = 64;
pub const COMPANDED_MVREF_THRESH: i32 = 8;
pub const MV_BORDER: i32 = 128;
pub const INTERP_EXTEND: i32 = 4;
pub const BORDERINPIXELS: i32 = 160;
pub const COUNT_SAT: u32 = 20;
pub const MAX_UPDATE_FACTOR: u32 = 128;

pub const B_WIDTH_LOG2: [u8; 13] = [0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4];
pub const B_HEIGHT_LOG2: [u8; 13] = [0, 1, 0, 1, 2, 1, 2, 3, 2, 3, 4, 3, 4];
pub const NUM_4X4_WIDE: [u8; 13] = [1, 1, 2, 2, 2, 4, 4, 4, 8, 8, 8, 16, 16];
pub const NUM_4X4_HIGH: [u8; 13] = [1, 2, 1, 2, 4, 2, 4, 8, 4, 8, 16, 8, 16];
pub const MI_WIDTH_LOG2: [u8; 13] = [0, 0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3];
pub const NUM_8X8_WIDE: [u8; 13] = [1, 1, 1, 1, 1, 2, 2, 2, 4, 4, 4, 8, 8];
pub const NUM_8X8_HIGH: [u8; 13] = [1, 1, 1, 1, 2, 1, 2, 4, 2, 4, 8, 4, 8];
pub const SIZE_GROUP: [u8; 13] = [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 3];
pub const MAX_TXSIZE: [u8; 13] = [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 3];
pub const TX_MODE_TO_BIGGEST_TX_SIZE: [u8; 5] = [0, 1, 2, 3, 3];

pub const SUBSIZE_LOOKUP: [[u8; 13]; 4] = [
  [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
  [14, 14, 14, 2, 14, 14, 5, 14, 14, 8, 14, 14, 11],
  [14, 14, 14, 1, 14, 14, 4, 14, 14, 7, 14, 14, 10],
  [14, 14, 14, 0, 14, 14, 3, 14, 14, 6, 14, 14, 9],
];

pub const SS_SIZE_LOOKUP: [[[u8; 2]; 2]; 13] = [
  [[0, 14], [14, 14]],
  [[1, 0], [14, 14]],
  [[2, 14], [0, 14]],
  [[3, 2], [1, 0]],
  [[4, 3], [14, 1]],
  [[5, 14], [3, 2]],
  [[6, 5], [4, 3]],
  [[7, 6], [14, 4]],
  [[8, 14], [6, 5]],
  [[9, 8], [7, 6]],
  [[10, 9], [14, 7]],
  [[11, 14], [9, 8]],
  [[12, 11], [10, 9]],
];

pub const MODE2TXFM: [u8; 14] = [
  DCT_DCT, ADST_DCT, DCT_ADST, DCT_DCT, ADST_ADST, ADST_DCT, DCT_ADST, DCT_ADST, ADST_DCT,
  ADST_ADST, DCT_DCT, DCT_DCT, DCT_DCT, DCT_DCT,
];

pub const COUNTER_TO_CONTEXT: [u8; 19] =
  [2, 3, 4, 1, 3, 9, 0, 9, 9, 5, 5, 9, 5, 9, 9, 9, 9, 9, 6];

pub const IDX_N_COLUMN_TO_SUBBLOCK: [[u8; 2]; 4] = [[1, 2], [1, 3], [3, 2], [3, 3]];

pub const LITERAL_TO_TYPE: [u8; 4] = [1, 0, 2, 3];

pub const SEGMENTATION_FEATURE_BITS: [u32; 4] = [8, 6, 2, 0];
pub const SEGMENTATION_FEATURE_SIGNED: [bool; 4] = [true, true, false, false];

pub const EXTRA_BITS: [(u8, u8, u16); 11] = [
  (0, 0, 0),
  (0, 0, 1),
  (0, 0, 2),
  (0, 0, 3),
  (0, 0, 4),
  (1, 1, 5),
  (2, 2, 7),
  (3, 3, 11),
  (4, 4, 19),
  (5, 5, 35),
  (6, 14, 67),
];

pub const CAT_PROBS: [&[u8]; 7] = [
  &[],
  &[159],
  &[165, 145],
  &[173, 148, 140],
  &[176, 155, 140, 135],
  &[180, 157, 141, 134, 130],
  &[
    254, 254, 254, 252, 249, 243, 230, 196, 177, 153, 140, 133, 130, 129,
  ],
];

pub const PARTITION_TREE: [i8; 6] = [0, 2, -1, 4, -2, -3];
pub const INTRA_MODE_TREE: [i8; 18] = [
  -DC_PRED.cast_signed(),
  2,
  -TM_PRED.cast_signed(),
  4,
  -V_PRED.cast_signed(),
  6,
  8,
  12,
  -H_PRED.cast_signed(),
  10,
  -D135_PRED.cast_signed(),
  -D117_PRED.cast_signed(),
  -D45_PRED.cast_signed(),
  14,
  -D63_PRED.cast_signed(),
  16,
  -D153_PRED.cast_signed(),
  -D207_PRED.cast_signed(),
];
pub const SEGMENT_TREE: [i8; 14] = [2, 4, 6, 8, 10, 12, 0, -1, -2, -3, -4, -5, -6, -7];
pub const TX_SIZE_32_TREE: [i8; 6] = [0, 2, -1, 4, -2, -3];
pub const TX_SIZE_16_TREE: [i8; 4] = [0, 2, -1, -2];
pub const TX_SIZE_8_TREE: [i8; 2] = [0, -1];

pub const INTER_MODE_TREE: [i8; 6] = [-2, 2, 0, 4, -1, -3];
pub const INTERP_FILTER_TREE: [i8; 4] = [0, 2, -1, -2];
pub const MV_JOINT_TREE: [i8; 6] = [0, 2, -1, 4, -2, -3];
pub const MV_CLASS_TREE: [i8; 20] = [
  0, 2, -1, 4, 6, 8, -2, -3, 10, 12, -4, -5, -6, 14, 16, 18, -7, -8, -9, -10,
];
pub const MV_FR_TREE: [i8; 6] = [0, 2, -1, 4, -2, -3];

pub const SMALL_TOKEN_TREE: [i8; 6] = [0, 0, 0, 4, -1, -2];
pub const BINARY_TREE: [i8; 2] = [0, -1];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transform {
  pub size: u8,
  pub kind: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Neighbours {
  pub above: u64,
  pub left: u64,
}

pub fn subsampled_size(size: u8, subsampling: Subsampling) -> Result<u8, Never> {
  let Ok(across) = row(&SS_SIZE_LOOKUP, u64::from(size));
  let Ok(down) = row(across, u64::from(subsampling.x));

  read(down, u64::from(subsampling.y))
}

pub fn pareto(probability: u8) -> Result<[u8; 8], Never> {
  let chosen = u64::from(probability.max(1));
  let pair = chosen.wrapping_sub(1).wrapping_shr(1);
  let first = pair.wrapping_mul(8);
  let second = pair.wrapping_add(1).wrapping_mul(8);
  let mut entries = [0u8; 8];

  for (node, entry) in (0_u64..).zip(entries.iter_mut()) {
    let Ok(near) = read(&PARETO_TABLE, first.wrapping_add(node));

    *entry = match chosen & 1 {
      1 => near,
      _even => {
        let Ok(far) = read(&PARETO_TABLE, second.wrapping_add(node));
        let Ok(between) =
          fitted::<u16, u8>(u16::from(near).wrapping_add(u16::from(far)).wrapping_shr(1));

        between
      }
    };
  }

  Ok(entries)
}

pub fn scan(transform: Transform) -> Result<&'static [u16], Never> {
  Ok(match (transform.size, transform.kind) {
    (0, ADST_DCT) => &ROW_SCAN_4X4,
    (0, DCT_ADST) => &COL_SCAN_4X4,
    (0, _other) => &DEFAULT_SCAN_4X4,
    (1, ADST_DCT) => &ROW_SCAN_8X8,
    (1, DCT_ADST) => &COL_SCAN_8X8,
    (1, _other) => &DEFAULT_SCAN_8X8,
    (2, ADST_DCT) => &ROW_SCAN_16X16,
    (2, DCT_ADST) => &COL_SCAN_16X16,
    (2, _other) => &DEFAULT_SCAN_16X16,
    (_largest, _any) => &DEFAULT_SCAN_32X32,
  })
}

pub fn neighbours(transform: Transform, position: u64) -> Result<Neighbours, Never> {
  let log2 = u32::from(transform.size).wrapping_add(2);
  let line = position.wrapping_shr(log2);
  let column = position & 1_u64.wrapping_shl(log2).wrapping_sub(1);
  let above = position.wrapping_sub(1_u64.wrapping_shl(log2));
  let left = position.wrapping_sub(1);

  let kind = match transform.size {
    TX_32X32 => DCT_DCT,
    _smaller => transform.kind,
  };

  Ok(match (line, column, kind) {
    (0, _any, _kind) => Neighbours { above: left, left },
    (_below, 0, _kind) => Neighbours { above, left: above },
    (_below, _right, DCT_ADST) => Neighbours { above, left: above },
    (_below, _right, ADST_DCT) => Neighbours { above: left, left },
    (_below, _right, _both) => Neighbours { above, left },
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::places::span;

  fn leaves(tree: &[i8]) -> Result<Vec<i8>, Never> {
    let mut found: Vec<i8> = tree
      .iter()
      .filter(|node| **node <= 0)
      .map(|node| node.wrapping_neg())
      .collect();

    found.sort_unstable();

    Ok(found)
  }

  #[test]
  fn trees_are_consistent() {
    let Ok(intra) = leaves(&INTRA_MODE_TREE);
    let Ok(class) = leaves(&MV_CLASS_TREE);
    let Ok(partition) = leaves(&PARTITION_TREE);
    let Ok(inter) = leaves(&INTER_MODE_TREE);

    assert_eq!(intra, (0..10).collect::<Vec<i8>>());
    assert_eq!(class, (0..11).collect::<Vec<i8>>());
    assert_eq!(partition, (0..4).collect::<Vec<i8>>());
    assert_eq!(inter, (0..4).collect::<Vec<i8>>());
  }

  #[test]
  fn pareto_rows() {
    let Ok(first) = pareto(1);
    let Ok(last) = pareto(255);
    let Ok(written) = span(&PARETO_TABLE, 1016..1024);

    assert_eq!(first, [3, 86, 128, 6, 86, 23, 88, 29]);
    assert_eq!(last.as_slice(), written);
  }

  #[test]
  fn default_coef_probs_unused_are_zero() {
    for (group, chunk) in (0u64..).zip(DEFAULT_COEF_PROBS.chunks(18)) {
      let band = group.wrapping_rem(6);

      for (context, probabilities) in (0_u64..).zip(chunk.chunks(3)) {
        match (band, context) {
          (0, 3..) => assert_eq!(probabilities, [0, 0, 0]),
          (_band, _context) => assert!(
            probabilities.iter().all(|probability| *probability > 0),
            "band {band} context {context}"
          ),
        }
      }
    }
  }
}

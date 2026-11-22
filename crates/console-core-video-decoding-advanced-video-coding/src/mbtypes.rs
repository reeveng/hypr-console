//! Macroblock / sub-macroblock type tables (7.4.5, Tables 7-11 .. 7-18).

use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;

use crate::picture::MbKind;

pub const PRED_L0: u8 = 1;
pub const PRED_L1: u8 = 2;
pub const PRED_BI: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Part {
  #[default]
  P16x16,
  P16x8,
  P8x16,
  P8x8,
}

fn rectangle(origin: Point<u8>, size: Size<u8>) -> Result<Rectangle<u8>, Never> {
  Ok(Rectangle { origin, size })
}

impl Part {
  pub(crate) fn block_count(self) -> Result<u8, Never> {
    Ok(match self {
      Self::P16x16 => 1,
      Self::P16x8 | Self::P8x16 => 2,
      Self::P8x8 => 4,
    })
  }

  pub(crate) fn block(self, position: u8) -> Result<Rectangle<u8>, Never> {
    let (origin, size) = match self {
      Self::P16x16 => (
        Point { x: 0, y: 0 },
        Size {
          width: 4,
          height: 4,
        },
      ),
      Self::P16x8 => (
        Point {
          x: 0,
          y: position.wrapping_mul(2),
        },
        Size {
          width: 4,
          height: 2,
        },
      ),
      Self::P8x16 => (
        Point {
          x: position.wrapping_mul(2),
          y: 0,
        },
        Size {
          width: 2,
          height: 4,
        },
      ),
      Self::P8x8 => (
        Point {
          x: (position & 1).wrapping_mul(2),
          y: position.wrapping_shr(1).wrapping_mul(2),
        },
        Size {
          width: 2,
          height: 2,
        },
      ),
    };
    rectangle(origin, size)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SubPart {
  #[default]
  S8x8,
  S8x4,
  S4x8,
  S4x4,
}

impl SubPart {
  pub(crate) fn block_count(self) -> Result<u8, Never> {
    Ok(match self {
      Self::S8x8 => 1,
      Self::S8x4 | Self::S4x8 => 2,
      Self::S4x4 => 4,
    })
  }

  pub(crate) fn block(self, position: u8) -> Result<Rectangle<u8>, Never> {
    let (origin, size) = match self {
      Self::S8x8 => (
        Point { x: 0, y: 0 },
        Size {
          width: 2,
          height: 2,
        },
      ),
      Self::S8x4 => (
        Point { x: 0, y: position },
        Size {
          width: 2,
          height: 1,
        },
      ),
      Self::S4x8 => (
        Point { x: position, y: 0 },
        Size {
          width: 1,
          height: 2,
        },
      ),
      Self::S4x4 => (
        Point {
          x: position & 1,
          y: position.wrapping_shr(1),
        },
        Size {
          width: 1,
          height: 1,
        },
      ),
    };
    rectangle(origin, size)
  }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MbTypeInfo {
  pub kind: MbKind,

  pub i16_mode: u8,

  pub i16_cbp: u8,
  pub part: Part,

  pub pred: [u8; 2],

  pub ref0: bool,
}

pub fn intra_mb_type(
  t: u32,
) -> core::result::Result<Option<MbTypeInfo>, console_core_never::Never> {
  let mut m = MbTypeInfo::default();

  match t {
    0 => m.kind = MbKind::I4x4,
    1..=24 => {
      m.kind = MbKind::I16x16;
      m.i16_mode = {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(
          ::core::primitive::u32::wrapping_rem(u32::wrapping_sub(t, 1), 4),
        );
        numeric_value
      };
      let chroma = ::core::primitive::u32::wrapping_rem(
        ::core::primitive::u32::wrapping_div(u32::wrapping_sub(t, 1), 4),
        3,
      );
      let luma = match ::core::primitive::bool::then_some(t >= 13, ()) {
        ::core::option::Option::Some(()) => 15,
        ::core::option::Option::None => 0,
      };
      m.i16_cbp = {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(
          luma | u32::wrapping_shl(chroma, 4_u32),
        );
        numeric_value
      };
    }
    25 => m.kind = MbKind::IPcm,
    _ => return Ok(None),
  }

  Ok(Some(m))
}

pub fn p_mb_type(t: u32) -> core::result::Result<Option<MbTypeInfo>, console_core_never::Never> {
  let mut m = MbTypeInfo {
    kind: MbKind::Inter,
    pred: [PRED_L0; 2],
    ..Default::default()
  };
  m.part = match t {
    0 => Part::P16x16,
    1 => Part::P16x8,
    2 => Part::P8x16,
    3 => Part::P8x8,
    4 => {
      m.ref0 = true;
      Part::P8x8
    }
    _ => return Ok(None),
  };

  Ok(Some(m))
}

pub fn b_mb_type(t: u32) -> core::result::Result<Option<MbTypeInfo>, console_core_never::Never> {
  let mut m = MbTypeInfo {
    kind: MbKind::Inter,
    ..Default::default()
  };

  match t {
    0 => {
      m.kind = MbKind::BDirect16x16;
      m.pred = [PRED_BI; 2];
    }
    1..=3 => {
      m.part = Part::P16x16;
      m.pred = [({
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(t);
        numeric_value
      }); 2];
    }
    4..=21 => {
      const PAIRS: [(u8, u8); 9] = [
        (PRED_L0, PRED_L0),
        (PRED_L1, PRED_L1),
        (PRED_L0, PRED_L1),
        (PRED_L1, PRED_L0),
        (PRED_L0, PRED_BI),
        (PRED_L1, PRED_BI),
        (PRED_BI, PRED_L0),
        (PRED_BI, PRED_L1),
        (PRED_BI, PRED_BI),
      ];
      let code = t.wrapping_sub(4).wrapping_div(2);
      let Ok(position) = console_core_number_conversion::index(code);
      let &(a, b) = match PAIRS.get(position) {
        Some(pair) => pair,
        None => return Ok(None),
      };
      m.pred = [a, b];
      m.part = match ::core::primitive::bool::then_some(t.is_multiple_of(2), ()) {
        ::core::option::Option::Some(()) => Part::P16x8,
        ::core::option::Option::None => Part::P8x16,
      };
    }
    22 => m.part = Part::P8x8,
    _ => return Ok(None),
  }

  Ok(Some(m))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubMbInfo {
  pub shape: SubPart,
  pub pred: u8,
  pub direct: bool,
}

pub fn p_sub_mb_type(
  t: u32,
) -> core::result::Result<Option<SubMbInfo>, console_core_never::Never> {
  let shape = match t {
    0 => SubPart::S8x8,
    1 => SubPart::S8x4,
    2 => SubPart::S4x8,
    3 => SubPart::S4x4,
    _ => return Ok(None),
  };

  Ok(Some(SubMbInfo {
    shape,
    pred: PRED_L0,
    direct: false,
  }))
}

pub fn b_sub_mb_type(
  t: u32,
) -> core::result::Result<Option<SubMbInfo>, console_core_never::Never> {
  let (shape, pred, direct) = match t {
    0 => (SubPart::S8x8, PRED_BI, true),
    1 => (SubPart::S8x8, PRED_L0, false),
    2 => (SubPart::S8x8, PRED_L1, false),
    3 => (SubPart::S8x8, PRED_BI, false),
    4 => (SubPart::S8x4, PRED_L0, false),
    5 => (SubPart::S4x8, PRED_L0, false),
    6 => (SubPart::S8x4, PRED_L1, false),
    7 => (SubPart::S4x8, PRED_L1, false),
    8 => (SubPart::S8x4, PRED_BI, false),
    9 => (SubPart::S4x8, PRED_BI, false),
    10 => (SubPart::S4x4, PRED_L0, false),
    11 => (SubPart::S4x4, PRED_L1, false),
    12 => (SubPart::S4x4, PRED_BI, false),
    _ => return Ok(None),
  };

  Ok(Some(SubMbInfo {
    shape,
    pred,
    direct,
  }))
}

#[rustfmt::skip]
pub const CBP_ME: [(u8, u8); 48] = [
    (47, 0), (31, 16), (15, 1), (0, 2), (23, 4), (27, 8), (29, 32), (30, 3),
    (7, 5), (11, 10), (13, 12), (14, 15), (39, 47), (43, 7), (45, 11), (46, 13),
    (16, 14), (3, 6), (5, 9), (10, 31), (12, 35), (19, 37), (21, 42), (26, 44),
    (28, 33), (35, 34), (37, 36), (42, 40), (44, 39), (1, 43), (2, 45), (4, 46),
    (8, 17), (17, 18), (18, 20), (20, 24), (24, 19), (6, 21), (9, 26), (22, 28),
    (25, 23), (32, 27), (33, 29), (34, 30), (36, 22), (40, 25), (38, 38), (41, 41),
];

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Debug)]
  enum CbpTableError {
    IntraPosition,
    InterPosition,
  }

  #[test]
  fn cbp_table_is_permutation() -> Result<(), CbpTableError> {
    let mut a = [false; 48];
    let mut b = [false; 48];

    for &(i, p) in &CBP_ME {
      let Ok(intra) = console_core_number_conversion::index(i);
      let Ok(inter) = console_core_number_conversion::index(p);
      let left = a.get_mut(intra).ok_or(CbpTableError::IntraPosition)?;
      let right = b.get_mut(inter).ok_or(CbpTableError::InterPosition)?;
      *left = true;
      *right = true;
    }

    assert!(a.iter().all(|&x| x) && b.iter().all(|&x| x));
    Ok(())
  }

  #[derive(Debug)]
  enum MbTypeTestError {
    IntraType,
    BidirectionalType,
  }

  #[test]
  fn i16_types() -> Result<(), MbTypeTestError> {
    for (code, mode, pattern) in [(1, 0, 0), (24, 3, 47), (13, 0, 15)] {
      let Ok(mapped) = intra_mb_type(code);
      let block = mapped.ok_or(MbTypeTestError::IntraType)?;
      assert_eq!(
        (block.kind, block.i16_mode, block.i16_cbp),
        (MbKind::I16x16, mode, pattern)
      );
    }

    Ok(())
  }

  #[test]
  fn b_types() -> Result<(), MbTypeTestError> {
    for (code, part, prediction) in [
      (12, Part::P16x8, [PRED_L0, PRED_BI]),
      (21, Part::P8x16, [PRED_BI, PRED_BI]),
      (11, Part::P8x16, [PRED_L1, PRED_L0]),
    ] {
      let Ok(mapped) = b_mb_type(code);
      let block = mapped.ok_or(MbTypeTestError::BidirectionalType)?;
      assert_eq!((block.part, block.pred), (part, prediction));
    }

    Ok(())
  }
}

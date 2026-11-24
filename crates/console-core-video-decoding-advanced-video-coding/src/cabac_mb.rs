//! CABAC slice data and macroblock layer parsing (7.3.4, 7.3.5 with 9.3 binarisations and context
//! index derivations).

use super::{CoefficientRange, MbCur, PredictionList, SliceDecoder, TransformEligibility};
use crate::cabac::Cabac;
use crate::error::{Error, Result, ensure, invalid};
use crate::mbtypes::*;
use crate::picture::MacroblockPrediction;
use crate::picture::{ChromaPlane, MbKind, MbState, TransformSize};
use crate::slice::SliceType;
use console_core_adaptive_arithmetic_decoding::{BitPosition, BytePosition, Exhaustion};
use console_core_geometry::{Point, Rectangle};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};

#[rustfmt::skip]
const SIG8_FRAME: [u8; 63] = [
    0, 1, 2, 3, 4, 5, 5, 4, 4, 3, 3, 4, 4, 4, 5, 5, 4, 4, 4, 4, 3, 3, 6, 7, 7, 7, 8, 9, 10, 9, 8, 7,
    7, 6, 11, 12, 13, 11, 6, 7, 8, 9, 14, 10, 9, 8, 6, 11, 12, 13, 11, 6, 9, 14, 10, 9, 11, 12, 13, 11, 14, 10, 12,
];

#[rustfmt::skip]
const LAST8: [u8; 63] = [
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8,
];

const CHROMA_DC_CTX: [u8; 4] = [0, 1, 2, 2];

#[derive(Clone, Copy)]
enum ResidualCategory {
  LumaDc,
  LumaAc,
  Luma4x4,
  ChromaDc,
  ChromaAc,
  Luma8x8,
}

#[derive(Clone, Copy)]
struct ResidualContexts {
  coded: Option<u16>,
  significant: u16,
  last: u16,
  magnitude: u16,
}

impl From<ResidualCategory> for ResidualContexts {
  fn from(category: ResidualCategory) -> Self {
    let (coded, significant, last, magnitude) = match category {
      ResidualCategory::LumaDc => (Some(85), 105, 166, 227),
      ResidualCategory::LumaAc => (Some(89), 120, 181, 237),
      ResidualCategory::Luma4x4 => (Some(93), 134, 195, 247),
      ResidualCategory::ChromaDc => (Some(97), 149, 210, 257),
      ResidualCategory::ChromaAc => (Some(101), 152, 213, 266),
      ResidualCategory::Luma8x8 => (None, 402, 417, 426),
    };
    Self {
      coded,
      significant,
      last,
      magnitude,
    }
  }
}

impl ResidualCategory {
  fn scan_contexts(self, position: u8) -> core::result::Result<(u16, u16), Never> {
    let Ok(offset) = index(position);
    let identity = u16::from(position);
    Ok(match self {
      Self::ChromaDc => match CHROMA_DC_CTX.get(offset) {
        Some(&increment) => (u16::from(increment), u16::from(increment)),
        None => (identity, identity),
      },
      Self::Luma8x8 => match (SIG8_FRAME.get(offset), LAST8.get(offset)) {
        (Some(&significant), Some(&last)) => (u16::from(significant), u16::from(last)),
        _ => (identity, identity),
      },
      Self::LumaDc | Self::LumaAc | Self::Luma4x4 | Self::ChromaAc => (identity, identity),
    })
  }
}

#[derive(Default)]
struct CoefficientHistory {
  unit_count: u16,
  larger_count: u16,
}

fn coefficient_magnitude(
  cabac: &mut Cabac,
  context: u16,
  category: ResidualCategory,
  history: &mut CoefficientHistory,
) -> core::result::Result<i32, Never> {
  let first = match history.larger_count {
    0 => history.unit_count.wrapping_add(1).min(4),
    _ => 0,
  };
  let Ok(larger) = cabac.decode_context(context.wrapping_add(first));

  match larger {
    0 => {
      history.unit_count = history.unit_count.wrapping_add(1);
      return Ok(1);
    }
    _ => {}
  }

  history.larger_count = history.larger_count.wrapping_add(1);
  let cap = match category {
    ResidualCategory::ChromaDc => 3,
    ResidualCategory::LumaDc
    | ResidualCategory::LumaAc
    | ResidualCategory::Luma4x4
    | ResidualCategory::ChromaAc
    | ResidualCategory::Luma8x8 => 4,
  };
  let unary = context
    .wrapping_add(5)
    .wrapping_add(history.larger_count.wrapping_sub(1).min(cap));

  for magnitude in 2i32..15 {
    let Ok(extension) = cabac.decode_context(unary);

    match extension {
      0 => {
        return Ok(magnitude);
      }
      _ => {}
    }
  }

  let Ok(suffix) = decode_bypass_suffix(cabac, 0);
  Ok(15i32.wrapping_add(suffix))
}

fn decode_bypass_suffix(
  cabac: &mut Cabac,
  leading_bits: u32,
) -> core::result::Result<i32, Never> {
  let mut magnitude = 0i32;
  let mut length = 25;

  for bit in leading_bits..=24 {
    let Ok(extension) = cabac.decode_bypass();

    match extension {
      0 => {
        length = bit;
        break;
      }
      _ => {}
    }

    magnitude = magnitude.wrapping_add(1i32.wrapping_shl(bit));
  }

  for bit in (0..length).rev() {
    let Ok(bypass) = cabac.decode_bypass();
    let Ok(symbol) = wrapped::<_, i32>(bypass);
    magnitude = magnitude.wrapping_add(symbol.wrapping_shl(bit));
  }

  Ok(magnitude)
}

fn residual_block_cabac(
  cabac: &mut Cabac,
  coefficients: &mut [i32],
  category: ResidualCategory,
  coded_increment: u16,
) -> core::result::Result<u8, Never> {
  let contexts = ResidualContexts::from(category);

  match contexts.coded {
    Some(base) => {
      let Ok(coded) = cabac.decode_context(base.wrapping_add(coded_increment));

      match coded {
        0 => {
          return Ok(0);
        }
        _ => {}
      }
    }
    None => {}
  }

  let Ok(length) = fixed_index(coefficients.len());
  let limit = match u8::try_from(length) {
    Ok(bounded @ 1..=64) => bounded,
    Ok(_unsupported_length) => return Ok(0),
    Err(_unsupported_length) => return Ok(0),
  };
  let implicit_last = limit.wrapping_sub(1);
  let mut positions = 1u64.wrapping_shl(u32::from(implicit_last));

  for position in 0..implicit_last {
    let Ok((significant, last)) = category.scan_contexts(position);
    let Ok(present) = cabac.decode_context(contexts.significant.wrapping_add(significant));

    match present {
      0 => {
        continue;
      }
      _ => {}
    }

    positions |= 1u64.wrapping_shl(u32::from(position));
    let Ok(final_position) = cabac.decode_context(contexts.last.wrapping_add(last));

    match final_position {
      1 => {
        positions &= !1u64.wrapping_shl(u32::from(implicit_last));
        break;
      }
      _ => {}
    }
  }

  let mut history = CoefficientHistory::default();
  let mut count = 0u8;

  for _ in 0..positions.count_ones() {
    let highest = 63u32.wrapping_sub(positions.leading_zeros());
    positions &= !1u64.wrapping_shl(highest);
    let Ok(position) = index(highest);
    let coefficient = match coefficients.get_mut(position) {
      Some(coefficient) => coefficient,
      None => break,
    };
    let Ok(magnitude) = coefficient_magnitude(cabac, contexts.magnitude, category, &mut history);
    let Ok(sign) = cabac.decode_bypass();
    *coefficient = match sign {
      1 => magnitude.wrapping_neg(),
      _ => magnitude,
    };
    count = count.wrapping_add(1);
  }

  Ok(count)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacroblockSyntax {
  Skip,
  Coded,
}

#[derive(Clone, Copy)]
enum IntraContexts {
  Intra,
  Predictive,
  Bidirectional,
}

#[derive(Clone, Copy)]
enum MotionComponent {
  Horizontal,
  Vertical,
}

fn motion_point(origin: Point<u8>) -> core::result::Result<Point<i8>, Never> {
  let Ok(x) = wrapped(origin.x);
  let Ok(y) = wrapped(origin.y);
  Ok(Point { x, y })
}

#[derive(Clone, Copy)]
enum CoefficientPlane {
  Luma,
  Cb,
  Cr,
}

impl CoefficientPlane {
  fn coefficient_count(
    self,
    state: &MbState,
    location: Point<u8>,
  ) -> core::result::Result<u16, Never> {
    let [cb, cr] = &state.nnz_c;
    let (coefficients, stride): (&[u8], u8) = match self {
      Self::Luma => (&state.nnz, 4),
      Self::Cb => (cb, 2),
      Self::Cr => (cr, 2),
    };
    let Ok(position) = index(location.y.wrapping_mul(stride) | location.x);
    Ok(u16::from(
      matches!(coefficients.get(position), Some(&count) if count != 0),
    ))
  }

  fn chroma_counts(self, state: &mut MbState) -> Result<&mut [u8; 4]> {
    let [cb, cr] = &mut state.nnz_c;

    match self {
      Self::Cb => Ok(cb),
      Self::Cr => Ok(cr),
      Self::Luma => invalid("coefficient position outside the chroma block"),
    }
  }
}

fn coefficient_position_error() -> core::result::Result<Error, Never> {
  Ok(Error::Invalid(
    "coefficient position outside the macroblock".to_owned(),
  ))
}

#[derive(Clone, Copy)]
enum ChromaResidual {
  Dc,
  Ac,
}

fn cbp_luma_context(neighbor: Option<&MbState>, block: u32) -> core::result::Result<u16, Never> {
  let increment = match neighbor {
    Some(state) => match state.kind {
      MbKind::IPcm => 0,
      MbKind::PSkip | MbKind::BSkip => 1,
      MbKind::None
      | MbKind::I4x4
      | MbKind::I8x8
      | MbKind::I16x16
      | MbKind::BDirect16x16
      | MbKind::Inter => u16::from(state.cbp & 1u8.wrapping_shl(block & 3) == 0),
    },
    None => 0,
  };
  Ok(increment)
}

fn cbp_chroma_context(
  neighbor: Option<&MbState>,
  residual: ChromaResidual,
) -> core::result::Result<u16, Never> {
  let increment = match neighbor {
    Some(state) => match state.kind {
      MbKind::IPcm => 1,
      MbKind::PSkip | MbKind::BSkip => 0,
      MbKind::None
      | MbKind::I4x4
      | MbKind::I8x8
      | MbKind::I16x16
      | MbKind::BDirect16x16
      | MbKind::Inter => {
        let chroma = state.cbp.wrapping_shr(4);

        match residual {
          ChromaResidual::Dc => u16::from(chroma != 0),
          ChromaResidual::Ac => u16::from(chroma == 2),
        }
      }
    },
    None => 0,
  };
  Ok(increment)
}

impl SliceDecoder<'_> {
  pub fn decode_cabac(&mut self, rbsp: &[u8]) -> Result<()> {
    let Ok(header_bits) = fixed_index(self.sh.header_bits);
    let init = match self.sh.slice_type {
      SliceType::I | SliceType::Si => 3,
      SliceType::P | SliceType::Sp | SliceType::B => self.sh.cabac_init_idc,
    };
    let Ok(qp) = self.sh.qp(self.pps);
    let mut cabac = Cabac::new(rbsp, BytePosition(header_bits.div_ceil(8)), qp, init)?;
    let count = self.pic.mb_w.wrapping_mul(self.pic.mb_h);
    let first = self.sh.first_mb_in_slice;

    for address in first..count {
      self.start_mb(address)?;
      let syntax = match self.sh.slice_type {
        SliceType::I | SliceType::Si => MacroblockSyntax::Coded,
        SliceType::P | SliceType::Sp | SliceType::B => {
          let Ok(syntax) = self.decode_macroblock_syntax(&mut cabac);
          syntax
        }
      };

      let decoded = match syntax {
        MacroblockSyntax::Skip => self.decode_skip(),
        MacroblockSyntax::Coded => self.decode_mb_cabac(&mut cabac),
      };
      self.finish_mb(decoded)?;
      let Ok(exhaustion) = cabac.exhaustion();
      ensure(
        exhaustion != Exhaustion::Overrun,
        || "CABAC read past end of slice data",
      )?;
      let Ok(terminate) = cabac.decode_terminate();

      match terminate {
        1 => {
          return Ok(());
        }
        _ => {}
      }
    }

    let outside = first.max(count);
    invalid(format!("macroblock address {outside} out of range"))
  }

  fn neighboring_states(&self) -> core::result::Result<[Option<&MbState>; 2], Never> {
    let [left, top, ..] = self.nb;
    Ok([left, top].map(|neighbor| {
      let Ok(state) = self.macroblock(neighbor);
      state
    }))
  }

  fn neighboring_kinds(&self) -> core::result::Result<[Option<MbKind>; 2], Never> {
    let Ok(states) = self.neighboring_states();
    Ok(states.map(|state| state.map(|mb| mb.kind)))
  }

  fn decode_macroblock_syntax(
    &self,
    cabac: &mut Cabac,
  ) -> core::result::Result<MacroblockSyntax, Never> {
    let Ok(kinds) = self.neighboring_kinds();
    let [left, top] = kinds.map(|neighbor| {
      u16::from(matches!(neighbor, Some(kind) if !matches!(kind, MbKind::PSkip | MbKind::BSkip)))
    });
    let base = match self.sh.slice_type {
      SliceType::B => 24u16,
      SliceType::I | SliceType::Si | SliceType::P | SliceType::Sp => 11u16,
    };
    let Ok(skip) = cabac.decode_context(base.wrapping_add(left).wrapping_add(top));
    Ok(match skip {
      1 => MacroblockSyntax::Skip,
      _ => MacroblockSyntax::Coded,
    })
  }

  fn decode_mb_type_i(
    &self,
    cabac: &mut Cabac,
    contexts: IntraContexts,
  ) -> core::result::Result<u32, Never> {
    let base = match contexts {
      IntraContexts::Intra => {
        let Ok(kinds) = self.neighboring_kinds();
        let [left, top] = kinds.map(|neighbor| {
          u16::from(
            matches!(neighbor, Some(kind) if !matches!(kind, MbKind::I4x4 | MbKind::I8x8)),
          )
        });
        3u16.wrapping_add(left).wrapping_add(top)
      }
      IntraContexts::Predictive => 17,
      IntraContexts::Bidirectional => 32,
    };
    let Ok(kind) = cabac.decode_context(base);

    match kind {
      0 => {
        return Ok(0);
      }
      _ => {}
    }

    let Ok(pcm) = cabac.decode_terminate();

    match pcm {
      1 => {
        return Ok(25);
      }
      _ => {}
    }

    let [luma_ctx, chroma_ctx, chroma_second, pred_first, pred_second] = match contexts {
      IntraContexts::Intra => [6, 7, 8, 9, 10],
      IntraContexts::Predictive => [18, 19, 19, 20, 20],
      IntraContexts::Bidirectional => [33, 34, 34, 35, 35],
    };
    let Ok(luma) = cabac.decode_context(luma_ctx);
    let Ok(chroma_base) = cabac.decode_context(chroma_ctx);
    let chroma = match chroma_base {
      0 => 0,
      _ => {
        let Ok(extension) = cabac.decode_context(chroma_second);
        chroma_base.wrapping_add(extension)
      }
    };
    let Ok(pred0) = cabac.decode_context(pred_first);
    let Ok(pred1) = cabac.decode_context(pred_second);
    Ok(
      1u32
        .wrapping_add(pred0.wrapping_mul(2) | pred1)
        .wrapping_add(chroma.wrapping_mul(4))
        .wrapping_add(luma.wrapping_mul(12)),
    )
  }

  fn decode_mb_type_p(
    &self,
    cabac: &mut Cabac,
  ) -> core::result::Result<Option<MbTypeInfo>, Never> {
    let Ok(intra) = cabac.decode_context(14);

    match intra {
      1 => {
        let Ok(kind) = self.decode_mb_type_i(cabac, IntraContexts::Predictive);
        return intra_mb_type(kind);
      }
      _ => {}
    }

    let Ok(secondary) = cabac.decode_context(15);
    let kind = match secondary {
      0 => {
        let Ok(split) = cabac.decode_context(16);
        split.wrapping_mul(3)
      }
      _ => {
        let Ok(vertical) = cabac.decode_context(17);
        2u32.wrapping_sub(vertical)
      }
    };
    p_mb_type(kind)
  }

  fn decode_mb_type_b(
    &self,
    cabac: &mut Cabac,
  ) -> core::result::Result<Option<MbTypeInfo>, Never> {
    let Ok(kinds) = self.neighboring_kinds();
    let [left, top] = kinds.map(|neighbor| {
      u16::from(
        matches!(neighbor, Some(kind) if !matches!(kind, MbKind::BSkip | MbKind::BDirect16x16)),
      )
    });
    let Ok(direct) = cabac.decode_context(27u16.wrapping_add(left).wrapping_add(top));

    match direct {
      0 => {
        return b_mb_type(0);
      }
      _ => {}
    }

    let Ok(split) = cabac.decode_context(30);

    match split {
      0 => {
        let Ok(list) = cabac.decode_context(32);
        return b_mb_type(1u32.wrapping_add(list));
      }
      _ => {}
    }

    let Ok(b2) = cabac.decode_context(31);
    let Ok(b3) = cabac.decode_context(32);
    let Ok(b4) = cabac.decode_context(32);
    let Ok(b5) = cabac.decode_context(32);
    let code = b2.wrapping_shl(3) | b3.wrapping_shl(2) | b4.wrapping_shl(1) | b5;

    match code {
      0b1101 => {
        let Ok(kind) = self.decode_mb_type_i(cabac, IntraContexts::Bidirectional);
        intra_mb_type(kind)
      }
      0b1111 => b_mb_type(22),
      0b1110 => b_mb_type(11),
      0..=7 => b_mb_type(3u32.wrapping_add(code)),
      _ => {
        let Ok(b6) = cabac.decode_context(32);
        b_mb_type(12u32.wrapping_add((code & 7).wrapping_shl(1) | b6))
      }
    }
  }

  fn decode_mb_type(&self, cabac: &mut Cabac) -> Result<MbTypeInfo> {
    let Ok(info) = match self.sh.slice_type {
      SliceType::I | SliceType::Si => {
        let Ok(kind) = self.decode_mb_type_i(cabac, IntraContexts::Intra);
        intra_mb_type(kind)
      }
      SliceType::P | SliceType::Sp => self.decode_mb_type_p(cabac),
      SliceType::B => self.decode_mb_type_b(cabac),
    };

    match info {
      Some(info_2) => Ok(info_2),
      None => invalid("invalid CABAC mb_type"),
    }
  }

  fn b_sub_mb_code(cabac: &mut Cabac) -> core::result::Result<u32, Never> {
    let Ok(direct) = cabac.decode_context(36);

    match direct {
      0 => {
        return Ok(0);
      }
      _ => {}
    }

    let Ok(split) = cabac.decode_context(37);

    match split {
      0 => {
        let Ok(list) = cabac.decode_context(39);
        return Ok(1u32.wrapping_add(list));
      }
      _ => {}
    }

    let Ok(small) = cabac.decode_context(38);

    match small {
      1 => {
        let Ok(square) = cabac.decode_context(39);

        match square {
          1 => {
            let Ok(list) = cabac.decode_context(39);
            return Ok(11u32.wrapping_add(list));
          }
          _ => {}
        }
      }
      _ => {}
    }

    let Ok(b0) = cabac.decode_context(39);
    let Ok(b1) = cabac.decode_context(39);
    let base = 3u32.wrapping_add(small.wrapping_mul(4));
    Ok(base.wrapping_add(b0.wrapping_shl(1) | b1))
  }

  fn p_sub_mb_code(cabac: &mut Cabac) -> core::result::Result<u32, Never> {
    let Ok(square) = cabac.decode_context(21);

    match square {
      1 => {
        return Ok(0);
      }
      _ => {}
    }

    let Ok(split) = cabac.decode_context(22);

    match split {
      0 => {
        return Ok(1);
      }
      _ => {}
    }

    let Ok(vertical) = cabac.decode_context(23);
    Ok(3u32.wrapping_sub(vertical))
  }

  fn decode_sub_mb_type(&self, cabac: &mut Cabac) -> Result<SubMbInfo> {
    let Ok(info) = match self.sh.slice_type {
      SliceType::B => {
        let Ok(code) = Self::b_sub_mb_code(cabac);
        b_sub_mb_type(code)
      }
      SliceType::I | SliceType::Si | SliceType::P | SliceType::Sp => {
        let Ok(code) = Self::p_sub_mb_code(cabac);
        p_sub_mb_type(code)
      }
    };

    match info {
      Some(info_2) => Ok(info_2),
      None => invalid("invalid CABAC sub_mb_type"),
    }
  }

  fn context_block(
    &self,
    plane: CoefficientPlane,
    sample: Point<i8>,
  ) -> core::result::Result<Option<(&MbState, Point<u8>)>, Never> {
    let [left, top, ..] = self.nb;
    let edge = match plane {
      CoefficientPlane::Luma => 3,
      CoefficientPlane::Cb | CoefficientPlane::Cr => 1,
    };
    let (address, x, y) = match (sample.x, sample.y) {
      (..=-1, y) => (left, edge, y),
      (x, ..=-1) => (top, x, edge),
      (x, y) => (Some(self.mb_addr), x, y),
    };
    let Ok(block) = self.macroblock(address);
    let Ok(local_x) = wrapped(x);
    let Ok(local_y) = wrapped(y);
    Ok(block.map(|state| {
      (
        state,
        Point {
          x: local_x,
          y: local_y,
        },
      )
    }))
  }

  fn reference_context(
    &self,
    list: PredictionList,
    point: Point<i8>,
  ) -> core::result::Result<u16, Never> {
    let Ok(block) = self.context_block(CoefficientPlane::Luma, point);
    let (state, position) = match block {
      Some(block_2) => block_2,
      None => return Ok(0),
    };
    let subblock = position.y.wrapping_shr(1).wrapping_mul(2) | position.x.wrapping_shr(1);
    let direct = state.direct8x8 & 1u8.wrapping_shl(u32::from(subblock)) != 0;

    match ::core::primitive::bool::then_some(
      matches!(
        MacroblockPrediction::from(state.kind),
        MacroblockPrediction::Intra
      ) || matches!(state.kind, MbKind::PSkip | MbKind::BSkip)
        || direct,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(0);
      }
      ::core::option::Option::None => {}
    }

    let Ok(references) = list.entry(&state.ref_idx);
    let Ok(slot) = index(subblock);
    Ok(u16::from(
      matches!(references.get(slot), Some(&reference) if reference > 0),
    ))
  }

  fn decode_ref_idx(
    &self,
    cabac: &mut Cabac,
    list: PredictionList,
    point: Point<i8>,
  ) -> core::result::Result<u32, Never> {
    let [left, top] = [
      Point {
        x: point.x.wrapping_sub(1),
        y: point.y,
      },
      Point {
        x: point.x,
        y: point.y.wrapping_sub(1),
      },
    ];
    let Ok(left_ref) = self.reference_context(list, left);
    let Ok(top_ref) = self.reference_context(list, top);
    let Ok(first) = cabac.decode_context(
      54u16
        .wrapping_add(left_ref)
        .wrapping_add(top_ref.wrapping_mul(2)),
    );

    match first {
      0 => {
        return Ok(0);
      }
      _ => {}
    }

    let Ok(second) = cabac.decode_context(58);

    match second {
      0 => {
        return Ok(1);
      }
      _ => {}
    }

    for reference in 2..=32 {
      let Ok(extension) = cabac.decode_context(59);

      match extension {
        0 => {
          return Ok(reference);
        }
        _ => {}
      }
    }

    Ok(33)
  }

  fn neighboring_mvd(
    &self,
    list: PredictionList,
    component: MotionComponent,
    point: Point<i8>,
  ) -> core::result::Result<u16, Never> {
    let Ok(block) = self.context_block(CoefficientPlane::Luma, point);
    let (state, position) = match block {
      Some(block_2) => block_2,
      None => return Ok(0),
    };
    let raster = position.y.wrapping_mul(4) | position.x;
    let Ok(vectors) = list.entry(&state.mvd);
    let Ok(offset) = index(raster);
    let amount = match vectors.get(offset) {
      Some(&[horizontal, vertical]) => match component {
        MotionComponent::Horizontal => horizontal,
        MotionComponent::Vertical => vertical,
      },
      None => 0,
    };
    Ok(u16::from(amount))
  }

  fn decode_mvd_component(
    &self,
    cabac: &mut Cabac,
    list: PredictionList,
    component: MotionComponent,
    point: Point<i8>,
  ) -> core::result::Result<i32, Never> {
    let Ok(left) = self.neighboring_mvd(
      list,
      component,
      Point {
        x: point.x.wrapping_sub(1),
        y: point.y,
      },
    );
    let Ok(top) = self.neighboring_mvd(
      list,
      component,
      Point {
        x: point.x,
        y: point.y.wrapping_sub(1),
      },
    );
    let context = match left.wrapping_add(top) {
      0..=2 => 0,
      3..=32 => 1,
      _ => 2,
    };
    let base = match component {
      MotionComponent::Horizontal => 40u16,
      MotionComponent::Vertical => 47,
    };
    let Ok(first) = cabac.decode_context(base.wrapping_add(context));

    match first {
      0 => {
        return Ok(0);
      }
      _ => {}
    }

    let mut magnitude = 9i32;

    for prefix in 1u16..9 {
      let Ok(extension) = cabac.decode_context(base.wrapping_add(prefix.wrapping_add(2).min(6)));

      match extension {
        0 => {
          magnitude = i32::from(prefix);
          break;
        }
        _ => {}
      }
    }

    match magnitude {
      9 => {
        let Ok(suffix) = decode_bypass_suffix(cabac, 3);
        magnitude = magnitude.wrapping_add(suffix);
      }
      _ => {}
    }

    let Ok(sign) = cabac.decode_bypass();
    Ok(match sign {
      1 => magnitude.wrapping_neg(),
      _ => magnitude,
    })
  }

  fn decode_mvd(
    &self,
    cabac: &mut Cabac,
    list: PredictionList,
    point: Point<i8>,
  ) -> core::result::Result<[i32; 2], Never> {
    let Ok(horizontal) =
      self.decode_mvd_component(cabac, list, MotionComponent::Horizontal, point);
    let Ok(vertical) = self.decode_mvd_component(cabac, list, MotionComponent::Vertical, point);
    Ok([horizontal, vertical])
  }

  fn set_cur_ref(
    &mut self,
    list: PredictionList,
    block: Rectangle<u8>,
    reference: i8,
  ) -> core::result::Result<(), Never> {
    let Ok(state) = self.mb_mut();
    let [first, second] = &mut state.ref_idx;
    let references = match list {
      PredictionList::First => first,
      PredictionList::Second => second,
    };
    let Rectangle { origin, size } = block;

    for y in (origin.y..origin.y.wrapping_add(size.height)).step_by(2) {
      'over_x_2: for x in (origin.x..origin.x.wrapping_add(size.width)).step_by(2) {
        let block_2 = y.wrapping_shr(1).wrapping_mul(2) | x.wrapping_shr(1);
        let Ok(position) = index(block_2);

        match references.get_mut(position) {
          Some(slot) => *slot = reference,
          None => continue 'over_x_2,
        }
      }
    }

    Ok(())
  }

  fn set_cur_mvd(
    &mut self,
    list: PredictionList,
    block: Rectangle<u8>,
    delta: [i32; 2],
  ) -> core::result::Result<(), Never> {
    let magnitude = delta.map(|amount| {
      let Ok(component) = wrapped::<_, u8>(amount.unsigned_abs().min(127));
      component
    });
    let Ok(state) = self.mb_mut();
    let [first, second] = &mut state.mvd;
    let magnitudes = match list {
      PredictionList::First => first,
      PredictionList::Second => second,
    };
    let Rectangle { origin, size } = block;

    for y in origin.y..origin.y.wrapping_add(size.height) {
      'over_x_2: for x in origin.x..origin.x.wrapping_add(size.width) {
        let raster = y.wrapping_mul(4) | x;
        let Ok(position) = index(raster);

        match magnitudes.get_mut(position) {
          Some(slot) => *slot = magnitude,
          None => continue 'over_x_2,
        }
      }
    }

    Ok(())
  }

  fn decode_cbp(&self, cabac: &mut Cabac) -> core::result::Result<u8, Never> {
    let Ok([left, top]) = self.neighboring_states();
    let mut luma = 0u8;

    for block in 0u32..4 {
      let left_code = match block & 1 {
        0 => {
          let Ok(neighbor) = cbp_luma_context(left, block.wrapping_add(1));
          neighbor
        }
        _ => u16::from(luma & 1u8.wrapping_shl(block.wrapping_sub(1)) == 0),
      };
      let top_code = match block & 2 {
        0 => {
          let Ok(neighbor) = cbp_luma_context(top, block.wrapping_add(2));
          neighbor
        }
        _ => u16::from(luma & 1u8.wrapping_shl(block.wrapping_sub(2)) == 0),
      };
      let context = 73u16
        .wrapping_add(left_code)
        .wrapping_add(top_code.wrapping_mul(2));
      let Ok(bin) = cabac.decode_context(context);
      let Ok(bit) = wrapped::<_, u8>(bin);
      luma |= bit.wrapping_shl(block);
    }

    let Ok(chroma) = Self::decode_cbp_chroma(cabac, [left, top]);
    Ok(luma | chroma.wrapping_shl(4))
  }

  fn decode_cbp_chroma(
    cabac: &mut Cabac,
    neighbors: [Option<&MbState>; 2],
  ) -> core::result::Result<u8, Never> {
    let [left, top] = neighbors;
    let Ok(left_dc) = cbp_chroma_context(left, ChromaResidual::Dc);
    let Ok(top_dc) = cbp_chroma_context(top, ChromaResidual::Dc);
    let Ok(coded) = cabac.decode_context(
      77u16
        .wrapping_add(left_dc)
        .wrapping_add(top_dc.wrapping_mul(2)),
    );

    match coded {
      0 => {
        return Ok(0);
      }
      _ => {}
    }

    let Ok(left_ac) = cbp_chroma_context(left, ChromaResidual::Ac);
    let Ok(top_ac) = cbp_chroma_context(top, ChromaResidual::Ac);
    let Ok(ac) = cabac.decode_context(
      81u16
        .wrapping_add(left_ac)
        .wrapping_add(top_ac.wrapping_mul(2)),
    );
    let Ok(extension) = wrapped::<_, u8>(ac);
    Ok(1u8.wrapping_add(extension))
  }

  fn decode_qp_delta(&self, cabac: &mut Cabac) -> Result<i32> {
    let base = 60u16.wrapping_add(u16::from(self.prev_qp_delta_nz));
    let Ok(coded) = cabac.decode_context(base);

    match coded {
      0 => {
        return Ok(0);
      }
      _ => {}
    }

    let mut context = 62;

    for code in 1u16..=128 {
      let Ok(extension) = cabac.decode_context(context);

      match extension {
        0 => {
          let magnitude = i32::from(code.div_ceil(2));
          return Ok(match code & 1 {
            1 => magnitude,
            _ => magnitude.wrapping_neg(),
          });
        }
        _ => {}
      }

      context = 63;
    }

    invalid("mb_qp_delta too large")
  }

  fn coded_dc_context(
    &self,
    bit: u32,
    prediction: MacroblockPrediction,
  ) -> core::result::Result<u16, Never> {
    let Ok(neighbors) = self.neighboring_states();
    let [left, top] = neighbors.map(|neighbor| match neighbor {
      Some(state) => {
        u16::from(state.kind == MbKind::IPcm || state.cbf_dc & 1u8.wrapping_shl(bit) != 0)
      }
      None => u16::from(prediction == MacroblockPrediction::Intra),
    });
    Ok(left.wrapping_add(top.wrapping_mul(2)))
  }

  fn coefficient_context(
    &self,
    plane: CoefficientPlane,
    origin: Point<i8>,
    prediction: MacroblockPrediction,
  ) -> core::result::Result<u16, Never> {
    let [left, top] = [
      Point {
        x: origin.x.wrapping_sub(1),
        y: origin.y,
      },
      Point {
        x: origin.x,
        y: origin.y.wrapping_sub(1),
      },
    ]
    .map(|point| {
      let Ok(block) = self.context_block(plane, point);

      match block {
        Some((state, location)) => {
          let Ok(nonzero) = plane.coefficient_count(state, location);
          nonzero
        }
        None => u16::from(prediction == MacroblockPrediction::Intra),
      }
    });
    Ok(left.wrapping_add(top.wrapping_mul(2)))
  }

  fn residual_luma_dc(
    &mut self,
    cabac: &mut Cabac,
    prediction: MacroblockPrediction,
  ) -> core::result::Result<(), Never> {
    let Ok(context) = self.coded_dc_context(0, prediction);
    let mut levels = [0; 16];
    let Ok(count) = residual_block_cabac(cabac, &mut levels, ResidualCategory::LumaDc, context);

    match count {
      0 => {}
      _ => {
        let Ok(state) = self.mb_mut();
        state.cbf_dc |= 1;
        let Ok(()) = self.put_luma_dc(&levels);
      }
    }

    Ok(())
  }

  fn residual_luma_8x8(&mut self, cabac: &mut Cabac, block: u8) -> Result<()> {
    let mut levels = [0; 64];
    let Ok(_) = residual_block_cabac(cabac, &mut levels, ResidualCategory::Luma8x8, 0);
    let Ok(state) = self.mb_mut();
    let base = (block & 1).wrapping_mul(2) | (block & 2).wrapping_mul(4);

    for offset in [0, 1, 4, 5] {
      let Ok(position) = index(base.wrapping_add(offset));
      let value = state.nnz.get_mut(position).ok_or_else(|| {
        let Ok(error) = coefficient_position_error();
        error
      })?;
      *value = 1;
    }

    let Ok(()) = self.put_luma8(block, &levels);
    Ok(())
  }

  fn residual_luma_4x4(
    &mut self,
    cabac: &mut Cabac,
    block: u8,
    kind: MbKind,
    prediction: MacroblockPrediction,
  ) -> Result<()> {
    let x = (block & 1).wrapping_mul(2);
    let y = block.wrapping_shr(1).wrapping_mul(2);

    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
      let coordinate = Point {
        x: x.wrapping_add(dx),
        y: y.wrapping_add(dy),
      };
      let Ok(point) = motion_point(coordinate);
      let Ok(context) = self.coefficient_context(CoefficientPlane::Luma, point, prediction);
      let mut levels = [0; 16];
      let Ok(count) = match kind {
        MbKind::I16x16 => {
          let [_, ac @ ..] = &mut levels;
          residual_block_cabac(cabac, ac, ResidualCategory::LumaAc, context)
        }
        MbKind::None
        | MbKind::I4x4
        | MbKind::I8x8
        | MbKind::IPcm
        | MbKind::PSkip
        | MbKind::BSkip
        | MbKind::BDirect16x16
        | MbKind::Inter => {
          residual_block_cabac(cabac, &mut levels, ResidualCategory::Luma4x4, context)
        }
      };
      let raster = coordinate.y.wrapping_mul(4) | coordinate.x;
      let Ok(position) = index(raster);
      let Ok(state) = self.mb_mut();
      let slot = state.nnz.get_mut(position).ok_or_else(|| {
        let Ok(error) = coefficient_position_error();
        error
      })?;

      *slot = count;
      match count {
        0 => {}
        _ => {
          let range = CoefficientRange::from(kind == MbKind::I16x16);
          let Ok(()) = self.put_luma4(raster, &levels, range);
        }
      }
    }

    Ok(())
  }

  fn residual_chroma_dc(
    &mut self,
    cabac: &mut Cabac,
    prediction: MacroblockPrediction,
  ) -> core::result::Result<(), Never> {
    for (component, bit) in [(ChromaPlane::Cb, 1), (ChromaPlane::Cr, 2)] {
      let Ok(context) = self.coded_dc_context(bit, prediction);
      let mut levels = [0; 4];
      let Ok(count) =
        residual_block_cabac(cabac, &mut levels, ResidualCategory::ChromaDc, context);

      match count {
        0 => {}
        _ => {
          let Ok(state) = self.mb_mut();
          state.cbf_dc |= 1u8.wrapping_shl(bit);
          let Ok(()) = self.put_chroma_dc(component, &levels);
        }
      }
    }

    Ok(())
  }

  fn residual_chroma_ac(
    &mut self,
    cabac: &mut Cabac,
    prediction: MacroblockPrediction,
  ) -> Result<()> {
    for (component, plane) in [
      (ChromaPlane::Cb, CoefficientPlane::Cb),
      (ChromaPlane::Cr, CoefficientPlane::Cr),
    ] {
      for block in 0u8..4 {
        let location = Point {
          x: block & 1,
          y: block.wrapping_shr(1),
        };
        let Ok(point) = motion_point(location);
        let Ok(context) = self.coefficient_context(plane, point, prediction);
        let mut levels = [0; 16];
        let [_, ac @ ..] = &mut levels;
        let Ok(count) = residual_block_cabac(cabac, ac, ResidualCategory::ChromaAc, context);
        let Ok(state) = self.mb_mut();
        let positions = plane.chroma_counts(state)?;
        let Ok(position) = index(block);
        let slot = positions.get_mut(position).ok_or_else(|| {
          let Ok(error) = coefficient_position_error();
          error
        })?;

        *slot = count;
        match count {
          0 => {}
          _ => {
            let Ok(()) = self.put_chroma_ac(component, block, &levels);
          }
        }
      }
    }

    Ok(())
  }

  fn residual_cabac(&mut self, cabac: &mut Cabac, cbp: u8) -> Result<()> {
    let Ok(state) = self.mb();
    let kind = state.kind;
    let use_8x8 = state.transform_size == TransformSize::EightByEight;
    let prediction = MacroblockPrediction::from(kind);

    match ::core::primitive::bool::then_some(kind == MbKind::I16x16, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.residual_luma_dc(cabac, prediction);
      }
      ::core::option::Option::None => {}
    }

    for block in 0u8..4 {
      match ::core::primitive::bool::then_some(cbp & 1u8.wrapping_shl(u32::from(block)) == 0, ())
      {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(use_8x8, ()) {
        ::core::option::Option::Some(()) => self.residual_luma_8x8(cabac, block)?,
        ::core::option::Option::None => self.residual_luma_4x4(cabac, block, kind, prediction)?,
      }
    }

    let chroma = cbp.wrapping_shr(4);

    match chroma {
      0 => {}
      _ => {
        let Ok(()) = self.residual_chroma_dc(cabac, prediction);
      }
    }

    match chroma {
      2 => {
        self.residual_chroma_ac(cabac, prediction)?;
      }
      _ => {}
    }

    Ok(())
  }

  fn decode_pcm_cabac(&mut self, cabac: &mut Cabac) -> Result<()> {
    let Ok(BitPosition(bit_position)) = cabac.bit_pos();
    let byte_position = bit_position.div_ceil(8);
    let Ok(position) = index(byte_position);
    let Ok(data) = cabac.data();
    let samples = match data
      .get(position..)
      .and_then(|samples| samples.first_chunk::<384>())
    {
      Some(samples) => samples,
      None => return invalid("truncated I_PCM macroblock"),
    };
    let Ok(()) = cabac.set_byte_pos(BytePosition(byte_position.wrapping_add(384)));
    cabac.init_engine()?;
    let Ok(()) = self.finish_pcm();
    let Ok(()) = self.write_pcm(samples);
    self.prev_qp_delta_nz = false;
    Ok(())
  }

  fn decode_intra_modes_cabac(
    &mut self,
    cabac: &mut Cabac,
    size: TransformSize,
  ) -> core::result::Result<(), Never> {
    let count = match size {
      TransformSize::FourByFour => 16u8,
      TransformSize::EightByEight => 4u8,
    };
    let Ok(limit) = index(count);

    for mode in self.cur.rem_mode.iter_mut().take(limit) {
      let Ok(predicted) = cabac.decode_context(68);
      *mode = match predicted {
        1 => -1,
        _ => {
          let Ok(first) = cabac.decode_context(69);
          let Ok(second) = cabac.decode_context(69);
          let Ok(third) = cabac.decode_context(69);
          let Ok(value) = wrapped(first | second.wrapping_shl(1) | third.wrapping_shl(2));
          value
        }
      };
    }

    let Ok(()) = self.derive_intra_modes(size);
    Ok(())
  }

  fn store_ref_cabac(
    &mut self,
    cabac: &mut Cabac,
    list: PredictionList,
    partition: Rectangle<u8>,
    block: u8,
    count: u32,
  ) -> Result<()> {
    let reference = match count {
      0 | 1 => 0,
      _ => {
        let Ok(point) = motion_point(partition.origin);
        let Ok(raw_reference) = self.decode_ref_idx(cabac, list, point);
        let Ok(reference) = wrapped(raw_reference);
        reference
      }
    };
    let Ok(position) = index(block);
    let Ok(references) = list.entry_mut(&mut self.cur.ref_idx);
    let slot = references
      .get_mut(position)
      .ok_or_else(|| Error::Invalid("position outside the motion partition".to_owned()))?;
    *slot = reference;
    let Ok(()) = self.set_cur_ref(list, partition, reference);
    Ok(())
  }

  fn store_mvd_cabac(
    &mut self,
    cabac: &mut Cabac,
    list: PredictionList,
    partition: Rectangle<u8>,
    block: u8,
  ) -> Result<()> {
    let Ok(point) = motion_point(partition.origin);
    let Ok(delta) = self.decode_mvd(cabac, list, point);
    let Ok(()) = self.set_cur_mvd(list, partition, delta);
    let Ok(position) = index(block);
    let Ok(motion_vectors) = list.entry_mut(&mut self.cur.mvd);
    let slot = motion_vectors
      .get_mut(position)
      .ok_or_else(|| Error::Invalid("position outside the motion partition".to_owned()))?;
    *slot = delta.map(|value| {
      let Ok(component) = wrapped(value);
      component
    });
    Ok(())
  }

  fn decode_subpart_refs_cabac(
    &mut self,
    cabac: &mut Cabac,
    info: MbTypeInfo,
    subtypes: [SubMbInfo; 4],
  ) -> Result<()> {
    for (list, mask, reference_count) in [(PredictionList::First, 1), (PredictionList::Second, 2)]
      .into_iter()
      .zip(self.sh.num_ref_idx_active)
      .map(|((list, mask), count)| (list, mask, count))
    {
      let count = match ::core::primitive::bool::then_some(info.ref0, ()) {
        ::core::option::Option::Some(()) => 1,
        ::core::option::Option::None => reference_count,
      };

      'inner_for_2: for (subtype, quadrant) in subtypes.into_iter().zip(0u8..4) {
        match ::core::primitive::bool::then_some(subtype.direct || subtype.pred & mask == 0, ()) {
          ::core::option::Option::Some(()) => {
            continue 'inner_for_2;
          }
          ::core::option::Option::None => {}
        }

        let Ok(rectangle) = Part::P8x8.block(quadrant);
        self.store_ref_cabac(cabac, list, rectangle, quadrant, count)?;
      }
    }

    Ok(())
  }

  fn decode_subparts_cabac(&mut self, cabac: &mut Cabac, info: MbTypeInfo) -> Result<()> {
    let checked_value = self.decode_sub_mb_type(cabac)?;
    let subtypes = [
      checked_value,
      { self.decode_sub_mb_type(cabac)? },
      { self.decode_sub_mb_type(cabac)? },
      { self.decode_sub_mb_type(cabac)? },
    ];
    self.cur.sub = subtypes;
    let mut direct_mask = 0u8;

    for (subtype, bit) in subtypes.into_iter().zip(0u32..4) {
      direct_mask |= u8::from(subtype.direct).wrapping_shl(bit);
    }

    let Ok(state) = self.mb_mut();
    state.direct8x8 = direct_mask;
    self.decode_subpart_refs_cabac(cabac, info, subtypes)?;

    for (list, mask) in [(PredictionList::First, 1), (PredictionList::Second, 2)] {
      'inner_for_2: for (subtype, quadrant) in subtypes.into_iter().zip(0u8..4) {
        match ::core::primitive::bool::then_some(subtype.direct || subtype.pred & mask == 0, ()) {
          ::core::option::Option::Some(()) => {
            continue 'inner_for_2;
          }
          ::core::option::Option::None => {}
        }

        let Ok(parent) = Part::P8x8.block(quadrant);
        let Ok(length) = subtype.shape.block_count();

        for child in 0..length {
          let Ok(relative) = subtype.shape.block(child);
          let origin = Point {
            x: parent.origin.x.wrapping_add(relative.origin.x),
            y: parent.origin.y.wrapping_add(relative.origin.y),
          };
          let partition = Rectangle {
            origin,
            size: relative.size,
          };
          let offset = quadrant.wrapping_mul(4).wrapping_add(child);
          self.store_mvd_cabac(cabac, list, partition, offset)?;
        }
      }
    }

    Ok(())
  }

  fn decode_partitions_cabac(&mut self, cabac: &mut Cabac, info: MbTypeInfo) -> Result<()> {
    let Ok(length) = info.part.block_count();

    for ((list, mask), count) in [(PredictionList::First, 1), (PredictionList::Second, 2)]
      .into_iter()
      .zip(self.sh.num_ref_idx_active)
    {
      'inner_for_2: for (prediction, child) in info.pred.into_iter().zip(0..length) {
        match ::core::primitive::bool::then_some(prediction & mask == 0, ()) {
          ::core::option::Option::Some(()) => {
            continue 'inner_for_2;
          }
          ::core::option::Option::None => {}
        }

        let Ok(partition) = info.part.block(child);
        self.store_ref_cabac(cabac, list, partition, child, count)?;
      }
    }

    for (list, mask) in [(PredictionList::First, 1), (PredictionList::Second, 2)] {
      'inner_for_2: for (prediction, child) in info.pred.into_iter().zip(0..length) {
        match ::core::primitive::bool::then_some(prediction & mask == 0, ()) {
          ::core::option::Option::Some(()) => {
            continue 'inner_for_2;
          }
          ::core::option::Option::None => {}
        }

        let Ok(partition) = info.part.block(child);
        self.store_mvd_cabac(cabac, list, partition, child.wrapping_mul(4))?;
      }
    }

    Ok(())
  }

  fn macroblock_prediction_cabac(&mut self, cabac: &mut Cabac, info: MbTypeInfo) -> Result<()> {
    match info.kind {
      MbKind::I4x4 | MbKind::I8x8 | MbKind::I16x16 => {
        match info.kind {
          MbKind::I4x4 | MbKind::I8x8 => {
            let size = TransformSize::from(info.kind == MbKind::I8x8);
            let Ok(()) = self.decode_intra_modes_cabac(cabac, size);
          }
          MbKind::None
          | MbKind::I16x16
          | MbKind::IPcm
          | MbKind::PSkip
          | MbKind::BSkip
          | MbKind::BDirect16x16
          | MbKind::Inter => {}
        }

        let Ok(mode) = self.decode_chroma_mode(cabac);
        let Ok(state) = self.mb_mut();
        state.intra_chroma_mode = mode;
      }
      MbKind::BDirect16x16 => {
        let Ok(state) = self.mb_mut();
        state.direct8x8 = 15;
      }
      MbKind::None | MbKind::IPcm | MbKind::PSkip | MbKind::BSkip | MbKind::Inter => match info
        .part
      {
        Part::P8x8 => self.decode_subparts_cabac(cabac, info)?,
        Part::P16x16 | Part::P16x8 | Part::P8x16 => self.decode_partitions_cabac(cabac, info)?,
      },
    }

    Ok(())
  }

  fn complete_mb_cabac(&mut self, cabac: &mut Cabac, info: MbTypeInfo, cbp: u8) -> Result<()> {
    let prediction = MacroblockPrediction::from(info.kind);
    self.cur.cbp = cbp;
    let Ok(state) = self.mb_mut();
    state.cbp = cbp;

    match prediction {
      MacroblockPrediction::Intra => {}
      MacroblockPrediction::Inter => {
        let Ok(eligibility) = self.transform_eligibility();
        let eligible = cbp & 15 != 0
          && self.pps.transform_8x8_mode
          && eligibility == TransformEligibility::EightByEight;

        match ::core::primitive::bool::then_some(eligible, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(size) = self.decode_transform_size(cabac);
            let Ok(state_2) = self.mb_mut();
            state_2.transform_size = size;
          }
          ::core::option::Option::None => {}
        }

        self.derive_inter_motion()?;
      }
    }

    self.prev_qp_delta_nz =
      match ::core::primitive::bool::then_some(cbp != 0 || info.kind == MbKind::I16x16, ()) {
        ::core::option::Option::Some(()) => {
          let delta = self.decode_qp_delta(cabac)?;
          self.apply_qp_delta(delta)?;
          delta != 0
        }
        ::core::option::Option::None => false,
      };

    let Ok(()) = self.store_qp();
    self.residual_cabac(cabac, cbp)?;

    match prediction {
      MacroblockPrediction::Intra => {
        let Ok(()) = self.reconstruct_intra();
      }
      MacroblockPrediction::Inter => {
        let Ok(()) = self.predict_inter();
        let Ok(()) = self.reconstruct_inter_residual();
      }
    }

    Ok(())
  }

  fn decode_mb_cabac(&mut self, cabac: &mut Cabac) -> Result<()> {
    let mut info = self.decode_mb_type(cabac)?;
    self.cur = MbCur {
      info,
      ..Default::default()
    };

    match ::core::primitive::bool::then_some(info.kind == MbKind::IPcm, ()) {
      ::core::option::Option::Some(()) => {
        return self.decode_pcm_cabac(cabac);
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(
      info.kind == MbKind::I4x4 && self.pps.transform_8x8_mode,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(size) = self.decode_transform_size(cabac);

        match ::core::primitive::bool::then_some(size == TransformSize::EightByEight, ()) {
          ::core::option::Option::Some(()) => {
            info.kind = MbKind::I8x8;
            self.cur.info.kind = MbKind::I8x8;
            let Ok(state) = self.mb_mut();
            state.transform_size = TransformSize::EightByEight;
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    let Ok(state) = self.mb_mut();
    state.kind = info.kind;
    self.macroblock_prediction_cabac(cabac, info)?;
    let cbp = match info.kind {
      MbKind::I16x16 => info.i16_cbp,
      MbKind::None
      | MbKind::I4x4
      | MbKind::I8x8
      | MbKind::IPcm
      | MbKind::PSkip
      | MbKind::BSkip
      | MbKind::BDirect16x16
      | MbKind::Inter => {
        let Ok(pattern) = self.decode_cbp(cabac);
        pattern
      }
    };
    self.complete_mb_cabac(cabac, info, cbp)
  }

  fn decode_transform_size(
    &self,
    cabac: &mut Cabac,
  ) -> core::result::Result<TransformSize, Never> {
    let Ok(neighbors) = self.neighboring_states();
    let [left, top] = neighbors.map(|state| {
      u16::from(matches!(state, Some(mb) if mb.transform_size == TransformSize::EightByEight))
    });
    let Ok(decision) = cabac.decode_context(399u16.wrapping_add(left).wrapping_add(top));
    Ok(match decision {
      1 => TransformSize::EightByEight,
      _ => TransformSize::FourByFour,
    })
  }

  fn decode_chroma_mode(&self, cabac: &mut Cabac) -> core::result::Result<u8, Never> {
    let Ok(neighbors) = self.neighboring_states();
    let [left, top] = neighbors.map(|state| {
      u16::from(matches!(state, Some(mb)
        if MacroblockPrediction::from(mb.kind) == MacroblockPrediction::Intra
        && mb.kind != MbKind::IPcm && mb.intra_chroma_mode != 0))
    });
    let Ok(first) = cabac.decode_context(64u16.wrapping_add(left).wrapping_add(top));

    match first {
      0 => {
        return Ok(0);
      }
      _ => {}
    }

    let Ok(second) = cabac.decode_context(67);

    match second {
      0 => {
        return Ok(1);
      }
      _ => {}
    }

    let Ok(third) = cabac.decode_context(67);
    let Ok(extension) = wrapped::<_, u8>(third);
    Ok(2u8.wrapping_add(extension))
  }
}

#![forbid(unsafe_code)]

use crate::include::dav1d::headers::Rav1dFilterMode;
use crate::src::align::ArrayDefault;
use crate::src::enum_map::DefaultValue;
use crate::src::enum_map::EnumKey;
use crate::src::in_range::InRange;
use console_core_geometry::Size;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::mem;
use std::ops::Neg;
use strum::EnumCount;
use strum::FromRepr;
use zerocopy::FromBytes;
use zerocopy::FromZeros;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "these are metadata type numbers carried in AV1 OBUs"
  )
)]
pub enum ObuMetaType {
  HdrCll = 1,
  HdrMdcv = 2,
  Scalability = 3,
  ItutT35 = 4,
  Timecode = 5,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, EnumCount, FromRepr, Default, Debug)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the AV1 transform size order indexes the scan, transform and probability tables"
  )
)]
pub enum TxfmSize {
  #[default]
  S4x4 = 0,
  S8x8 = 1,
  S16x16 = 2,
  S32x32 = 3,
  S64x64 = 4,

  R4x8 = 5,
  R8x4 = 6,
  R8x16 = 7,
  R16x8 = 8,
  R16x32 = 9,
  R32x16 = 10,
  R32x64 = 11,
  R64x32 = 12,
  R4x16 = 13,
  R16x4 = 14,
  R8x32 = 15,
  R32x8 = 16,
  R16x64 = 17,
  R64x16 = 18,
}

impl TxfmSize {
  pub const NUM_SQUARE: u8 = {
    let Ok(last) = Self::S64x64.ordinal();
    last.wrapping_add(1)
  };

  #[cfg_attr(
    dylint_lib = "explicit011_no_as_cast",
    expect(
      explicit011_no_as_cast,
      reason = "all transform size discriminants fit in a byte and this is their exact ordinal in the codec tables"
    )
  )]
  pub(crate) const fn ordinal(self) -> Result<u8, console_core_never::Never> {
    Ok(self as u8)
  }
}

impl DefaultValue for TxfmSize {
  const DEFAULT: Self = Self::S4x4;
}

impl ArrayDefault for TxfmSize {
  fn default() -> Self {
    Default::default()
  }
}

pub(crate) enum TransformShape {
  Square,
  Rectangular,
}

impl TxfmSize {
  #[must_use]
  pub(crate) const fn shape(self) -> Result<TransformShape, console_core_never::Never> {
    Ok(match self {
      Self::S4x4 | Self::S8x8 | Self::S16x16 | Self::S32x32 | Self::S64x64 => {
        TransformShape::Square
      }
      Self::R4x8
      | Self::R8x4
      | Self::R8x16
      | Self::R16x8
      | Self::R16x32
      | Self::R32x16
      | Self::R32x64
      | Self::R64x32
      | Self::R4x16
      | Self::R16x4
      | Self::R8x32
      | Self::R32x8
      | Self::R16x64
      | Self::R64x16 => TransformShape::Rectangular,
    })
  }

  pub(crate) const fn extent(self) -> Result<Size<u32>, console_core_never::Never> {
    use TxfmSize::*;

    let (width, height) = match self {
      S4x4 => (4, 4),
      S8x8 => (8, 8),
      S16x16 => (16, 16),
      S32x32 => (32, 32),
      S64x64 => (64, 64),
      R4x8 => (4, 8),
      R8x4 => (8, 4),
      R8x16 => (8, 16),
      R16x8 => (16, 8),
      R16x32 => (16, 32),
      R32x16 => (32, 16),
      R32x64 => (32, 64),
      R64x32 => (64, 32),
      R4x16 => (4, 16),
      R16x4 => (16, 4),
      R8x32 => (8, 32),
      R32x8 => (32, 8),
      R16x64 => (16, 64),
      R64x16 => (64, 16),
    };
    Ok(Size { width, height })
  }
}

#[repr(u8)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, EnumCount)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the subdivision levels index the block partition and dimension tables in descending size order"
  )
)]
pub enum BlockLevel {
  #[default]
  Bl128x128 = 0,
  Bl64x64 = 1,
  Bl32x32 = 2,
  Bl16x16 = 3,
  Bl8x8 = 4,
}

impl BlockLevel {
  pub(crate) const fn ordinal(self) -> Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::Bl128x128 => 0,
      Self::Bl64x64 => 1,
      Self::Bl32x32 => 2,
      Self::Bl16x16 => 3,
      Self::Bl8x8 => 4,
    })
  }

  pub const fn decrease(self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok({
      match self {
        BlockLevel::Bl8x8 => None,
        BlockLevel::Bl16x16 => Some(BlockLevel::Bl8x8),
        BlockLevel::Bl32x32 => Some(BlockLevel::Bl16x16),
        BlockLevel::Bl64x64 => Some(BlockLevel::Bl32x32),
        BlockLevel::Bl128x128 => Some(BlockLevel::Bl64x64),
      }
    })
  }
}

pub type TxfmType = u8;
pub const N_TX_TYPES_PLUS_LL: u8 = 17;
pub const WHT_WHT: TxfmType = 16;
pub const H_FLIPADST: TxfmType = 15;
pub const V_FLIPADST: TxfmType = 14;
pub const H_ADST: TxfmType = 13;
pub const V_ADST: TxfmType = 12;
pub const H_DCT: TxfmType = 11;
pub const V_DCT: TxfmType = 10;
pub const IDTX: TxfmType = 9;
pub const FLIPADST_ADST: TxfmType = 8;
pub const ADST_FLIPADST: TxfmType = 7;
pub const FLIPADST_FLIPADST: TxfmType = 6;
pub const DCT_FLIPADST: TxfmType = 5;
pub const FLIPADST_DCT: TxfmType = 4;
pub const ADST_ADST: TxfmType = 3;
pub const DCT_ADST: TxfmType = 2;
pub const ADST_DCT: TxfmType = 1;
pub const DCT_DCT: TxfmType = 0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, FromRepr)]
#[repr(u8)]
pub enum TxClass {
  TwoD,
  H,
  V,
}

impl TxClass {
  pub(crate) const fn ordinal(self) -> Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::TwoD => 0,
      Self::H => 1,
      Self::V => 2,
    })
  }
}

pub type IntraPredMode = u8;
pub const FILTER_PRED: IntraPredMode = 13;
pub const Z3_PRED: IntraPredMode = 8;
pub const Z2_PRED: IntraPredMode = 7;
pub const Z1_PRED: IntraPredMode = 6;
pub const DC_128_PRED: IntraPredMode = 5;
pub const TOP_DC_PRED: IntraPredMode = 4;
pub const LEFT_DC_PRED: IntraPredMode = 3;
pub const N_IMPL_INTRA_PRED_MODES: u8 = 14;
pub const N_UV_INTRA_PRED_MODES: u8 = 14;
pub const CFL_PRED: IntraPredMode = 13;
pub const N_INTRA_PRED_MODES: u8 = 13;
pub const PAETH_PRED: IntraPredMode = 12;
pub const SMOOTH_H_PRED: IntraPredMode = 11;
pub const SMOOTH_V_PRED: IntraPredMode = 10;
pub const SMOOTH_PRED: IntraPredMode = 9;
pub const VERT_LEFT_PRED: IntraPredMode = 8;
pub const HOR_UP_PRED: IntraPredMode = 7;
pub const HOR_DOWN_PRED: IntraPredMode = 6;
pub const VERT_RIGHT_PRED: IntraPredMode = 5;
pub const DIAG_DOWN_RIGHT_PRED: IntraPredMode = 4;
pub const DIAG_DOWN_LEFT_PRED: IntraPredMode = 3;
pub const HOR_PRED: IntraPredMode = 2;
pub const VERT_PRED: IntraPredMode = 1;
pub const DC_PRED: IntraPredMode = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr, EnumCount, Default)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the entropy decoder maps the coded AV1 inter-intra symbol directly to this mode"
  )
)]
pub enum InterIntraPredMode {
  #[default]
  Dc = 0,
  Vert = 1,
  Hor = 2,
  Smooth = 3,
}

impl InterIntraPredMode {
  pub(crate) const fn ordinal(self) -> Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::Dc => 0,
      Self::Vert => 1,
      Self::Hor => 2,
      Self::Smooth => 3,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, FromRepr, EnumCount)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the coded AV1 partition symbol selects this case and indexes the partition probability tables"
  )
)]
pub enum BlockPartition {
  #[default]
  None = 0,
  H = 1,
  V = 2,
  Split = 3,
  TopSplit = 4,
  BottomSplit = 5,
  LeftSplit = 6,
  RightSplit = 7,
  H4 = 8,
  V4 = 9,
}

impl BlockPartition {
  pub(crate) const fn ordinal(self) -> Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::None => 0,
      Self::H => 1,
      Self::V => 2,
      Self::Split => 3,
      Self::TopSplit => 4,
      Self::BottomSplit => 5,
      Self::LeftSplit => 6,
      Self::RightSplit => 7,
      Self::H4 => 8,
      Self::V4 => 9,
    })
  }

  pub const N_SUB8X8_PARTITIONS: u8 = 4;
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, FromRepr, EnumCount, FromZeros)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the coded block size numbering indexes the AV1 dimension, wedge and context tables"
  )
)]
pub enum BlockSize {
  Bs128x128 = 0,
  Bs128x64 = 1,
  Bs64x128 = 2,
  Bs64x64 = 3,
  Bs64x32 = 4,
  Bs64x16 = 5,
  Bs32x64 = 6,
  Bs32x32 = 7,
  Bs32x16 = 8,
  Bs32x8 = 9,
  Bs16x64 = 10,
  Bs16x32 = 11,
  Bs16x16 = 12,
  Bs16x8 = 13,
  Bs16x4 = 14,
  Bs8x32 = 15,
  Bs8x16 = 16,
  Bs8x8 = 17,
  Bs8x4 = 18,
  Bs4x16 = 19,
  Bs4x8 = 20,
  Bs4x4 = 21,
}

impl BlockSize {
  #[cfg_attr(
    dylint_lib = "explicit011_no_as_cast",
    expect(
      explicit011_no_as_cast,
      reason = "the block size is a repr(u8) enum with the exact ordinals required by the codec tables"
    )
  )]
  pub(crate) const fn ordinal(self) -> Result<u8, console_core_never::Never> {
    Ok(self as u8)
  }
}

#[derive(Clone, Copy, PartialEq, Eq, EnumCount, Default, FromRepr)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the filter pair numbering is shared by the AV1 interpolation mapping and dense DSP dispatch tables"
  )
)]
pub enum Filter2d {
  #[default]
  Regular8Tap = 0,
  RegularSmooth8Tap = 1,
  RegularSharp8Tap = 2,
  SharpRegular8Tap = 3,
  SharpSmooth8Tap = 4,
  Sharp8Tap = 5,
  SmoothRegular8Tap = 6,
  Smooth8Tap = 7,
  SmoothSharp8Tap = 8,
  Bilinear = 9,
}

impl EnumKey<{ Self::COUNT }> for Filter2d {
  fn ordinal(self) -> u8 {
    match self {
      Self::Regular8Tap => 0,
      Self::RegularSmooth8Tap => 1,
      Self::RegularSharp8Tap => 2,
      Self::SharpRegular8Tap => 3,
      Self::SharpSmooth8Tap => 4,
      Self::Sharp8Tap => 5,
      Self::SmoothRegular8Tap => 6,
      Self::Smooth8Tap => 7,
      Self::SmoothSharp8Tap => 8,
      Self::Bilinear => 9,
    }
  }
}

impl Filter2d {
  pub const fn h(&self) -> core::result::Result<Rav1dFilterMode, console_core_never::Never> {
    Ok({
      use Filter2d::*;

      match *self {
        Regular8Tap | RegularSmooth8Tap | RegularSharp8Tap => Rav1dFilterMode::Regular8Tap,
        SharpRegular8Tap | SharpSmooth8Tap | Sharp8Tap => Rav1dFilterMode::Sharp8Tap,
        SmoothRegular8Tap | Smooth8Tap | SmoothSharp8Tap => Rav1dFilterMode::Smooth8Tap,
        Bilinear => Rav1dFilterMode::Bilinear,
      }
    })
  }

  pub const fn v(&self) -> core::result::Result<Rav1dFilterMode, console_core_never::Never> {
    Ok({
      use Filter2d::*;

      match *self {
        Regular8Tap | SharpRegular8Tap | SmoothRegular8Tap => Rav1dFilterMode::Regular8Tap,
        RegularSharp8Tap | Sharp8Tap | SmoothSharp8Tap => Rav1dFilterMode::Sharp8Tap,
        RegularSmooth8Tap | SharpSmooth8Tap | Smooth8Tap => Rav1dFilterMode::Smooth8Tap,
        Bilinear => Rav1dFilterMode::Bilinear,
      }
    })
  }

  pub const fn hv(
    &self,
  ) -> core::result::Result<(Rav1dFilterMode, Rav1dFilterMode), console_core_never::Never> {
    Ok({
      (
        ({
          let Ok(explicit_value) = self.h();
          explicit_value
        }),
        ({
          let Ok(explicit_value) = self.v();
          explicit_value
        }),
      )
    })
  }
}

pub type InterPredMode = u8;
pub const NEWMV: InterPredMode = 3;
pub const GLOBALMV: InterPredMode = 2;
pub const NEARMV: InterPredMode = 1;
pub const NEARESTMV: InterPredMode = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum DrlProximity {
  #[default]
  Nearest,
  Nearer,
  Near,
  Nearish,
}

impl DrlProximity {
  pub(crate) fn ordinal(self) -> Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::Nearest => 0,
      Self::Nearer => 1,
      Self::Near => 2,
      Self::Nearish => 3,
    })
  }
}

pub type CompInterPredMode = u8;
pub const N_COMP_INTER_PRED_MODES: u8 = 8;
pub const NEWMV_NEWMV: CompInterPredMode = 7;
pub const GLOBALMV_GLOBALMV: CompInterPredMode = 6;
pub const NEWMV_NEARMV: CompInterPredMode = 5;
pub const NEARMV_NEWMV: CompInterPredMode = 4;
pub const NEWMV_NEARESTMV: CompInterPredMode = 3;
pub const NEARESTMV_NEWMV: CompInterPredMode = 2;
pub const NEARMV_NEARMV: CompInterPredMode = 1;
pub const NEARESTMV_NEARESTMV: CompInterPredMode = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompInterType {
  WeightedAvg = 0b00011,
  Avg = 0b00101,
  Seg = 0b00110,
  Wedge = 0b01001,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterIntraType {
  Blend,
  Wedge,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct Mv {
  pub y: i16,
  pub x: i16,
}

pub(crate) enum MotionVectorValidity {
  Valid,
  Invalid,
}

impl Mv {
  pub const ZERO: Self = Self { y: 0, x: 0 };

  pub const INVALID: Self = Self {
    y: i16::MIN,
    x: i16::MIN,
  };

  pub(crate) fn validity(self) -> Result<MotionVectorValidity, console_core_never::Never> {
    Ok(match self {
      Self::INVALID => MotionVectorValidity::Invalid,
      _ => MotionVectorValidity::Valid,
    })
  }
}

impl Neg for Mv {
  type Output = Self;

  fn neg(self) -> Self::Output {
    Self {
      y: (self.y).wrapping_neg(),
      x: (self.x).wrapping_neg(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr, Default)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the coded AV1 motion mode numbers select translation, overlapped motion compensation and warp"
  )
)]
pub enum MotionMode {
  #[default]
  Translation = 0,
  Obmc = 1,
  Warp = 2,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Av1BlockIntra {
  pub y_mode: u8,
  pub uv_mode: u8,
  pub tx: TxfmSize,
  pub pal_sz: [u8; 2],
  pub y_angle: i8,
  pub uv_angle: i8,
  pub cfl_alpha: [i8; 2],
}

#[derive(Clone, Copy, PartialEq, Eq, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(transparent)]
pub struct MaskedInterIntraPredMode(u8);

impl MaskedInterIntraPredMode {
  pub const fn get(self) -> core::result::Result<InterIntraPredMode, console_core_never::Never> {
    Ok(match self.0 & 0b11 {
      0 => InterIntraPredMode::Dc,
      1 => InterIntraPredMode::Vert,
      2 => InterIntraPredMode::Hor,
      _ => InterIntraPredMode::Smooth,
    })
  }

  pub const fn new(
    this: InterIntraPredMode,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self(match this {
      InterIntraPredMode::Dc => 0,
      InterIntraPredMode::Vert => 1,
      InterIntraPredMode::Hor => 2,
      InterIntraPredMode::Smooth => 3,
    }))
  }
}

impl Default for MaskedInterIntraPredMode {
  fn default() -> Self {
    let Ok(masked_inter_intra_pred_mode) = Self::new(Default::default());

    masked_inter_intra_pred_mode
  }
}

impl From<InterIntraPredMode> for MaskedInterIntraPredMode {
  fn from(value: InterIntraPredMode) -> Self {
    let Ok(masked_inter_intra_pred_mode) = MaskedInterIntraPredMode::new(value);

    masked_inter_intra_pred_mode
  }
}

impl From<MaskedInterIntraPredMode> for InterIntraPredMode {
  fn from(value: MaskedInterIntraPredMode) -> Self {
    {
      let Ok(explicit_value) = value.get();
      explicit_value
    }
  }
}

#[derive(Clone, Copy, Default, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct Av1BlockInter1d {
  pub mv: [Mv; 2],
  pub wedge_idx: u8,

  pub mask_sign: u8,

  pub interintra_mode: MaskedInterIntraPredMode,

  pub _padding: u8,
}

#[derive(Clone, Copy)]
pub(crate) enum MaskPolarity {
  Normal,
  Inverted,
}

impl From<MaskPolarity> for u8 {
  fn from(polarity: MaskPolarity) -> Self {
    match polarity {
      MaskPolarity::Normal => 0,
      MaskPolarity::Inverted => 1,
    }
  }
}

impl From<MaskPolarity> for i32 {
  fn from(polarity: MaskPolarity) -> Self {
    i32::from(u8::from(polarity))
  }
}

impl Av1BlockInter1d {
  #[must_use]
  pub(crate) fn mask_polarity(&self) -> Result<MaskPolarity, console_core_never::Never> {
    Ok(match self.mask_sign {
      0 => MaskPolarity::Normal,
      _ => MaskPolarity::Inverted,
    })
  }
}

#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct Av1BlockInter2d {
  pub mv2d: Mv,

  matrix: [i16; 4],
}

pub(crate) enum BlockWarp {
  Identity,
  Affine([i16; 4]),
}

impl Av1BlockInter2d {
  #[doc = "Preserve the packed layout shared by the one- and two-dimensional inter blocks."]
  pub(crate) fn from_warp(mv2d: Mv, warp: BlockWarp) -> Result<Self, console_core_never::Never> {
    let matrix = match warp {
      BlockWarp::Identity => [i16::MIN, 0, 0, 0],
      BlockWarp::Affine(coefficients) => coefficients,
    };
    Ok(Self { mv2d, matrix })
  }

  pub(crate) fn warp(&self) -> Result<BlockWarp, console_core_never::Never> {
    Ok(match self.matrix {
      [i16::MIN, ..] => BlockWarp::Identity,
      coefficients => BlockWarp::Affine(coefficients),
    })
  }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Av1BlockInterNd {
  pub one_d: Av1BlockInter1d,
}

impl Av1BlockInterNd {
  pub fn two_d(&self) -> core::result::Result<&Av1BlockInter2d, console_core_never::Never> {
    Ok({
      const _: () =
        assert!(mem::size_of::<Av1BlockInter1d>() == mem::size_of::<Av1BlockInter2d>());
      const _: () =
        assert!(mem::align_of::<Av1BlockInter1d>() == mem::align_of::<Av1BlockInter2d>());

      zerocopy::transmute_ref!(&self.one_d)
    })
  }
}

impl From<Av1BlockInter1d> for Av1BlockInterNd {
  fn from(one_d: Av1BlockInter1d) -> Self {
    Self { one_d }
  }
}

impl From<Av1BlockInter2d> for Av1BlockInterNd {
  fn from(two_d: Av1BlockInter2d) -> Self {
    let one_d = zerocopy::transmute!(two_d);
    Self { one_d }
  }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Av1BlockInter {
  pub nd: Av1BlockInterNd,
  pub comp_type: Option<CompInterType>,
  pub inter_mode: u8,
  pub motion_mode: MotionMode,
  pub drl_idx: DrlProximity,
  pub r#ref: [i8; 2],
  pub max_ytx: TxfmSize,
  pub filter2d: Filter2d,
  pub interintra_type: Option<InterIntraType>,
  pub tx_split0: u8,
  pub tx_split1: u16,
}

#[derive(Clone, Copy)]
pub enum Av1BlockIntraInter {
  Intra(Av1BlockIntra),
  Inter(Av1BlockInter),
}

impl Av1BlockIntraInter {
  pub fn filter2d(&self) -> core::result::Result<Filter2d, console_core_never::Never> {
    Ok(match self {
      Self::Inter(inter) => inter.filter2d,
      Self::Intra(_intra) => Filter2d::default(),
    })
  }
}

impl Default for Av1BlockIntraInter {
  fn default() -> Self {
    Self::Intra(Default::default())
  }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct SegmentId {
  id: InRange<
    u8,
    0,
    {
      ::console_core_number_conversion::const_wrapped_integer!(unsigned u8 => u128, Self::COUNT)
        - 1
    },
  >,
}

impl SegmentId {
  pub const COUNT: u8 = 8;

  pub fn new(id: u8) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(validated) = InRange::new(id);

    Ok(validated.map(|code| Self { id: code }))
  }

  pub fn get(&self) -> Result<u8, console_core_never::Never> {
    self.id.get()
  }

  pub fn min() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(id) = InRange::constant(0);
    Ok(Self { id })
  }

  pub fn max() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(id) = InRange::constant(Self::COUNT.wrapping_sub(1));
    Ok(Self { id })
  }
}

impl Display for SegmentId {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    write!(f, "{}", self.id)
  }
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Av1Block {
  pub bl: BlockLevel,
  pub bs: u8,
  pub bp: BlockPartition,
  pub seg_id: SegmentId,
  pub skip_mode: u8,
  pub skip: u8,
  pub uvtx: TxfmSize,
  pub ii: Av1BlockIntraInter,
}

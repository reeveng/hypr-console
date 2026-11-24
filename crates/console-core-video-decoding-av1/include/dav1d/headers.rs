use crate::src::align::ArrayDefault;
use crate::src::enum_map::EnumKey;
use crate::src::error::{Rav1dError, Rav1dResult};
use crate::src::levels::SegmentId;
use crate::src::relaxed_atomic::RelaxedAtomic;
use console_core_never::Never;
use console_core_synchronization::nonpoison::unpoison;
use std::ffi::c_int;
use std::ffi::c_uint;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::ops::BitAnd;
use std::ops::Deref;
use std::ops::Sub;
use std::sync::{Arc, Mutex};
use strum::EnumCount;
use strum::FromRepr;

#[doc = "A Rust value and its C ABI representation stored together for the lifetime of the decoder's reference."]
pub struct DRav1d<R, D> {
  pub rav1d: R,
  pub dav1d: D,
}

impl<R, D> DRav1d<R, D>
where
  R: Clone + Into<D>,
{
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public bridge retains its infallible value conversion and both owned representations"
    )
  )]
  pub fn from_rav1d(rav1d: R) -> Self {
    let dav1d = <R as Into<D>>::into(rav1d.clone());
    Self { rav1d, dav1d }
  }
}

impl<R, D> Deref for DRav1d<R, D> {
  type Target = R;

  fn deref(&self) -> &Self::Target {
    &self.rav1d
  }
}

impl<R, D> Default for DRav1d<R, D>
where
  R: Default,
  D: Default,
{
  fn default() -> Self {
    Self {
      rav1d: Default::default(),
      dav1d: Default::default(),
    }
  }
}

pub const DAV1D_MAX_CDEF_STRENGTHS: ::console_core_api_compatibility::NativeSize = 8;
pub const DAV1D_MAX_OPERATING_POINTS: ::console_core_api_compatibility::NativeSize = 32;
pub const DAV1D_MAX_TILE_COLS: ::console_core_api_compatibility::NativeSize = 64;
pub const DAV1D_MAX_TILE_ROWS: ::console_core_api_compatibility::NativeSize = 64;
pub const DAV1D_MAX_SEGMENTS: u8 = 8;
const _: () = {
  assert!(SegmentId::COUNT == 8);
};
pub const DAV1D_NUM_REF_FRAMES: ::console_core_api_compatibility::NativeSize = 8;
pub const DAV1D_PRIMARY_REF_NONE: u8 = 7;
pub const DAV1D_REFS_PER_FRAME: ::console_core_api_compatibility::NativeSize = 7;
pub const DAV1D_TOTAL_REFS_PER_FRAME: ::console_core_api_compatibility::NativeSize =
  DAV1D_REFS_PER_FRAME.wrapping_add(1);

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "these constants are the native array lengths for the C-compatible header and decoder tables"
  )
)]
mod table_dimensions {
  use super::*;
  pub(crate) const RAV1D_MAX_CDEF_STRENGTHS: usize = DAV1D_MAX_CDEF_STRENGTHS;
  pub(crate) const RAV1D_MAX_OPERATING_POINTS: usize = DAV1D_MAX_OPERATING_POINTS;
  pub(crate) const RAV1D_MAX_TILE_COLS: usize = DAV1D_MAX_TILE_COLS;
  pub(crate) const RAV1D_MAX_TILE_ROWS: usize = DAV1D_MAX_TILE_ROWS;
  pub(crate) const RAV1D_PRIMARY_REF_NONE: u8 = DAV1D_PRIMARY_REF_NONE;
  pub(crate) const RAV1D_REFS_PER_FRAME: usize = DAV1D_REFS_PER_FRAME;
  pub(crate) const RAV1D_TOTAL_REFS_PER_FRAME: usize = DAV1D_TOTAL_REFS_PER_FRAME;
}

pub(crate) use table_dimensions::{
  RAV1D_MAX_CDEF_STRENGTHS, RAV1D_MAX_OPERATING_POINTS, RAV1D_MAX_TILE_COLS, RAV1D_MAX_TILE_ROWS,
  RAV1D_PRIMARY_REF_NONE, RAV1D_REFS_PER_FRAME, RAV1D_TOTAL_REFS_PER_FRAME,
};

pub type Dav1dObuType = c_uint;
pub const DAV1D_OBU_PADDING: Dav1dObuType = 15;
pub const DAV1D_OBU_REDUNDANT_FRAME_HDR: Dav1dObuType = 7;
pub const DAV1D_OBU_FRAME: Dav1dObuType = 6;
pub const DAV1D_OBU_METADATA: Dav1dObuType = 5;
pub const DAV1D_OBU_TILE_GRP: Dav1dObuType = 4;
pub const DAV1D_OBU_FRAME_HDR: Dav1dObuType = 3;
pub const DAV1D_OBU_TD: Dav1dObuType = 2;
pub const DAV1D_OBU_SEQ_HDR: Dav1dObuType = 1;

#[derive(Clone, Copy, PartialEq, Eq, FromRepr)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the numeric discriminants are the AV1 bitstream OBU type codes"
  )
)]
pub enum Rav1dObuType {
  SeqHdr = 1,
  Td = 2,
  FrameHdr = 3,
  TileGrp = 4,
  Metadata = 5,
  Frame = 6,
  RedundantFrameHdr = 7,
  Padding = 15,
}

pub type Dav1dTxfmMode = c_uint;
pub const DAV1D_N_TX_MODES: ::console_core_api_compatibility::NativeSize = Rav1dTxfmMode::COUNT;
pub const DAV1D_TX_4X4_ONLY: Dav1dTxfmMode = 0;
pub const DAV1D_TX_LARGEST: Dav1dTxfmMode = 1;
pub const DAV1D_TX_SWITCHABLE: Dav1dTxfmMode = 2;

#[derive(Clone, Copy, PartialEq, Eq, FromRepr, EnumCount, Default)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the numeric discriminants are the AV1 transform mode codes used in both the decoder and its C ABI"
  )
)]
pub enum Rav1dTxfmMode {
  #[default]
  Only4x4 = 0,
  Largest = 1,
  Switchable = 2,
}

impl From<Rav1dTxfmMode> for Dav1dTxfmMode {
  fn from(value: Rav1dTxfmMode) -> Self {
    match value {
      Rav1dTxfmMode::Only4x4 => 0,
      Rav1dTxfmMode::Largest => 1,
      Rav1dTxfmMode::Switchable => 2,
    }
  }
}

impl TryFrom<Dav1dTxfmMode> for Rav1dTxfmMode {
  type Error = ();

  fn try_from(value: Dav1dTxfmMode) -> Result<Self, Self::Error> {
    Self::from_repr({
      let ::core::result::Result::Ok(explicit_position) =
        ::console_core_number_conversion::wrapped_index::<::core::primitive::u32>(value);
      explicit_position
    })
    .ok_or(())
  }
}

pub type Dav1dFilterMode = u8;
pub const DAV1D_N_SWITCHABLE_FILTERS: ::console_core_api_compatibility::NativeSize = 3;
pub const DAV1D_N_FILTERS: ::console_core_api_compatibility::NativeSize =
  Rav1dFilterMode::N_FILTERS;
pub const DAV1D_FILTER_SWITCHABLE: Dav1dFilterMode = 4;
pub const DAV1D_FILTER_BILINEAR: Dav1dFilterMode = 3;
pub const DAV1D_FILTER_8TAP_SHARP: Dav1dFilterMode = 2;
pub const DAV1D_FILTER_8TAP_SMOOTH: Dav1dFilterMode = 1;
pub const DAV1D_FILTER_8TAP_REGULAR: Dav1dFilterMode = 0;

#[derive(Clone, Copy, PartialEq, Eq, FromRepr, Default, Debug)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the filter numbers are used by the bitstream and the established DSP filter entries"
  )
)]
pub enum Rav1dFilterMode {
  #[default]
  Regular8Tap = 0,
  Smooth8Tap = 1,
  Sharp8Tap = 2,
  Bilinear = 3,
  Switchable = 4,
}

impl ArrayDefault for Rav1dFilterMode {
  fn default() -> Self {
    Default::default()
  }
}

impl Rav1dFilterMode {
  pub const N_FILTERS: ::console_core_api_compatibility::NativeSize = 4;
  pub const N_SWITCHABLE_FILTERS: Self = Self::Bilinear;
}

impl From<Rav1dFilterMode> for Dav1dFilterMode {
  fn from(value: Rav1dFilterMode) -> Self {
    match value {
      Rav1dFilterMode::Regular8Tap => 0,
      Rav1dFilterMode::Smooth8Tap => 1,
      Rav1dFilterMode::Sharp8Tap => 2,
      Rav1dFilterMode::Bilinear => 3,
      Rav1dFilterMode::Switchable => 4,
    }
  }
}

impl TryFrom<Dav1dFilterMode> for Rav1dFilterMode {
  type Error = ();

  fn try_from(value: Dav1dFilterMode) -> Result<Self, Self::Error> {
    Self::from_repr(
      match ::console_core_number_conversion::index::<::core::primitive::u8>(value) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      },
    )
    .ok_or(())
  }
}

pub type Dav1dAdaptiveBoolean = c_uint;
pub const DAV1D_OFF: Dav1dAdaptiveBoolean = 0;
pub const DAV1D_ON: Dav1dAdaptiveBoolean = 1;
pub const DAV1D_ADAPTIVE: Dav1dAdaptiveBoolean = 2;

#[derive(Clone, Copy, PartialEq, Eq, FromRepr)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the C boundary and coded sequence header use these three numeric choices"
  )
)]
pub enum Rav1dAdaptiveBoolean {
  Off = 0,
  On = 1,
  Adaptive = 2,
}

impl From<bool> for Rav1dAdaptiveBoolean {
  fn from(value: bool) -> Self {
    match ::core::primitive::bool::then_some(value, ()) {
      ::core::option::Option::Some(()) => Self::On,
      ::core::option::Option::None => Self::Off,
    }
  }
}

impl From<Rav1dAdaptiveBoolean> for Dav1dAdaptiveBoolean {
  fn from(value: Rav1dAdaptiveBoolean) -> Self {
    match value {
      Rav1dAdaptiveBoolean::Off => 0,
      Rav1dAdaptiveBoolean::On => 1,
      Rav1dAdaptiveBoolean::Adaptive => 2,
    }
  }
}

impl TryFrom<Dav1dAdaptiveBoolean> for Rav1dAdaptiveBoolean {
  type Error = ();

  fn try_from(value: Dav1dAdaptiveBoolean) -> Result<Self, Self::Error> {
    Self::from_repr({
      let ::core::result::Result::Ok(explicit_position) =
        ::console_core_number_conversion::wrapped_index::<::core::primitive::u32>(value);
      explicit_position
    })
    .ok_or(())
  }
}

pub type Dav1dRestorationType = u8;
pub const DAV1D_RESTORATION_NONE: Dav1dRestorationType = Rav1dRestorationType::None.to_repr();
pub const DAV1D_RESTORATION_SWITCHABLE: Dav1dRestorationType =
  Rav1dRestorationType::Switchable.to_repr();
pub const DAV1D_RESTORATION_WIENER: Dav1dRestorationType = Rav1dRestorationType::Wiener.to_repr();
pub const DAV1D_RESTORATION_SGRPROJ: Dav1dRestorationType =
  Rav1dRestorationType::SgrProj(SgrIdx::I0).to_repr();

#[derive(Clone, Copy, PartialEq, Eq, FromRepr)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the AV1 self-guided restoration parameter index selects a row of the codec table"
  )
)]
pub enum SgrIdx {
  I0 = 0,
  I1 = 1,
  I2 = 2,
  I3 = 3,
  I4 = 4,
  I5 = 5,
  I6 = 6,
  I7 = 7,
  I8 = 8,
  I9 = 9,
  I10 = 10,
  I11 = 11,
  I12 = 12,
  I13 = 13,
  I14 = 14,
  I15 = 15,
}

impl SgrIdx {
  const fn code(self) -> Result<u8, Never> {
    Ok(match self {
      Self::I0 => 0,
      Self::I1 => 1,
      Self::I2 => 2,
      Self::I3 => 3,
      Self::I4 => 4,
      Self::I5 => 5,
      Self::I6 => 6,
      Self::I7 => 7,
      Self::I8 => 8,
      Self::I9 => 9,
      Self::I10 => 10,
      Self::I11 => 11,
      Self::I12 => 12,
      Self::I13 => 13,
      Self::I14 => 14,
      Self::I15 => 15,
    })
  }
}

impl Display for SgrIdx {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    let Ok(code) = self.code();
    write!(f, "{code}")
  }
}

impl Debug for SgrIdx {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    write!(f, "{}", self)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Rav1dRestorationType {
  #[default]
  None,
  Switchable,
  Wiener,
  SgrProj(SgrIdx),
}

impl Rav1dRestorationType {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public const encoding is used directly to define the existing C restoration constants"
    )
  )]
  pub const fn to_repr(&self) -> Dav1dRestorationType {
    match *self {
      Self::None => 0,
      Self::Switchable => 1,
      Self::Wiener => 2,
      Self::SgrProj(idx) => {
        let Ok(code) = idx.code();
        3_u8.wrapping_add(code)
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the established representation lookup returns the presence of a recognized restoration code"
    )
  )]
  pub const fn from_repr(repr: ::console_core_api_compatibility::NativeSize) -> Option<Self> {
    Some(match repr {
      0 => Self::None,
      1 => Self::Switchable,
      2 => Self::Wiener,
      3 => Self::SgrProj(SgrIdx::I0),
      _ => return None,
    })
  }
}

pub type Dav1dWarpedMotionType = c_uint;
pub const DAV1D_WM_TYPE_IDENTITY: Dav1dWarpedMotionType = 0;
pub const DAV1D_WM_TYPE_TRANSLATION: Dav1dWarpedMotionType = 1;
pub const DAV1D_WM_TYPE_ROT_ZOOM: Dav1dWarpedMotionType = 2;
pub const DAV1D_WM_TYPE_AFFINE: Dav1dWarpedMotionType = 3;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromRepr)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the AV1 global motion table and the C conversion use these numeric modes"
  )
)]
pub enum Rav1dWarpedMotionType {
  Identity = 0,
  Translation = 1,
  RotZoom = 2,
  Affine = 3,
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dWarpedMotionParams {
  pub r#type: Dav1dWarpedMotionType,
  pub matrix: [i32; 6],
  pub abcd: [i16; 4],
}

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the C layout accessors return a single fixed-width coefficient from their fixed-size wire array"
  )
)]
impl Dav1dWarpedMotionParams {
  #[must_use]
  pub const fn alpha(&self) -> i16 {
    self.abcd[0]
  }

  #[must_use]
  pub const fn beta(&self) -> i16 {
    self.abcd[1]
  }

  #[must_use]
  pub const fn gamma(&self) -> i16 {
    self.abcd[2]
  }

  #[must_use]
  pub const fn delta(&self) -> i16 {
    self.abcd[3]
  }
}

#[derive(Clone)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "the matrix is built in stages by the decoder and its checked C conversion validates only the incoming numeric motion type"
  )
)]
pub struct Rav1dWarpedMotionParams {
  pub r#type: Rav1dWarpedMotionType,
  pub matrix: [i32; 6],
  pub abcd: RelaxedAtomic<[i16; 4]>,
}

#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the established warp accessors read a single coefficient for existing reconstruction and diagnostics callers"
  )
)]
impl Rav1dWarpedMotionParams {
  #[must_use]
  pub fn alpha(&self) -> i16 {
    let Ok(abcd) = self.abcd.get();

    match abcd {
      [_explicit_element, ..] => _explicit_element,
    }
  }

  #[must_use]
  pub fn beta(&self) -> i16 {
    let Ok(abcd) = self.abcd.get();

    match abcd {
      [_, _explicit_element, ..] => _explicit_element,
    }
  }

  #[must_use]
  pub fn gamma(&self) -> i16 {
    let Ok(abcd) = self.abcd.get();

    match abcd {
      [_, _, _explicit_element, ..] => _explicit_element,
    }
  }

  #[must_use]
  pub fn delta(&self) -> i16 {
    let Ok(abcd) = self.abcd.get();

    match abcd {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    }
  }
}

impl TryFrom<Dav1dWarpedMotionParams> for Rav1dWarpedMotionParams {
  type Error = ();

  fn try_from(value: Dav1dWarpedMotionParams) -> Result<Self, Self::Error> {
    let Dav1dWarpedMotionParams {
      r#type,
      matrix,
      abcd,
    } = value;
    let rav1d_warped_motion_type = Rav1dWarpedMotionType::from_repr({
      let ::core::result::Result::Ok(explicit_position) =
        ::console_core_number_conversion::wrapped_index::<::core::primitive::u32>(r#type);
      explicit_position
    })
    .ok_or(())?;

    Ok(Self {
            r#type: rav1d_warped_motion_type,
            matrix,
            abcd: <crate::src::relaxed_atomic::RelaxedAtomic<[::core::primitive::i16; 4]> as ::core::convert::From<[::core::primitive::i16; 4]>>::from(abcd),
        })
  }
}

impl From<Rav1dWarpedMotionParams> for Dav1dWarpedMotionParams {
  fn from(value: Rav1dWarpedMotionParams) -> Self {
    let Rav1dWarpedMotionParams {
      r#type,
      matrix,
      abcd,
    } = value;
    Self {
      r#type: match r#type {
        Rav1dWarpedMotionType::Identity => DAV1D_WM_TYPE_IDENTITY,
        Rav1dWarpedMotionType::Translation => DAV1D_WM_TYPE_TRANSLATION,
        Rav1dWarpedMotionType::RotZoom => DAV1D_WM_TYPE_ROT_ZOOM,
        Rav1dWarpedMotionType::Affine => DAV1D_WM_TYPE_AFFINE,
      },
      matrix,
      abcd: ({
        let Ok(explicit_value) = abcd.get();
        explicit_value
      }),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, EnumCount, FromRepr, Default)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the format numbering is shared by the fixed C compatibility table and its layout flags"
  )
)]
pub enum Rav1dPixelLayout {
  #[default]
  I400 = 0,
  I420 = 1,
  I422 = 2,
  I444 = 3,
}

impl Rav1dPixelLayout {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public const encoding is used directly by established C ABI constants"
    )
  )]
  pub const fn into_rav1d(self) -> Dav1dPixelLayout {
    match self {
      Self::I400 => 0,
      Self::I420 => 1,
      Self::I422 => 2,
      Self::I444 => 3,
    }
  }
}

impl Sub for Rav1dPixelLayout {
  type Output = Rav1dPixelLayout;

  #[cfg_attr(
    dylint_lib = "explicit004_no_panic",
    expect(
      explicit004_no_panic,
      reason = "the existing Sub trait returns a layout directly and preserves its invalid subtraction contract"
    )
  )]
  fn sub(self, rhs: Self) -> Self::Output {
    let difference = self
      .ordinal()
      .checked_sub(rhs.ordinal())
      .expect("pixel layout subtraction requires an ordered pair");

    Self::try_from(u32::from(difference))
      .expect("the distance between two layout codes fits the layout table")
  }
}

pub type Dav1dPixelLayout = c_uint;
pub const DAV1D_PIXEL_LAYOUT_I400: Dav1dPixelLayout = Rav1dPixelLayout::I400.into_rav1d();
pub const DAV1D_PIXEL_LAYOUT_I420: Dav1dPixelLayout = Rav1dPixelLayout::I420.into_rav1d();
pub const DAV1D_PIXEL_LAYOUT_I422: Dav1dPixelLayout = Rav1dPixelLayout::I422.into_rav1d();
pub const DAV1D_PIXEL_LAYOUT_I444: Dav1dPixelLayout = Rav1dPixelLayout::I444.into_rav1d();

impl From<Rav1dPixelLayout> for Dav1dPixelLayout {
  fn from(value: Rav1dPixelLayout) -> Self {
    value.into_rav1d()
  }
}

impl TryFrom<Dav1dPixelLayout> for Rav1dPixelLayout {
  type Error = ();

  fn try_from(value: Dav1dPixelLayout) -> Result<Self, Self::Error> {
    match value {
      0 => Ok(Self::I400),
      1 => Ok(Self::I420),
      2 => Ok(Self::I422),
      3 => Ok(Self::I444),
      _ => Err(()),
    }
  }
}

impl EnumKey<{ Self::COUNT }> for Rav1dPixelLayout {
  fn ordinal(self) -> u8 {
    match self {
      Self::I400 => 0,
      Self::I420 => 1,
      Self::I422 => 2,
      Self::I444 => 3,
    }
  }
}

impl BitAnd for Rav1dPixelLayout {
  type Output = bool;

  fn bitand(self, rhs: Self) -> Self::Output {
    (self.ordinal() & rhs.ordinal()) != 0
  }
}

#[derive(Clone, Copy, PartialEq, Eq, EnumCount)]
pub(crate) enum Rav1dPixelLayoutSubSampled {
  I420,
  I422,
  I444,
}

impl EnumKey<{ Self::COUNT }> for Rav1dPixelLayoutSubSampled {
  fn ordinal(self) -> u8 {
    match self {
      Self::I420 => 0,
      Self::I422 => 1,
      Self::I444 => 2,
    }
  }
}

impl TryFrom<Rav1dPixelLayout> for Rav1dPixelLayoutSubSampled {
  type Error = ();

  fn try_from(value: Rav1dPixelLayout) -> Result<Self, Self::Error> {
    use Rav1dPixelLayout::*;
    Ok(match value {
      I400 => return Err(()),
      I420 => Self::I420,
      I422 => Self::I422,
      I444 => Self::I444,
    })
  }
}

impl From<Rav1dPixelLayoutSubSampled> for Rav1dPixelLayout {
  fn from(value: Rav1dPixelLayoutSubSampled) -> Self {
    use Rav1dPixelLayoutSubSampled::*;

    match value {
      I420 => Self::I420,
      I422 => Self::I422,
      I444 => Self::I444,
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, FromRepr, Default)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the AV1 coded frame type and C headers use these exact numeric values"
  )
)]
pub enum Rav1dFrameType {
  #[default]
  Key = 0,
  Inter = 1,
  Intra = 2,
  Switch = 3,
}

impl Rav1dFrameType {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public const encoding is used directly by established C ABI constants"
    )
  )]
  pub const fn into_rav1d(self) -> Dav1dFrameType {
    match self {
      Self::Key => 0,
      Self::Inter => 1,
      Self::Intra => 2,
      Self::Switch => 3,
    }
  }
}

pub type Dav1dFrameType = c_uint;
pub const DAV1D_FRAME_TYPE_KEY: Dav1dFrameType = Rav1dFrameType::Key.into_rav1d();
pub const DAV1D_FRAME_TYPE_INTER: Dav1dFrameType = Rav1dFrameType::Inter.into_rav1d();
pub const DAV1D_FRAME_TYPE_INTRA: Dav1dFrameType = Rav1dFrameType::Intra.into_rav1d();
pub const DAV1D_FRAME_TYPE_SWITCH: Dav1dFrameType = Rav1dFrameType::Switch.into_rav1d();

impl From<Rav1dFrameType> for Dav1dFrameType {
  fn from(value: Rav1dFrameType) -> Self {
    value.into_rav1d()
  }
}

impl TryFrom<Dav1dFrameType> for Rav1dFrameType {
  type Error = ();

  fn try_from(value: Dav1dFrameType) -> Result<Self, Self::Error> {
    match value {
      0 => Ok(Self::Key),
      1 => Ok(Self::Inter),
      2 => Ok(Self::Intra),
      3 => Ok(Self::Switch),
      _ => Err(()),
    }
  }
}

impl Rav1dFrameType {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public const predicate supplies the same classification throughout the existing decoder callers"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(
      explicit007_no_bool_return,
      reason = "the existing frame classification predicate is used with the decoder boolean conditions"
    )
  )]
  pub const fn is_inter_or_switch(&self) -> bool {
    matches!(self, Self::Inter | Self::Switch)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the public const predicate supplies the same classification throughout the existing decoder callers"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(
      explicit007_no_bool_return,
      reason = "the existing frame classification predicate is used with the decoder boolean conditions"
    )
  )]
  pub const fn is_key_or_intra(&self) -> bool {
    matches!(self, Self::Key | Self::Intra)
  }
}

pub type Dav1dColorPrimaries = c_uint;
pub const DAV1D_COLOR_PRI_BT709: Dav1dColorPrimaries = Rav1dColorPrimaries::BT709.to_dav1d();
pub const DAV1D_COLOR_PRI_UNKNOWN: Dav1dColorPrimaries = Rav1dColorPrimaries::UNKNOWN.to_dav1d();
pub const DAV1D_COLOR_PRI_BT470M: Dav1dColorPrimaries = Rav1dColorPrimaries::BT470M.to_dav1d();
pub const DAV1D_COLOR_PRI_BT470BG: Dav1dColorPrimaries = Rav1dColorPrimaries::BT470BG.to_dav1d();
pub const DAV1D_COLOR_PRI_BT601: Dav1dColorPrimaries = Rav1dColorPrimaries::BT601.to_dav1d();
pub const DAV1D_COLOR_PRI_SMPTE240: Dav1dColorPrimaries =
  Rav1dColorPrimaries::SMPTE240.to_dav1d();
pub const DAV1D_COLOR_PRI_FILM: Dav1dColorPrimaries = Rav1dColorPrimaries::FILM.to_dav1d();
pub const DAV1D_COLOR_PRI_BT2020: Dav1dColorPrimaries = Rav1dColorPrimaries::BT2020.to_dav1d();
pub const DAV1D_COLOR_PRI_XYZ: Dav1dColorPrimaries = Rav1dColorPrimaries::XYZ.to_dav1d();
pub const DAV1D_COLOR_PRI_SMPTE431: Dav1dColorPrimaries =
  Rav1dColorPrimaries::SMPTE431.to_dav1d();
pub const DAV1D_COLOR_PRI_SMPTE432: Dav1dColorPrimaries =
  Rav1dColorPrimaries::SMPTE432.to_dav1d();
pub const DAV1D_COLOR_PRI_EBU3213: Dav1dColorPrimaries = Rav1dColorPrimaries::EBU3213.to_dav1d();
pub const DAV1D_COLOR_PRI_RESERVED: Dav1dColorPrimaries = 255;

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "every byte is a representable CICP code including reserved values and the C conversion rejects values outside one byte"
  )
)]
pub struct Rav1dColorPrimaries(pub u8);

impl Rav1dColorPrimaries {
  pub const BT709: Self = Self(1);
  pub const UNKNOWN: Self = Self(2);
  pub const BT470M: Self = Self(4);
  pub const BT470BG: Self = Self(5);
  pub const BT601: Self = Self(6);
  pub const SMPTE240: Self = Self(7);
  pub const FILM: Self = Self(8);
  pub const BT2020: Self = Self(9);
  pub const XYZ: Self = Self(10);
  pub const SMPTE431: Self = Self(11);
  pub const SMPTE432: Self = Self(12);
  pub const EBU3213: Self = Self(22);

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the color code is used directly by the existing public const CICP constants"
    )
  )]
  const fn to_dav1d(self) -> Dav1dColorPrimaries {
    ::console_core_number_conversion::const_wrapped_integer!(unsigned ::core::primitive::u8 => Dav1dColorPrimaries, self.0)
  }
}

impl From<Rav1dColorPrimaries> for Dav1dColorPrimaries {
  fn from(value: Rav1dColorPrimaries) -> Self {
    value.to_dav1d()
  }
}

impl TryFrom<Dav1dColorPrimaries> for Rav1dColorPrimaries {
  type Error = ();

  fn try_from(value: Dav1dColorPrimaries) -> Result<Self, Self::Error> {
    u8::try_from(value).map(Self).map_err(|_| ())
  }
}

pub type Dav1dTransferCharacteristics = c_uint;
pub const DAV1D_TRC_BT709: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::BT709.to_dav1d();
pub const DAV1D_TRC_UNKNOWN: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::UNKNOWN.to_dav1d();
pub const DAV1D_TRC_BT470M: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::BT470M.to_dav1d();
pub const DAV1D_TRC_BT470BG: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::BT470BG.to_dav1d();
pub const DAV1D_TRC_BT601: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::BT601.to_dav1d();
pub const DAV1D_TRC_SMPTE240: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::SMPTE240.to_dav1d();
pub const DAV1D_TRC_LINEAR: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::LINEAR.to_dav1d();
pub const DAV1D_TRC_LOG100: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::LOG100.to_dav1d();
pub const DAV1D_TRC_LOG100_SQRT10: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::LOG100_SQRT10.to_dav1d();
pub const DAV1D_TRC_IEC61966: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::IEC61966.to_dav1d();
pub const DAV1D_TRC_BT1361: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::BT1361.to_dav1d();
pub const DAV1D_TRC_SRGB: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::SRGB.to_dav1d();
pub const DAV1D_TRC_BT2020_10BIT: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::BT2020_10BIT.to_dav1d();
pub const DAV1D_TRC_BT2020_12BIT: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::BT2020_12BIT.to_dav1d();
pub const DAV1D_TRC_SMPTE2084: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::SMPTE2084.to_dav1d();
pub const DAV1D_TRC_SMPTE428: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::SMPTE428.to_dav1d();
pub const DAV1D_TRC_HLG: Dav1dTransferCharacteristics =
  Rav1dTransferCharacteristics::HLG.to_dav1d();
pub const DAV1D_TRC_RESERVED: Dav1dTransferCharacteristics = 255;

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "every byte is a representable CICP code including reserved values and the C conversion rejects values outside one byte"
  )
)]
pub struct Rav1dTransferCharacteristics(pub u8);

impl Rav1dTransferCharacteristics {
  pub const _RESERVED_0: Self = Self(0);
  pub const BT709: Self = Self(1);
  pub const UNKNOWN: Self = Self(2);
  pub const _RESERVED_3: Self = Self(3);
  pub const BT470M: Self = Self(4);
  pub const BT470BG: Self = Self(5);
  pub const BT601: Self = Self(6);
  pub const SMPTE240: Self = Self(7);
  pub const LINEAR: Self = Self(8);
  pub const LOG100: Self = Self(9);
  pub const LOG100_SQRT10: Self = Self(10);
  pub const IEC61966: Self = Self(11);
  pub const BT1361: Self = Self(12);
  pub const SRGB: Self = Self(13);
  pub const BT2020_10BIT: Self = Self(14);
  pub const BT2020_12BIT: Self = Self(15);
  pub const SMPTE2084: Self = Self(16);
  pub const SMPTE428: Self = Self(17);
  pub const HLG: Self = Self(18);

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the transfer code is used directly by the existing public const CICP constants"
    )
  )]
  const fn to_dav1d(self) -> Dav1dTransferCharacteristics {
    ::console_core_number_conversion::const_wrapped_integer!(unsigned ::core::primitive::u8 => Dav1dTransferCharacteristics, self.0)
  }
}

impl From<Rav1dTransferCharacteristics> for Dav1dTransferCharacteristics {
  fn from(value: Rav1dTransferCharacteristics) -> Self {
    value.to_dav1d()
  }
}

impl TryFrom<Dav1dTransferCharacteristics> for Rav1dTransferCharacteristics {
  type Error = ();

  fn try_from(value: Dav1dTransferCharacteristics) -> Result<Self, Self::Error> {
    u8::try_from(value).map(Self).map_err(|_| ())
  }
}

pub type Dav1dMatrixCoefficients = c_uint;
pub const DAV1D_MC_IDENTITY: Dav1dMatrixCoefficients =
  Rav1dMatrixCoefficients::IDENTITY.to_dav1d();
pub const DAV1D_MC_BT709: Dav1dMatrixCoefficients = Rav1dMatrixCoefficients::BT709.to_dav1d();
pub const DAV1D_MC_UNKNOWN: Dav1dMatrixCoefficients = Rav1dMatrixCoefficients::UNKNOWN.to_dav1d();
pub const DAV1D_MC_FCC: Dav1dMatrixCoefficients = Rav1dMatrixCoefficients::FCC.to_dav1d();
pub const DAV1D_MC_BT470BG: Dav1dMatrixCoefficients = Rav1dMatrixCoefficients::BT470BG.to_dav1d();
pub const DAV1D_MC_BT601: Dav1dMatrixCoefficients = Rav1dMatrixCoefficients::BT601.to_dav1d();
pub const DAV1D_MC_SMPTE240: Dav1dMatrixCoefficients =
  Rav1dMatrixCoefficients::SMPTE240.to_dav1d();
pub const DAV1D_MC_SMPTE_YCGCO: Dav1dMatrixCoefficients =
  Rav1dMatrixCoefficients::SMPTE_YCGCO.to_dav1d();
pub const DAV1D_MC_BT2020_NCL: Dav1dMatrixCoefficients =
  Rav1dMatrixCoefficients::BT2020_NCL.to_dav1d();
pub const DAV1D_MC_BT2020_CL: Dav1dMatrixCoefficients =
  Rav1dMatrixCoefficients::BT2020_CL.to_dav1d();
pub const DAV1D_MC_SMPTE2085: Dav1dMatrixCoefficients =
  Rav1dMatrixCoefficients::SMPTE2085.to_dav1d();
pub const DAV1D_MC_CHROMAT_NCL: Dav1dMatrixCoefficients =
  Rav1dMatrixCoefficients::CHROMAT_NCL.to_dav1d();
pub const DAV1D_MC_CHROMAT_CL: Dav1dMatrixCoefficients =
  Rav1dMatrixCoefficients::CHROMAT_CL.to_dav1d();
pub const DAV1D_MC_ICTCP: Dav1dMatrixCoefficients = Rav1dMatrixCoefficients::ICTCP.to_dav1d();
pub const DAV1D_MC_RESERVED: Dav1dMatrixCoefficients = 255;

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "every byte is a representable CICP code including reserved values and the C conversion rejects values outside one byte"
  )
)]
pub struct Rav1dMatrixCoefficients(pub u8);

impl Rav1dMatrixCoefficients {
  pub const IDENTITY: Self = Self(0);
  pub const BT709: Self = Self(1);
  pub const UNKNOWN: Self = Self(2);
  pub const _RESERVED_3: Self = Self(3);
  pub const FCC: Self = Self(4);
  pub const BT470BG: Self = Self(5);
  pub const BT601: Self = Self(6);
  pub const SMPTE240: Self = Self(7);
  pub const SMPTE_YCGCO: Self = Self(8);
  pub const BT2020_NCL: Self = Self(9);
  pub const BT2020_CL: Self = Self(10);
  pub const SMPTE2085: Self = Self(11);
  pub const CHROMAT_NCL: Self = Self(12);
  pub const CHROMAT_CL: Self = Self(13);
  pub const ICTCP: Self = Self(14);

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the matrix code is used directly by the existing public const CICP constants"
    )
  )]
  const fn to_dav1d(self) -> Dav1dMatrixCoefficients {
    ::console_core_number_conversion::const_wrapped_integer!(unsigned ::core::primitive::u8 => Dav1dMatrixCoefficients, self.0)
  }
}

impl From<Rav1dMatrixCoefficients> for Dav1dMatrixCoefficients {
  fn from(value: Rav1dMatrixCoefficients) -> Self {
    value.to_dav1d()
  }
}

impl TryFrom<Dav1dMatrixCoefficients> for Rav1dMatrixCoefficients {
  type Error = ();

  fn try_from(value: Dav1dMatrixCoefficients) -> Result<Self, Self::Error> {
    u8::try_from(value).map(Self).map_err(|_| ())
  }
}

pub type Dav1dChromaSamplePosition = c_uint;
pub const DAV1D_CHR_UNKNOWN: Dav1dChromaSamplePosition = 0;
pub const DAV1D_CHR_VERTICAL: Dav1dChromaSamplePosition = 1;
pub const DAV1D_CHR_COLOCATED: Dav1dChromaSamplePosition = 2;

#[derive(Clone, Copy, PartialEq, Eq, FromRepr)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the raw values include the AV1 reserved chroma sample position"
  )
)]
pub enum Rav1dChromaSamplePosition {
  Unknown = 0,
  #[doc = "Halfway between two luma samples."]
  Vertical = 1,
  Colocated = 2,
  _Reserved = 3,
}

impl From<Rav1dChromaSamplePosition> for Dav1dChromaSamplePosition {
  fn from(value: Rav1dChromaSamplePosition) -> Self {
    match value {
      Rav1dChromaSamplePosition::Unknown => 0,
      Rav1dChromaSamplePosition::Vertical => 1,
      Rav1dChromaSamplePosition::Colocated => 2,
      Rav1dChromaSamplePosition::_Reserved => 3,
    }
  }
}

impl TryFrom<Dav1dChromaSamplePosition> for Rav1dChromaSamplePosition {
  type Error = ();

  fn try_from(value: Dav1dChromaSamplePosition) -> Result<Self, Self::Error> {
    Self::from_repr({
      let ::core::result::Result::Ok(explicit_position) =
        ::console_core_number_conversion::wrapped_index::<::core::primitive::u32>(value);
      explicit_position
    })
    .ok_or(())
  }
}

#[repr(C)]
pub struct Rav1dContentLightLevel {
  pub max_content_light_level: u16,
  pub max_frame_average_light_level: u16,
}

pub type Dav1dContentLightLevel = Rav1dContentLightLevel;

#[repr(C)]
pub struct Rav1dMasteringDisplay {
  pub primaries: [[u16; 2]; 3],
  pub white_point: [u16; 2],
  pub max_luminance: u32,
  pub min_luminance: u32,
}

pub type Dav1dMasteringDisplay = Rav1dMasteringDisplay;

#[doc = "A C view into the boxed payload retained by the corresponding immutable frame metadata."]
#[cfg(feature = "c-ffi")]
#[repr(transparent)]
pub struct ITUTT35PayloadPtr(*const u8);

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "the containing metadata owns the immutable payload in a Box; moving its C view does not move or mutate those bytes"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "moving the C view preserves the lifetime and location of the immutable allocation retained with it"
  )
)]
// SAFETY: Moving the view preserves the address of the immutable payload owned by the frame.
unsafe impl Send for ITUTT35PayloadPtr {}

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "the pointer exposes only immutable payload bytes whose owner lives at least as long as the C view"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C view only lends shared access to immutable boxed bytes"
  )
)]
// SAFETY: Shared access only exposes a raw pointer to the immutable payload retained by the frame.
unsafe impl Sync for ITUTT35PayloadPtr {}

#[cfg(feature = "c-ffi")]
#[repr(C)]
pub struct Dav1dITUTT35 {
  pub country_code: u8,
  pub country_code_extension_byte: u8,
  pub payload_size: ::console_core_api_compatibility::NativeSize,
  pub payload: ITUTT35PayloadPtr,
}

#[cfg(not(feature = "c-ffi"))]
pub type Dav1dITUTT35 = Rav1dITUTT35;

#[derive(Clone)]
#[repr(C)]
pub struct Rav1dITUTT35 {
  pub country_code: u8,
  pub country_code_extension_byte: u8,
  pub payload: Box<[u8]>,
}

#[cfg(feature = "c-ffi")]
impl From<&Rav1dITUTT35> for Dav1dITUTT35 {
  fn from(value: &Rav1dITUTT35) -> Self {
    let Rav1dITUTT35 {
      country_code,
      country_code_extension_byte,
      ref payload,
    } = *value;
    Self {
      country_code,
      country_code_extension_byte,
      payload_size: payload.len(),
      payload: ITUTT35PayloadPtr(payload.as_ptr()),
    }
  }
}

impl Rav1dITUTT35 {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the owned metadata buffer must be made exclusive before immutable frame references retain the payload allocation"
    )
  )]
  pub fn to_immut(
    mutable: &mut Arc<Mutex<Vec<Rav1dITUTT35>>>,
  ) -> Rav1dResult<Arc<DRav1d<Box<[Rav1dITUTT35]>, Box<[Dav1dITUTT35]>>>> {
    let pending = match Arc::try_unwrap(std::mem::take(mutable)) {
      Ok(exclusive) => unpoison(exclusive.into_inner()),
      Err(shared) => {
        *mutable = shared;
        return Err(Rav1dError::EGeneric);
      }
    };
    let Ok(metadata) = Self::freeze(pending);
    Ok(metadata)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the C picture and its Rust owner retain the immutable packets and C payload views until their respective releases"
    )
  )]
  pub(crate) fn freeze(
    pending: Vec<Rav1dITUTT35>,
  ) -> Result<Arc<DRav1d<Box<[Rav1dITUTT35]>, Box<[Dav1dITUTT35]>>>, console_core_never::Never>
  {
    let rav1d = pending.into_boxed_slice();
    #[cfg(feature = "c-ffi")]
    let dav1d = rav1d.iter().map(Dav1dITUTT35::from).collect();
    #[cfg(not(feature = "c-ffi"))]
    let dav1d = rav1d.clone();
    Ok(Arc::new(DRav1d { rav1d, dav1d }))
  }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Dav1dSequenceHeaderOperatingPoint {
  pub major_level: u8,
  pub minor_level: u8,
  pub initial_display_delay: u8,
  pub idc: u16,
  pub tier: u8,
  pub decoder_model_param_present: u8,
  pub display_model_param_present: u8,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Rav1dSequenceHeaderOperatingPoint {
  pub major_level: u8,
  pub minor_level: u8,
  pub initial_display_delay: u8,
  pub idc: u16,
  pub tier: u8,
  pub decoder_model_param_present: u8,
  pub display_model_param_present: u8,
}

impl From<Dav1dSequenceHeaderOperatingPoint> for Rav1dSequenceHeaderOperatingPoint {
  fn from(value: Dav1dSequenceHeaderOperatingPoint) -> Self {
    let Dav1dSequenceHeaderOperatingPoint {
      major_level,
      minor_level,
      initial_display_delay,
      idc,
      tier,
      decoder_model_param_present,
      display_model_param_present,
    } = value;
    Self {
      major_level,
      minor_level,
      initial_display_delay,
      idc,
      tier,
      decoder_model_param_present,
      display_model_param_present,
    }
  }
}

impl From<Rav1dSequenceHeaderOperatingPoint> for Dav1dSequenceHeaderOperatingPoint {
  fn from(value: Rav1dSequenceHeaderOperatingPoint) -> Self {
    let Rav1dSequenceHeaderOperatingPoint {
      major_level,
      minor_level,
      initial_display_delay,
      idc,
      tier,
      decoder_model_param_present,
      display_model_param_present,
    } = value;
    Self {
      major_level,
      minor_level,
      initial_display_delay,
      idc,
      tier,
      decoder_model_param_present,
      display_model_param_present,
    }
  }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Dav1dSequenceHeaderOperatingParameterInfo {
  pub decoder_buffer_delay: u32,
  pub encoder_buffer_delay: u32,
  pub low_delay_mode: u8,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Rav1dSequenceHeaderOperatingParameterInfo {
  pub decoder_buffer_delay: u32,
  pub encoder_buffer_delay: u32,
  pub low_delay_mode: u8,
}

impl From<Dav1dSequenceHeaderOperatingParameterInfo>
  for Rav1dSequenceHeaderOperatingParameterInfo
{
  fn from(value: Dav1dSequenceHeaderOperatingParameterInfo) -> Self {
    let Dav1dSequenceHeaderOperatingParameterInfo {
      decoder_buffer_delay,
      encoder_buffer_delay,
      low_delay_mode,
    } = value;
    Self {
      decoder_buffer_delay,
      encoder_buffer_delay,
      low_delay_mode,
    }
  }
}

impl From<Rav1dSequenceHeaderOperatingParameterInfo>
  for Dav1dSequenceHeaderOperatingParameterInfo
{
  fn from(value: Rav1dSequenceHeaderOperatingParameterInfo) -> Self {
    let Rav1dSequenceHeaderOperatingParameterInfo {
      decoder_buffer_delay,
      encoder_buffer_delay,
      low_delay_mode,
    } = value;
    Self {
      decoder_buffer_delay,
      encoder_buffer_delay,
      low_delay_mode,
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dSequenceHeader {
  pub profile: u8,
  pub max_width: c_int,
  pub max_height: c_int,
  pub layout: Dav1dPixelLayout,
  pub pri: Dav1dColorPrimaries,
  pub trc: Dav1dTransferCharacteristics,
  pub mtrx: Dav1dMatrixCoefficients,
  pub chr: Dav1dChromaSamplePosition,
  pub hbd: u8,
  pub color_range: u8,
  pub num_operating_points: u8,
  pub operating_points: [Dav1dSequenceHeaderOperatingPoint; DAV1D_MAX_OPERATING_POINTS],
  pub still_picture: u8,
  pub reduced_still_picture_header: u8,
  pub timing_info_present: u8,
  pub num_units_in_tick: u32,
  pub time_scale: u32,
  pub equal_picture_interval: u8,
  pub num_ticks_per_picture: u32,
  pub decoder_model_info_present: u8,
  pub encoder_decoder_buffer_delay_length: u8,
  pub num_units_in_decoding_tick: u32,
  pub buffer_removal_delay_length: u8,
  pub frame_presentation_delay_length: u8,
  pub display_model_info_present: u8,
  pub width_n_bits: u8,
  pub height_n_bits: u8,
  pub frame_id_numbers_present: u8,
  pub delta_frame_id_n_bits: u8,
  pub frame_id_n_bits: u8,
  pub sb128: u8,
  pub filter_intra: u8,
  pub intra_edge_filter: u8,
  pub inter_intra: u8,
  pub masked_compound: u8,
  pub warped_motion: u8,
  pub dual_filter: u8,
  pub order_hint: u8,
  pub jnt_comp: u8,
  pub ref_frame_mvs: u8,
  pub screen_content_tools: Dav1dAdaptiveBoolean,
  pub force_integer_mv: Dav1dAdaptiveBoolean,
  pub order_hint_n_bits: u8,
  pub super_res: u8,
  pub cdef: u8,
  pub restoration: u8,
  pub ss_hor: u8,
  pub ss_ver: u8,
  pub monochrome: u8,
  pub color_description_present: u8,
  pub separate_uv_delta_q: u8,
  pub film_grain_present: u8,
  pub operating_parameter_info:
    [Dav1dSequenceHeaderOperatingParameterInfo; DAV1D_MAX_OPERATING_POINTS],
}

const _: () = {
  use core::mem::{align_of, offset_of, size_of};

  assert!(size_of::<Dav1dSequenceHeader>() == 808);
  assert!(align_of::<Dav1dSequenceHeader>() == 4);
  assert!(offset_of!(Dav1dSequenceHeader, profile) == 0);
  assert!(offset_of!(Dav1dSequenceHeader, max_width) == 4);
  assert!(offset_of!(Dav1dSequenceHeader, max_height) == 8);
  assert!(offset_of!(Dav1dSequenceHeader, layout) == 12);
  assert!(offset_of!(Dav1dSequenceHeader, pri) == 16);
  assert!(offset_of!(Dav1dSequenceHeader, trc) == 20);
  assert!(offset_of!(Dav1dSequenceHeader, mtrx) == 24);
  assert!(offset_of!(Dav1dSequenceHeader, chr) == 28);
  assert!(offset_of!(Dav1dSequenceHeader, hbd) == 32);
  assert!(offset_of!(Dav1dSequenceHeader, color_range) == 33);
  assert!(offset_of!(Dav1dSequenceHeader, num_operating_points) == 34);
  assert!(offset_of!(Dav1dSequenceHeader, operating_points) == 36);
  assert!(offset_of!(Dav1dSequenceHeader, still_picture) == 356);
  assert!(offset_of!(Dav1dSequenceHeader, reduced_still_picture_header) == 357);
  assert!(offset_of!(Dav1dSequenceHeader, timing_info_present) == 358);
  assert!(offset_of!(Dav1dSequenceHeader, num_units_in_tick) == 360);
  assert!(offset_of!(Dav1dSequenceHeader, time_scale) == 364);
  assert!(offset_of!(Dav1dSequenceHeader, equal_picture_interval) == 368);
  assert!(offset_of!(Dav1dSequenceHeader, num_ticks_per_picture) == 372);
  assert!(offset_of!(Dav1dSequenceHeader, decoder_model_info_present) == 376);
  assert!(offset_of!(Dav1dSequenceHeader, encoder_decoder_buffer_delay_length) == 377);
  assert!(offset_of!(Dav1dSequenceHeader, num_units_in_decoding_tick) == 380);
  assert!(offset_of!(Dav1dSequenceHeader, buffer_removal_delay_length) == 384);
  assert!(offset_of!(Dav1dSequenceHeader, frame_presentation_delay_length) == 385);
  assert!(offset_of!(Dav1dSequenceHeader, display_model_info_present) == 386);
  assert!(offset_of!(Dav1dSequenceHeader, width_n_bits) == 387);
  assert!(offset_of!(Dav1dSequenceHeader, height_n_bits) == 388);
  assert!(offset_of!(Dav1dSequenceHeader, frame_id_numbers_present) == 389);
  assert!(offset_of!(Dav1dSequenceHeader, delta_frame_id_n_bits) == 390);
  assert!(offset_of!(Dav1dSequenceHeader, frame_id_n_bits) == 391);
  assert!(offset_of!(Dav1dSequenceHeader, sb128) == 392);
  assert!(offset_of!(Dav1dSequenceHeader, filter_intra) == 393);
  assert!(offset_of!(Dav1dSequenceHeader, intra_edge_filter) == 394);
  assert!(offset_of!(Dav1dSequenceHeader, inter_intra) == 395);
  assert!(offset_of!(Dav1dSequenceHeader, masked_compound) == 396);
  assert!(offset_of!(Dav1dSequenceHeader, warped_motion) == 397);
  assert!(offset_of!(Dav1dSequenceHeader, dual_filter) == 398);
  assert!(offset_of!(Dav1dSequenceHeader, order_hint) == 399);
  assert!(offset_of!(Dav1dSequenceHeader, jnt_comp) == 400);
  assert!(offset_of!(Dav1dSequenceHeader, ref_frame_mvs) == 401);
  assert!(offset_of!(Dav1dSequenceHeader, screen_content_tools) == 404);
  assert!(offset_of!(Dav1dSequenceHeader, force_integer_mv) == 408);
  assert!(offset_of!(Dav1dSequenceHeader, order_hint_n_bits) == 412);
  assert!(offset_of!(Dav1dSequenceHeader, super_res) == 413);
  assert!(offset_of!(Dav1dSequenceHeader, cdef) == 414);
  assert!(offset_of!(Dav1dSequenceHeader, restoration) == 415);
  assert!(offset_of!(Dav1dSequenceHeader, ss_hor) == 416);
  assert!(offset_of!(Dav1dSequenceHeader, ss_ver) == 417);
  assert!(offset_of!(Dav1dSequenceHeader, monochrome) == 418);
  assert!(offset_of!(Dav1dSequenceHeader, color_description_present) == 419);
  assert!(offset_of!(Dav1dSequenceHeader, separate_uv_delta_q) == 420);
  assert!(offset_of!(Dav1dSequenceHeader, film_grain_present) == 421);
  assert!(offset_of!(Dav1dSequenceHeader, operating_parameter_info) == 424);
};

#[derive(Clone, Copy, PartialEq, Eq, FromRepr)]
#[cfg_attr(
  dylint_lib = "explicit070_spaced_discriminants",
  expect(
    explicit070_spaced_discriminants,
    reason = "the numeric discriminants are the AV1 sequence profile codes"
  )
)]
pub enum Rav1dProfile {
  Main = 0,
  High = 1,
  Professional = 2,
}

impl From<Rav1dProfile> for u8 {
  fn from(value: Rav1dProfile) -> Self {
    match value {
      Rav1dProfile::Main => 0,
      Rav1dProfile::High => 1,
      Rav1dProfile::Professional => 2,
    }
  }
}

impl TryFrom<u8> for Rav1dProfile {
  type Error = ();

  fn try_from(value: u8) -> Result<Self, Self::Error> {
    Self::from_repr(
      match ::console_core_number_conversion::index::<::core::primitive::u8>(value) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      },
    )
    .ok_or(())
  }
}

#[derive(Clone)]
#[repr(C)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "the public decoded fields already carry their valid Rust code types and the C conversion rejects only unrecognized raw codes"
  )
)]
pub struct Rav1dSequenceHeader {
  pub profile: Rav1dProfile,
  pub max_width: c_int,
  pub max_height: c_int,
  pub layout: Rav1dPixelLayout,
  pub pri: Rav1dColorPrimaries,
  pub trc: Rav1dTransferCharacteristics,
  pub mtrx: Rav1dMatrixCoefficients,
  pub chr: Rav1dChromaSamplePosition,
  pub hbd: u8,
  pub color_range: u8,
  pub num_operating_points: u8,
  pub operating_points: [Rav1dSequenceHeaderOperatingPoint; RAV1D_MAX_OPERATING_POINTS],
  pub still_picture: u8,
  pub reduced_still_picture_header: u8,
  pub timing_info_present: u8,
  pub num_units_in_tick: u32,
  pub time_scale: u32,
  pub equal_picture_interval: u8,
  pub num_ticks_per_picture: u32,
  pub decoder_model_info_present: u8,
  pub encoder_decoder_buffer_delay_length: u8,
  pub num_units_in_decoding_tick: u32,
  pub buffer_removal_delay_length: u8,
  pub frame_presentation_delay_length: u8,
  pub display_model_info_present: u8,
  pub width_n_bits: u8,
  pub height_n_bits: u8,
  pub frame_id_numbers_present: u8,
  pub delta_frame_id_n_bits: u8,
  pub frame_id_n_bits: u8,
  pub sb128: u8,
  pub filter_intra: u8,
  pub intra_edge_filter: u8,
  pub inter_intra: u8,
  pub masked_compound: u8,
  pub warped_motion: u8,
  pub dual_filter: u8,
  pub order_hint: u8,
  pub jnt_comp: u8,
  pub ref_frame_mvs: u8,
  pub screen_content_tools: Rav1dAdaptiveBoolean,
  pub force_integer_mv: Rav1dAdaptiveBoolean,
  pub order_hint_n_bits: u8,
  pub super_res: u8,
  pub cdef: u8,
  pub restoration: u8,
  pub ss_hor: u8,
  pub ss_ver: u8,
  pub monochrome: u8,
  pub color_description_present: u8,
  pub separate_uv_delta_q: u8,
  pub film_grain_present: u8,
  pub operating_parameter_info:
    [Rav1dSequenceHeaderOperatingParameterInfo; RAV1D_MAX_OPERATING_POINTS],
}

impl Rav1dSequenceHeader {
  #[doc = "Compares the fields that identify a coded sequence while leaving decoder buffering information out of the comparison."]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the existing header classification performs a direct equality comparison"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(
      explicit007_no_bool_return,
      reason = "this established predicate compares two headers and returns their equality directly"
    )
  )]
  pub fn eq_without_operating_parameter_info(&self, other: &Self) -> bool {
    #[cfg_attr(
      dylint_lib = "explicit080_macro_budget",
      expect(
        explicit080_macro_budget,
        reason = "the single field list must both exhaustively destructure the header and compare the decoded sequence fields"
      )
    )]
    macro_rules! compare {
      ($($field:ident),+ $(,)?) => {{
        let Self { $($field,)+ operating_parameter_info: _ } = self;
        true $(&& *$field == other.$field)*
      }};
    }

    compare!(
      profile,
      max_width,
      max_height,
      layout,
      pri,
      trc,
      mtrx,
      chr,
      hbd,
      color_range,
      num_operating_points,
      operating_points,
      still_picture,
      reduced_still_picture_header,
      timing_info_present,
      num_units_in_tick,
      time_scale,
      equal_picture_interval,
      num_ticks_per_picture,
      decoder_model_info_present,
      encoder_decoder_buffer_delay_length,
      num_units_in_decoding_tick,
      buffer_removal_delay_length,
      frame_presentation_delay_length,
      display_model_info_present,
      width_n_bits,
      height_n_bits,
      frame_id_numbers_present,
      delta_frame_id_n_bits,
      frame_id_n_bits,
      sb128,
      filter_intra,
      intra_edge_filter,
      inter_intra,
      masked_compound,
      warped_motion,
      dual_filter,
      order_hint,
      jnt_comp,
      ref_frame_mvs,
      screen_content_tools,
      force_integer_mv,
      order_hint_n_bits,
      super_res,
      cdef,
      restoration,
      ss_hor,
      ss_ver,
      monochrome,
      color_description_present,
      separate_uv_delta_q,
      film_grain_present,
    )
  }
}

fn checked_header_field<Domain, Wire>(wire: Wire) -> Rav1dResult<Domain>
where
  Domain: TryFrom<Wire>,
{
  Domain::try_from(wire).map_err(|_invalid_code| Rav1dError::EINVAL)
}

impl TryFrom<Dav1dSequenceHeader> for Rav1dSequenceHeader {
  type Error = Rav1dError;

  fn try_from(value: Dav1dSequenceHeader) -> Rav1dResult<Self> {
    let profile = checked_header_field(value.profile)?;
    let layout = checked_header_field(value.layout)?;
    let pri = checked_header_field(value.pri)?;
    let trc = checked_header_field(value.trc)?;
    let mtrx = checked_header_field(value.mtrx)?;
    let chr = checked_header_field(value.chr)?;
    let screen_content_tools = checked_header_field(value.screen_content_tools)?;
    let force_integer_mv = checked_header_field(value.force_integer_mv)?;

    Ok(Self {
      profile,
      max_width: value.max_width,
      max_height: value.max_height,
      layout,
      pri,
      trc,
      mtrx,
      chr,
      hbd: value.hbd,
      color_range: value.color_range,
      num_operating_points: value.num_operating_points,
      operating_points: value
        .operating_points
        .map(Rav1dSequenceHeaderOperatingPoint::from),
      still_picture: value.still_picture,
      reduced_still_picture_header: value.reduced_still_picture_header,
      timing_info_present: value.timing_info_present,
      num_units_in_tick: value.num_units_in_tick,
      time_scale: value.time_scale,
      equal_picture_interval: value.equal_picture_interval,
      num_ticks_per_picture: value.num_ticks_per_picture,
      decoder_model_info_present: value.decoder_model_info_present,
      encoder_decoder_buffer_delay_length: value.encoder_decoder_buffer_delay_length,
      num_units_in_decoding_tick: value.num_units_in_decoding_tick,
      buffer_removal_delay_length: value.buffer_removal_delay_length,
      frame_presentation_delay_length: value.frame_presentation_delay_length,
      display_model_info_present: value.display_model_info_present,
      width_n_bits: value.width_n_bits,
      height_n_bits: value.height_n_bits,
      frame_id_numbers_present: value.frame_id_numbers_present,
      delta_frame_id_n_bits: value.delta_frame_id_n_bits,
      frame_id_n_bits: value.frame_id_n_bits,
      sb128: value.sb128,
      filter_intra: value.filter_intra,
      intra_edge_filter: value.intra_edge_filter,
      inter_intra: value.inter_intra,
      masked_compound: value.masked_compound,
      warped_motion: value.warped_motion,
      dual_filter: value.dual_filter,
      order_hint: value.order_hint,
      jnt_comp: value.jnt_comp,
      ref_frame_mvs: value.ref_frame_mvs,
      screen_content_tools,
      force_integer_mv,
      order_hint_n_bits: value.order_hint_n_bits,
      super_res: value.super_res,
      cdef: value.cdef,
      restoration: value.restoration,
      ss_hor: value.ss_hor,
      ss_ver: value.ss_ver,
      monochrome: value.monochrome,
      color_description_present: value.color_description_present,
      separate_uv_delta_q: value.separate_uv_delta_q,
      film_grain_present: value.film_grain_present,
      operating_parameter_info: value
        .operating_parameter_info
        .map(Rav1dSequenceHeaderOperatingParameterInfo::from),
    })
  }
}

impl From<Rav1dSequenceHeader> for Dav1dSequenceHeader {
  fn from(value: Rav1dSequenceHeader) -> Self {
    Self {
      profile: u8::from(value.profile),
      max_width: value.max_width,
      max_height: value.max_height,
      layout: u32::from(value.layout),
      pri: u32::from(value.pri),
      trc: u32::from(value.trc),
      mtrx: u32::from(value.mtrx),
      chr: u32::from(value.chr),
      hbd: value.hbd,
      color_range: value.color_range,
      num_operating_points: value.num_operating_points,
      operating_points: value
        .operating_points
        .map(Dav1dSequenceHeaderOperatingPoint::from),
      still_picture: value.still_picture,
      reduced_still_picture_header: value.reduced_still_picture_header,
      timing_info_present: value.timing_info_present,
      num_units_in_tick: value.num_units_in_tick,
      time_scale: value.time_scale,
      equal_picture_interval: value.equal_picture_interval,
      num_ticks_per_picture: value.num_ticks_per_picture,
      decoder_model_info_present: value.decoder_model_info_present,
      encoder_decoder_buffer_delay_length: value.encoder_decoder_buffer_delay_length,
      num_units_in_decoding_tick: value.num_units_in_decoding_tick,
      buffer_removal_delay_length: value.buffer_removal_delay_length,
      frame_presentation_delay_length: value.frame_presentation_delay_length,
      display_model_info_present: value.display_model_info_present,
      width_n_bits: value.width_n_bits,
      height_n_bits: value.height_n_bits,
      frame_id_numbers_present: value.frame_id_numbers_present,
      delta_frame_id_n_bits: value.delta_frame_id_n_bits,
      frame_id_n_bits: value.frame_id_n_bits,
      sb128: value.sb128,
      filter_intra: value.filter_intra,
      intra_edge_filter: value.intra_edge_filter,
      inter_intra: value.inter_intra,
      masked_compound: value.masked_compound,
      warped_motion: value.warped_motion,
      dual_filter: value.dual_filter,
      order_hint: value.order_hint,
      jnt_comp: value.jnt_comp,
      ref_frame_mvs: value.ref_frame_mvs,
      screen_content_tools: u32::from(value.screen_content_tools),
      force_integer_mv: u32::from(value.force_integer_mv),
      order_hint_n_bits: value.order_hint_n_bits,
      super_res: value.super_res,
      cdef: value.cdef,
      restoration: value.restoration,
      ss_hor: value.ss_hor,
      ss_ver: value.ss_ver,
      monochrome: value.monochrome,
      color_description_present: value.color_description_present,
      separate_uv_delta_q: value.separate_uv_delta_q,
      film_grain_present: value.film_grain_present,
      operating_parameter_info: value
        .operating_parameter_info
        .map(Dav1dSequenceHeaderOperatingParameterInfo::from),
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dSegmentationData {
  pub delta_q: i16,
  pub delta_lf_y_v: i8,
  pub delta_lf_y_h: i8,
  pub delta_lf_u: i8,
  pub delta_lf_v: i8,
  pub r#ref: i8,
  pub skip: u8,
  pub globalmv: u8,
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dSegmentationData {
  pub delta_q: i16,
  pub delta_lf_y_v: i8,
  pub delta_lf_y_h: i8,
  pub delta_lf_u: i8,
  pub delta_lf_v: i8,
  pub r#ref: i8,
  pub skip: u8,
  pub globalmv: u8,
}

impl From<Dav1dSegmentationData> for Rav1dSegmentationData {
  fn from(value: Dav1dSegmentationData) -> Self {
    let Dav1dSegmentationData {
      delta_q,
      delta_lf_y_v,
      delta_lf_y_h,
      delta_lf_u,
      delta_lf_v,
      r#ref,
      skip,
      globalmv,
    } = value;
    Self {
      delta_q,
      delta_lf_y_v,
      delta_lf_y_h,
      delta_lf_u,
      delta_lf_v,
      r#ref,
      skip,
      globalmv,
    }
  }
}

impl From<Rav1dSegmentationData> for Dav1dSegmentationData {
  fn from(value: Rav1dSegmentationData) -> Self {
    let Rav1dSegmentationData {
      delta_q,
      delta_lf_y_v,
      delta_lf_y_h,
      delta_lf_u,
      delta_lf_v,
      r#ref,
      skip,
      globalmv,
    } = value;
    Self {
      delta_q,
      delta_lf_y_v,
      delta_lf_y_h,
      delta_lf_u,
      delta_lf_v,
      r#ref,
      skip,
      globalmv,
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dSegmentationDataSet {
  pub d: [Dav1dSegmentationData; {
    match console_core_number_conversion::const_u8_index(SegmentId::COUNT) {
      Ok(extent) => extent,
    }
  }],
  pub preskip: u8,
  pub last_active_segid: i8,
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dSegmentationDataSet {
  pub d: [Rav1dSegmentationData; {
    match console_core_number_conversion::const_u8_index(SegmentId::COUNT) {
      Ok(extent) => extent,
    }
  }],
  pub preskip: u8,
  pub last_active_segid: i8,
}

impl From<Dav1dSegmentationDataSet> for Rav1dSegmentationDataSet {
  fn from(value: Dav1dSegmentationDataSet) -> Self {
    let Dav1dSegmentationDataSet {
      d,
      preskip,
      last_active_segid,
    } = value;
    Self {
      d: d.map(|c| {
        <crate::include::dav1d::headers::Rav1dSegmentationData as ::core::convert::From<
          crate::include::dav1d::headers::Dav1dSegmentationData,
        >>::from(c)
      }),
      preskip,
      last_active_segid,
    }
  }
}

impl From<Rav1dSegmentationDataSet> for Dav1dSegmentationDataSet {
  fn from(value: Rav1dSegmentationDataSet) -> Self {
    let Rav1dSegmentationDataSet {
      d,
      preskip,
      last_active_segid,
    } = value;
    Self {
      d: d.map(|rust| {
        <crate::include::dav1d::headers::Dav1dSegmentationData as ::core::convert::From<
          crate::include::dav1d::headers::Rav1dSegmentationData,
        >>::from(rust)
      }),
      preskip,
      last_active_segid,
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dLoopfilterModeRefDeltas {
  pub mode_delta: [i8; 2],
  pub ref_delta: [i8; DAV1D_TOTAL_REFS_PER_FRAME],
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dLoopfilterModeRefDeltas {
  pub mode_delta: [i8; 2],
  pub ref_delta: [i8; RAV1D_TOTAL_REFS_PER_FRAME],
}

impl From<Dav1dLoopfilterModeRefDeltas> for Rav1dLoopfilterModeRefDeltas {
  fn from(value: Dav1dLoopfilterModeRefDeltas) -> Self {
    let Dav1dLoopfilterModeRefDeltas {
      mode_delta,
      ref_delta,
    } = value;
    Self {
      mode_delta,
      ref_delta,
    }
  }
}

impl From<Rav1dLoopfilterModeRefDeltas> for Dav1dLoopfilterModeRefDeltas {
  fn from(value: Rav1dLoopfilterModeRefDeltas) -> Self {
    let Rav1dLoopfilterModeRefDeltas {
      mode_delta,
      ref_delta,
    } = value;
    Self {
      mode_delta,
      ref_delta,
    }
  }
}

#[derive(Clone, Default)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "scaling, overlap and clipping are distinct coded grain choices and their constraints depend on the picture format"
  )
)]
pub struct Rav1dFilmGrainData {
  pub seed: c_uint,
  pub num_y_points: c_int,
  pub y_points: [[u8; 2]; 14],
  pub chroma_scaling_from_luma: bool,
  pub num_uv_points: [c_int; 2],
  pub uv_points: [[[u8; 2]; 10]; 2],
  pub scaling_shift: u8,
  pub ar_coeff_lag: c_int,
  pub ar_coeffs_y: [i8; 24],
  pub ar_coeffs_uv: [[i8; 28]; 2],
  pub ar_coeff_shift: u8,
  pub grain_scale_shift: u8,
  pub uv_mult: [c_int; 2],
  pub uv_luma_mult: [c_int; 2],
  pub uv_offset: [c_int; 2],
  pub overlap_flag: bool,
  pub clip_to_restricted_range: bool,
}

#[doc = "See the docs for [`Self::ar_coeff_shift`] for an explanation."]
#[derive(Clone)]
#[repr(C)]
#[repr(align(16))]
pub struct Dav1dFilmGrainData {
  pub seed: c_uint,
  pub num_y_points: c_int,
  pub y_points: [[u8; 2]; 14],
  pub chroma_scaling_from_luma: c_int,
  pub num_uv_points: [c_int; 2],
  pub uv_points: [[[u8; 2]; 10]; 2],
  pub scaling_shift: c_int,
  pub ar_coeff_lag: c_int,
  pub ar_coeffs_y: [i8; 24],
  pub ar_coeffs_uv: [[i8; 28]; 2],
  #[doc = ""]
  #[doc = "TODO(kkysen) This appears to be a bug in `dav1d`."]
  #[doc = ""]
  #[doc = "x86 asm uses `psrad` on a pointer to [`Self::ar_coeff_shift`]."]
  #[doc = "When `psrad`'s shift operand is a memory address, i.e. a `XMMWORD PTR`,"]
  #[doc = "it loads 128 bits from it, shifts by the lower 64 bits,"]
  #[doc = "and requires the 128 bits to be 128-bit/16-byte aligned."]
  #[doc = ""]
  #[doc = "Previously, and still in `dav1d`, [`Self::ar_coeff_shift`]"]
  #[doc = "is only 8-byte aligned, as is [`Self`]/[`Dav1dFilmGrainData`]."]
  #[doc = "However, in `dav1d`, [`Dav1dFilmGrainData`] is part of"]
  #[doc = "[`Dav1dFrameHeader`], which is allocated with [`malloc`]."]
  #[doc = "[`malloc`] happens to return 16-byte aligned pointers usually,"]
  #[doc = "but is not required to, so this is UB and will segfault if not aligned."]
  #[doc = ""]
  #[doc = "Due to the [`Rav1dFilmGrainData`] to [`Dav1dFilmGrainData`]"]
  #[doc = "conversion done now, the [`Dav1dFilmGrainData`] is stored on the stack,"]
  #[doc = "and often will not be 16-byte aligned, and thus will often segfault."]
  #[doc = ""]
  #[doc = "To fix this, [`Self::ar_coeff_shift`] must be 16-byte aligned."]
  #[doc = "This cannot be done only for the field without changing the offsets of"]
  #[doc = "the subsequent fields, however, so we instead align"]
  #[doc = "[`Dav1dFilmGrainData`] itself with `#[repr(align(16))]`."]
  #[doc = "[`Self::ar_coeff_shift`] is at offset `0xB0`/`176`,"]
  #[doc = "which is divisible by 16."]
  #[doc = ""]
  #[doc = "`psrad` also loads a full 128 bits, not just the 64 bits of"]
  #[doc = "[`Self::ar_coeff_shift`], even if it doesn't read them,"]
  #[doc = "so we must ensure that the following 64 bits are also deferenceable."]
  #[doc = "They indeed are in `dav1d`, but we must be careful,"]
  #[doc = "as [`Self::ar_coeff_shift`] being the last field would"]
  #[doc = "read 8 bytes out of bounds and be UB."]
  #[doc = ""]
  #[doc = "[`malloc`]: https://man7.org/linux/man-pages/man3/malloc.3.html"]
  pub ar_coeff_shift: u64,
  pub grain_scale_shift: c_int,
  pub uv_mult: [c_int; 2],
  pub uv_luma_mult: [c_int; 2],
  pub uv_offset: [c_int; 2],
  pub overlap_flag: c_int,
  pub clip_to_restricted_range: c_int,
}

impl From<Dav1dFilmGrainData> for Rav1dFilmGrainData {
  fn from(value: Dav1dFilmGrainData) -> Self {
    let Dav1dFilmGrainData {
      seed,
      num_y_points,
      y_points,
      chroma_scaling_from_luma,
      num_uv_points,
      uv_points,
      scaling_shift,
      ar_coeff_lag,
      ar_coeffs_y,
      ar_coeffs_uv,
      ar_coeff_shift,
      grain_scale_shift,
      uv_mult,
      uv_luma_mult,
      uv_offset,
      overlap_flag,
      clip_to_restricted_range,
    } = value;
    Self {
      seed,
      num_y_points,
      y_points,
      chroma_scaling_from_luma: chroma_scaling_from_luma != 0,
      num_uv_points,
      uv_points,
      scaling_shift: match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        scaling_shift,
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      },
      ar_coeff_lag,
      ar_coeffs_y,
      ar_coeffs_uv,
      ar_coeff_shift: match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        ar_coeff_shift,
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      },
      grain_scale_shift: match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        grain_scale_shift,
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      },
      uv_mult,
      uv_luma_mult,
      uv_offset,
      overlap_flag: overlap_flag != 0,
      clip_to_restricted_range: clip_to_restricted_range != 0,
    }
  }
}

impl From<Rav1dFilmGrainData> for Dav1dFilmGrainData {
  fn from(value: Rav1dFilmGrainData) -> Self {
    let Rav1dFilmGrainData {
      seed,
      num_y_points,
      y_points,
      chroma_scaling_from_luma,
      num_uv_points,
      uv_points,
      scaling_shift,
      ar_coeff_lag,
      ar_coeffs_y,
      ar_coeffs_uv,
      ar_coeff_shift,
      grain_scale_shift,
      uv_mult,
      uv_luma_mult,
      uv_offset,
      overlap_flag,
      clip_to_restricted_range,
    } = value;
    Self {
      seed,
      num_y_points,
      y_points,
      chroma_scaling_from_luma: <c_int as ::core::convert::From<::core::primitive::bool>>::from(
        chroma_scaling_from_luma,
      ),
      num_uv_points,
      uv_points,
      scaling_shift:
        <::core::primitive::i32 as ::core::convert::From<::core::primitive::u8>>::from(
          scaling_shift,
        ),
      ar_coeff_lag,
      ar_coeffs_y,
      ar_coeffs_uv,
      ar_coeff_shift:
        <::core::primitive::u64 as ::core::convert::From<::core::primitive::u8>>::from(
          ar_coeff_shift,
        ),
      grain_scale_shift: <::core::primitive::i32 as ::core::convert::From<
        ::core::primitive::u8,
      >>::from(grain_scale_shift),
      uv_mult,
      uv_luma_mult,
      uv_offset,
      overlap_flag: <c_int as ::core::convert::From<::core::primitive::bool>>::from(overlap_flag),
      clip_to_restricted_range: <c_int as ::core::convert::From<::core::primitive::bool>>::from(
        clip_to_restricted_range,
      ),
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dFrameHeaderFilmGrain {
  pub data: Dav1dFilmGrainData,
  pub present: u8,
  pub update: u8,
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dFrameHeaderFilmGrain {
  pub data: Rav1dFilmGrainData,
  pub present: u8,
  pub update: u8,
}

impl From<Dav1dFrameHeaderFilmGrain> for Rav1dFrameHeaderFilmGrain {
  fn from(value: Dav1dFrameHeaderFilmGrain) -> Self {
    let Dav1dFrameHeaderFilmGrain {
      data,
      present,
      update,
    } = value;
    Self {
      data: <crate::include::dav1d::headers::Rav1dFilmGrainData as ::core::convert::From<
        crate::include::dav1d::headers::Dav1dFilmGrainData,
      >>::from(data),
      present,
      update,
    }
  }
}

impl From<Rav1dFrameHeaderFilmGrain> for Dav1dFrameHeaderFilmGrain {
  fn from(value: Rav1dFrameHeaderFilmGrain) -> Self {
    let Rav1dFrameHeaderFilmGrain {
      data,
      present,
      update,
    } = value;
    Self {
      data: <crate::include::dav1d::headers::Dav1dFilmGrainData as ::core::convert::From<
        crate::include::dav1d::headers::Rav1dFilmGrainData,
      >>::from(data),
      present,
      update,
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dFrameHeaderOperatingPoint {
  pub buffer_removal_time: u32,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Rav1dFrameHeaderOperatingPoint {
  pub buffer_removal_time: u32,
}

impl From<Dav1dFrameHeaderOperatingPoint> for Rav1dFrameHeaderOperatingPoint {
  fn from(value: Dav1dFrameHeaderOperatingPoint) -> Self {
    let Dav1dFrameHeaderOperatingPoint {
      buffer_removal_time,
    } = value;
    Self {
      buffer_removal_time,
    }
  }
}

impl From<Rav1dFrameHeaderOperatingPoint> for Dav1dFrameHeaderOperatingPoint {
  fn from(value: Rav1dFrameHeaderOperatingPoint) -> Self {
    let Rav1dFrameHeaderOperatingPoint {
      buffer_removal_time,
    } = value;
    Self {
      buffer_removal_time,
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dFrameHeaderSuperRes {
  pub width_scale_denominator: u8,
  pub enabled: u8,
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dFrameHeaderSuperRes {
  pub width_scale_denominator: u8,
  pub enabled: bool,
}

impl From<Dav1dFrameHeaderSuperRes> for Rav1dFrameHeaderSuperRes {
  fn from(value: Dav1dFrameHeaderSuperRes) -> Self {
    let Dav1dFrameHeaderSuperRes {
      width_scale_denominator,
      enabled,
    } = value;
    Self {
      width_scale_denominator,
      enabled: enabled != 0,
    }
  }
}

impl From<Rav1dFrameHeaderSuperRes> for Dav1dFrameHeaderSuperRes {
  fn from(value: Rav1dFrameHeaderSuperRes) -> Self {
    let Rav1dFrameHeaderSuperRes {
      width_scale_denominator,
      enabled,
    } = value;
    Self {
      width_scale_denominator,
      enabled: <u8 as ::core::convert::From<::core::primitive::bool>>::from(enabled),
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dFrameHeaderTiling {
  pub uniform: u8,
  pub n_bytes: u8,
  pub min_log2_cols: u8,
  pub max_log2_cols: u8,
  pub log2_cols: u8,
  pub cols: u8,
  pub min_log2_rows: u8,
  pub max_log2_rows: u8,
  pub log2_rows: u8,
  pub rows: u8,
  pub col_start_sb: [u16; DAV1D_MAX_TILE_COLS + 1],
  pub row_start_sb: [u16; DAV1D_MAX_TILE_ROWS + 1],
  pub update: u16,
}

#[derive(Clone)]
#[repr(C)]
pub struct Rav1dFrameHeaderTiling {
  pub uniform: u8,
  pub n_bytes: u8,
  pub min_log2_cols: u8,
  pub max_log2_cols: u8,
  pub log2_cols: u8,
  pub cols: u8,
  pub min_log2_rows: u8,
  pub max_log2_rows: u8,
  pub log2_rows: u8,
  pub rows: u8,
  pub col_start_sb: [u16; RAV1D_MAX_TILE_COLS + 1],
  pub row_start_sb: [u16; RAV1D_MAX_TILE_ROWS + 1],
  pub update: u16,
}

impl Default for Rav1dFrameHeaderTiling {
  fn default() -> Self {
    Self {
      uniform: Default::default(),
      n_bytes: Default::default(),
      min_log2_cols: Default::default(),
      max_log2_cols: Default::default(),
      log2_cols: Default::default(),
      cols: Default::default(),
      min_log2_rows: Default::default(),
      max_log2_rows: Default::default(),
      log2_rows: Default::default(),
      rows: Default::default(),
      col_start_sb: [Default::default(); RAV1D_MAX_TILE_COLS + 1],
      row_start_sb: [Default::default(); RAV1D_MAX_TILE_ROWS + 1],
      update: Default::default(),
    }
  }
}

impl From<Dav1dFrameHeaderTiling> for Rav1dFrameHeaderTiling {
  fn from(value: Dav1dFrameHeaderTiling) -> Self {
    let Dav1dFrameHeaderTiling {
      uniform,
      n_bytes,
      min_log2_cols,
      max_log2_cols,
      log2_cols,
      cols,
      min_log2_rows,
      max_log2_rows,
      log2_rows,
      rows,
      col_start_sb,
      row_start_sb,
      update,
    } = value;
    Self {
      uniform,
      n_bytes,
      min_log2_cols,
      max_log2_cols,
      log2_cols,
      cols,
      min_log2_rows,
      max_log2_rows,
      log2_rows,
      rows,
      col_start_sb,
      row_start_sb,
      update,
    }
  }
}

impl From<Rav1dFrameHeaderTiling> for Dav1dFrameHeaderTiling {
  fn from(value: Rav1dFrameHeaderTiling) -> Self {
    let Rav1dFrameHeaderTiling {
      uniform,
      n_bytes,
      min_log2_cols,
      max_log2_cols,
      log2_cols,
      cols,
      min_log2_rows,
      max_log2_rows,
      log2_rows,
      rows,
      col_start_sb,
      row_start_sb,
      update,
    } = value;
    Self {
      uniform,
      n_bytes,
      min_log2_cols,
      max_log2_cols,
      log2_cols,
      cols,
      min_log2_rows,
      max_log2_rows,
      log2_rows,
      rows,
      col_start_sb,
      row_start_sb,
      update,
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dFrameHeaderQuant {
  pub yac: u8,
  pub ydc_delta: i8,
  pub udc_delta: i8,
  pub uac_delta: i8,
  pub vdc_delta: i8,
  pub vac_delta: i8,
  pub qm: u8,
  pub qm_y: u8,
  pub qm_u: u8,
  pub qm_v: u8,
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dFrameHeaderQuant {
  pub yac: u8,
  pub ydc_delta: i8,
  pub udc_delta: i8,
  pub uac_delta: i8,
  pub vdc_delta: i8,
  pub vac_delta: i8,
  pub qm: u8,
  pub qm_y: u8,
  pub qm_u: u8,
  pub qm_v: u8,
}

impl From<Dav1dFrameHeaderQuant> for Rav1dFrameHeaderQuant {
  fn from(value: Dav1dFrameHeaderQuant) -> Self {
    let Dav1dFrameHeaderQuant {
      yac,
      ydc_delta,
      udc_delta,
      uac_delta,
      vdc_delta,
      vac_delta,
      qm,
      qm_y,
      qm_u,
      qm_v,
    } = value;
    Self {
      yac,
      ydc_delta,
      udc_delta,
      uac_delta,
      vdc_delta,
      vac_delta,
      qm,
      qm_y,
      qm_u,
      qm_v,
    }
  }
}

impl From<Rav1dFrameHeaderQuant> for Dav1dFrameHeaderQuant {
  fn from(value: Rav1dFrameHeaderQuant) -> Self {
    let Rav1dFrameHeaderQuant {
      yac,
      ydc_delta,
      udc_delta,
      uac_delta,
      vdc_delta,
      vac_delta,
      qm,
      qm_y,
      qm_u,
      qm_v,
    } = value;
    Self {
      yac,
      ydc_delta,
      udc_delta,
      uac_delta,
      vdc_delta,
      vac_delta,
      qm,
      qm_y,
      qm_u,
      qm_v,
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dFrameHeaderSegmentation {
  pub enabled: u8,
  pub update_map: u8,
  pub temporal: u8,
  pub update_data: u8,
  pub seg_data: Dav1dSegmentationDataSet,
  pub lossless: [u8; {
    match console_core_number_conversion::const_u8_index(SegmentId::COUNT) {
      Ok(extent) => extent,
    }
  }],
  pub qidx: [u8; {
    match console_core_number_conversion::const_u8_index(SegmentId::COUNT) {
      Ok(extent) => extent,
    }
  }],
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dFrameHeaderSegmentation {
  pub enabled: u8,
  pub update_map: u8,
  pub temporal: u8,
  pub update_data: u8,
  pub seg_data: Rav1dSegmentationDataSet,
  pub lossless: [bool; {
    match console_core_number_conversion::const_u8_index(SegmentId::COUNT) {
      Ok(extent) => extent,
    }
  }],
  pub qidx: [u8; {
    match console_core_number_conversion::const_u8_index(SegmentId::COUNT) {
      Ok(extent) => extent,
    }
  }],
}

impl From<Dav1dFrameHeaderSegmentation> for Rav1dFrameHeaderSegmentation {
  fn from(value: Dav1dFrameHeaderSegmentation) -> Self {
    let Dav1dFrameHeaderSegmentation {
      enabled,
      update_map,
      temporal,
      update_data,
      seg_data,
      lossless,
      qidx,
    } = value;
    Self {
      enabled,
      update_map,
      temporal,
      update_data,
      seg_data:
        <crate::include::dav1d::headers::Rav1dSegmentationDataSet as ::core::convert::From<
          crate::include::dav1d::headers::Dav1dSegmentationDataSet,
        >>::from(seg_data),
      lossless: lossless.map(|e| e != 0),
      qidx,
    }
  }
}

impl From<Rav1dFrameHeaderSegmentation> for Dav1dFrameHeaderSegmentation {
  fn from(value: Rav1dFrameHeaderSegmentation) -> Self {
    let Rav1dFrameHeaderSegmentation {
      enabled,
      update_map,
      temporal,
      update_data,
      seg_data,
      lossless,
      qidx,
    } = value;
    Self {
      enabled,
      update_map,
      temporal,
      update_data,
      seg_data:
        <crate::include::dav1d::headers::Dav1dSegmentationDataSet as ::core::convert::From<
          crate::include::dav1d::headers::Rav1dSegmentationDataSet,
        >>::from(seg_data),
      lossless: lossless.map(|e| <u8 as ::core::convert::From<::core::primitive::bool>>::from(e)),
      qidx,
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dFrameHeaderDeltaQ {
  pub present: u8,
  pub res_log2: u8,
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dFrameHeaderDeltaQ {
  pub present: u8,
  pub res_log2: u8,
}

impl From<Dav1dFrameHeaderDeltaQ> for Rav1dFrameHeaderDeltaQ {
  fn from(value: Dav1dFrameHeaderDeltaQ) -> Self {
    let Dav1dFrameHeaderDeltaQ { present, res_log2 } = value;
    Self { present, res_log2 }
  }
}

impl From<Rav1dFrameHeaderDeltaQ> for Dav1dFrameHeaderDeltaQ {
  fn from(value: Rav1dFrameHeaderDeltaQ) -> Self {
    let Rav1dFrameHeaderDeltaQ { present, res_log2 } = value;
    Self { present, res_log2 }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dFrameHeaderDeltaLF {
  pub present: u8,
  pub res_log2: u8,
  pub multi: u8,
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dFrameHeaderDeltaLF {
  pub present: u8,
  pub res_log2: u8,
  pub multi: u8,
}

impl From<Dav1dFrameHeaderDeltaLF> for Rav1dFrameHeaderDeltaLF {
  fn from(value: Dav1dFrameHeaderDeltaLF) -> Self {
    let Dav1dFrameHeaderDeltaLF {
      present,
      res_log2,
      multi,
    } = value;
    Self {
      present,
      res_log2,
      multi,
    }
  }
}

impl From<Rav1dFrameHeaderDeltaLF> for Dav1dFrameHeaderDeltaLF {
  fn from(value: Rav1dFrameHeaderDeltaLF) -> Self {
    let Rav1dFrameHeaderDeltaLF {
      present,
      res_log2,
      multi,
    } = value;
    Self {
      present,
      res_log2,
      multi,
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dFrameHeaderDelta {
  pub q: Dav1dFrameHeaderDeltaQ,
  pub lf: Dav1dFrameHeaderDeltaLF,
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dFrameHeaderDelta {
  pub q: Rav1dFrameHeaderDeltaQ,
  pub lf: Rav1dFrameHeaderDeltaLF,
}

impl From<Dav1dFrameHeaderDelta> for Rav1dFrameHeaderDelta {
  fn from(value: Dav1dFrameHeaderDelta) -> Self {
    let Dav1dFrameHeaderDelta { q, lf } = value;
    Self {
      q: <crate::include::dav1d::headers::Rav1dFrameHeaderDeltaQ as ::core::convert::From<
        crate::include::dav1d::headers::Dav1dFrameHeaderDeltaQ,
      >>::from(q),
      lf: <crate::include::dav1d::headers::Rav1dFrameHeaderDeltaLF as ::core::convert::From<
        crate::include::dav1d::headers::Dav1dFrameHeaderDeltaLF,
      >>::from(lf),
    }
  }
}

impl From<Rav1dFrameHeaderDelta> for Dav1dFrameHeaderDelta {
  fn from(value: Rav1dFrameHeaderDelta) -> Self {
    let Rav1dFrameHeaderDelta { q, lf } = value;
    Self {
      q: <crate::include::dav1d::headers::Dav1dFrameHeaderDeltaQ as ::core::convert::From<
        crate::include::dav1d::headers::Rav1dFrameHeaderDeltaQ,
      >>::from(q),
      lf: <crate::include::dav1d::headers::Dav1dFrameHeaderDeltaLF as ::core::convert::From<
        crate::include::dav1d::headers::Rav1dFrameHeaderDeltaLF,
      >>::from(lf),
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dFrameHeaderLoopFilter {
  pub level_y: [u8; 2],
  pub level_u: u8,
  pub level_v: u8,
  pub mode_ref_delta_enabled: u8,
  pub mode_ref_delta_update: u8,
  pub mode_ref_deltas: Dav1dLoopfilterModeRefDeltas,
  pub sharpness: u8,
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dFrameHeaderLoopFilter {
  pub level_y: [u8; 2],
  pub level_u: u8,
  pub level_v: u8,
  pub mode_ref_delta_enabled: u8,
  pub mode_ref_delta_update: u8,
  pub mode_ref_deltas: Rav1dLoopfilterModeRefDeltas,
  pub sharpness: u8,
}

impl From<Dav1dFrameHeaderLoopFilter> for Rav1dFrameHeaderLoopFilter {
  fn from(value: Dav1dFrameHeaderLoopFilter) -> Self {
    let Dav1dFrameHeaderLoopFilter {
      level_y,
      level_u,
      level_v,
      mode_ref_delta_enabled,
      mode_ref_delta_update,
      mode_ref_deltas,
      sharpness,
    } = value;
    Self {
      level_y,
      level_u,
      level_v,
      mode_ref_delta_enabled,
      mode_ref_delta_update,
      mode_ref_deltas:
        <crate::include::dav1d::headers::Rav1dLoopfilterModeRefDeltas as ::core::convert::From<
          crate::include::dav1d::headers::Dav1dLoopfilterModeRefDeltas,
        >>::from(mode_ref_deltas),
      sharpness,
    }
  }
}

impl From<Rav1dFrameHeaderLoopFilter> for Dav1dFrameHeaderLoopFilter {
  fn from(value: Rav1dFrameHeaderLoopFilter) -> Self {
    let Rav1dFrameHeaderLoopFilter {
      level_y,
      level_u,
      level_v,
      mode_ref_delta_enabled,
      mode_ref_delta_update,
      mode_ref_deltas,
      sharpness,
    } = value;
    Self {
      level_y,
      level_u,
      level_v,
      mode_ref_delta_enabled,
      mode_ref_delta_update,
      mode_ref_deltas:
        <crate::include::dav1d::headers::Dav1dLoopfilterModeRefDeltas as ::core::convert::From<
          crate::include::dav1d::headers::Rav1dLoopfilterModeRefDeltas,
        >>::from(mode_ref_deltas),
      sharpness,
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dFrameHeaderCdef {
  pub damping: u8,
  pub n_bits: u8,
  pub y_strength: [u8; DAV1D_MAX_CDEF_STRENGTHS],
  pub uv_strength: [u8; DAV1D_MAX_CDEF_STRENGTHS],
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dFrameHeaderCdef {
  pub damping: u8,
  pub n_bits: u8,
  pub y_strength: [u8; RAV1D_MAX_CDEF_STRENGTHS],
  pub uv_strength: [u8; RAV1D_MAX_CDEF_STRENGTHS],
}

impl From<Dav1dFrameHeaderCdef> for Rav1dFrameHeaderCdef {
  fn from(value: Dav1dFrameHeaderCdef) -> Self {
    let Dav1dFrameHeaderCdef {
      damping,
      n_bits,
      y_strength,
      uv_strength,
    } = value;
    Self {
      damping,
      n_bits,
      y_strength,
      uv_strength,
    }
  }
}

impl From<Rav1dFrameHeaderCdef> for Dav1dFrameHeaderCdef {
  fn from(value: Rav1dFrameHeaderCdef) -> Self {
    let Rav1dFrameHeaderCdef {
      damping,
      n_bits,
      y_strength,
      uv_strength,
    } = value;
    Self {
      damping,
      n_bits,
      y_strength,
      uv_strength,
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dFrameHeaderRestoration {
  pub r#type: [Dav1dRestorationType; 3],
  pub unit_size: [u8; 2],
}

#[derive(Clone, Default)]
#[repr(C)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "Rust callers write typed restoration choices and only incoming raw C tags can fail conversion"
  )
)]
pub struct Rav1dFrameHeaderRestoration {
  pub r#type: [Rav1dRestorationType; 3],
  pub unit_size: [u8; 2],
}

impl TryFrom<Dav1dFrameHeaderRestoration> for Rav1dFrameHeaderRestoration {
  type Error = Rav1dError;

  fn try_from(value: Dav1dFrameHeaderRestoration) -> Rav1dResult<Self> {
    let [luma_result, blue_result, red_result] = value.r#type.map(|code| {
      Rav1dRestorationType::from_repr(match console_core_number_conversion::index(code) {
        Ok(native_code) => native_code,
      })
      .ok_or(Rav1dError::EINVAL)
    });
    let luma = luma_result?;
    let blue = blue_result?;
    let red = red_result?;

    Ok(Self {
      r#type: [luma, blue, red],
      unit_size: value.unit_size,
    })
  }
}

impl From<Rav1dFrameHeaderRestoration> for Dav1dFrameHeaderRestoration {
  fn from(value: Rav1dFrameHeaderRestoration) -> Self {
    let Rav1dFrameHeaderRestoration { r#type, unit_size } = value;
    Self {
      r#type: r#type.map(|e| e.to_repr()),
      unit_size,
    }
  }
}

#[derive(Clone)]
#[repr(C)]
pub struct Dav1dFrameHeader {
  pub film_grain: Dav1dFrameHeaderFilmGrain,
  pub frame_type: Dav1dFrameType,
  pub width: [c_int; 2],
  pub height: c_int,
  pub frame_offset: u8,
  pub temporal_id: u8,
  pub spatial_id: u8,
  pub show_existing_frame: u8,
  pub existing_frame_idx: u8,
  pub frame_id: u32,
  pub frame_presentation_delay: u32,
  pub show_frame: u8,
  pub showable_frame: u8,
  pub error_resilient_mode: u8,
  pub disable_cdf_update: u8,
  pub allow_screen_content_tools: u8,
  pub force_integer_mv: u8,
  pub frame_size_override: u8,
  pub primary_ref_frame: u8,
  pub buffer_removal_time_present: u8,
  pub operating_points: [Dav1dFrameHeaderOperatingPoint; DAV1D_MAX_OPERATING_POINTS],
  pub refresh_frame_flags: u8,
  pub render_width: c_int,
  pub render_height: c_int,
  pub super_res: Dav1dFrameHeaderSuperRes,
  pub have_render_size: u8,
  pub allow_intrabc: u8,
  pub frame_ref_short_signaling: u8,
  pub refidx: [i8; DAV1D_REFS_PER_FRAME],
  pub hp: u8,
  pub subpel_filter_mode: Dav1dFilterMode,
  pub switchable_motion_mode: u8,
  pub use_ref_frame_mvs: u8,
  pub refresh_context: u8,
  pub tiling: Dav1dFrameHeaderTiling,
  pub quant: Dav1dFrameHeaderQuant,
  pub segmentation: Dav1dFrameHeaderSegmentation,
  pub delta: Dav1dFrameHeaderDelta,
  pub all_lossless: u8,
  pub loopfilter: Dav1dFrameHeaderLoopFilter,
  pub cdef: Dav1dFrameHeaderCdef,
  pub restoration: Dav1dFrameHeaderRestoration,
  pub txfm_mode: Dav1dTxfmMode,
  pub switchable_comp_refs: u8,
  pub skip_mode_allowed: u8,
  pub skip_mode_enabled: u8,
  pub skip_mode_refs: [i8; 2],
  pub warp_motion: u8,
  pub reduced_txtp_set: u8,
  pub gmv: [Dav1dWarpedMotionParams; DAV1D_REFS_PER_FRAME],
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dFrameSize {
  pub width: [c_int; 2],
  pub height: c_int,
  pub render_width: c_int,
  pub render_height: c_int,
  pub super_res: Rav1dFrameHeaderSuperRes,
  pub have_render_size: u8,
}

#[derive(Clone, Default)]
#[repr(C)]
pub struct Rav1dFrameSkipMode {
  pub allowed: u8,
  pub enabled: u8,
  pub refs: [i8; 2],
}

#[derive(Clone, Default)]
#[repr(C)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "the frame record retains individual bitstream flags whose constraints also depend on the active sequence and reference frames"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "the decoder creates and updates typed frame fields while the C conversion separately validates its numeric tags"
  )
)]
pub struct Rav1dFrameHeader {
  pub size: Rav1dFrameSize,
  pub film_grain: Rav1dFrameHeaderFilmGrain,
  pub frame_type: Rav1dFrameType,
  pub frame_offset: u8,
  pub temporal_id: u8,
  pub spatial_id: u8,
  pub show_existing_frame: u8,
  pub existing_frame_idx: u8,
  pub frame_id: u32,
  pub frame_presentation_delay: u32,
  pub show_frame: u8,
  pub showable_frame: u8,
  pub error_resilient_mode: u8,
  pub disable_cdf_update: u8,
  pub allow_screen_content_tools: bool,
  pub force_integer_mv: bool,
  pub frame_size_override: bool,
  pub primary_ref_frame: u8,
  pub buffer_removal_time_present: u8,
  pub operating_points: [Rav1dFrameHeaderOperatingPoint; RAV1D_MAX_OPERATING_POINTS],
  pub refresh_frame_flags: u8,
  pub allow_intrabc: bool,
  pub frame_ref_short_signaling: u8,
  pub refidx: [i8; RAV1D_REFS_PER_FRAME],
  pub hp: bool,
  pub subpel_filter_mode: Rav1dFilterMode,
  pub switchable_motion_mode: u8,
  pub use_ref_frame_mvs: u8,
  pub refresh_context: u8,
  pub tiling: Rav1dFrameHeaderTiling,
  pub quant: Rav1dFrameHeaderQuant,
  pub segmentation: Rav1dFrameHeaderSegmentation,
  pub delta: Rav1dFrameHeaderDelta,
  pub all_lossless: bool,
  pub loopfilter: Rav1dFrameHeaderLoopFilter,
  pub cdef: Rav1dFrameHeaderCdef,
  pub restoration: Rav1dFrameHeaderRestoration,
  pub txfm_mode: Rav1dTxfmMode,
  pub switchable_comp_refs: u8,
  pub skip_mode: Rav1dFrameSkipMode,
  pub warp_motion: u8,
  pub reduced_txtp_set: u8,
  pub gmv: [Rav1dWarpedMotionParams; RAV1D_REFS_PER_FRAME],
}

impl TryFrom<Dav1dFrameHeader> for Rav1dFrameHeader {
  type Error = Rav1dError;

  fn try_from(value: Dav1dFrameHeader) -> Rav1dResult<Self> {
    let frame_type = checked_header_field(value.frame_type)?;
    let subpel_filter_mode = checked_header_field(value.subpel_filter_mode)?;
    let txfm_mode = checked_header_field(value.txfm_mode)?;
    let restoration = Rav1dFrameHeaderRestoration::try_from(value.restoration)?;
    let gmv = checked_global_motion(value.gmv)?;

    Ok(Self {
      size: Rav1dFrameSize {
        width: value.width,
        height: value.height,
        render_width: value.render_width,
        render_height: value.render_height,
        super_res: Rav1dFrameHeaderSuperRes::from(value.super_res),
        have_render_size: value.have_render_size,
      },
      film_grain: Rav1dFrameHeaderFilmGrain::from(value.film_grain),
      frame_type,
      frame_offset: value.frame_offset,
      temporal_id: value.temporal_id,
      spatial_id: value.spatial_id,
      show_existing_frame: value.show_existing_frame,
      existing_frame_idx: value.existing_frame_idx,
      frame_id: value.frame_id,
      frame_presentation_delay: value.frame_presentation_delay,
      show_frame: value.show_frame,
      showable_frame: value.showable_frame,
      error_resilient_mode: value.error_resilient_mode,
      disable_cdf_update: value.disable_cdf_update,
      allow_screen_content_tools: value.allow_screen_content_tools != 0,
      force_integer_mv: value.force_integer_mv != 0,
      frame_size_override: value.frame_size_override != 0,
      primary_ref_frame: value.primary_ref_frame,
      buffer_removal_time_present: value.buffer_removal_time_present,
      operating_points: value
        .operating_points
        .map(Rav1dFrameHeaderOperatingPoint::from),
      refresh_frame_flags: value.refresh_frame_flags,
      allow_intrabc: value.allow_intrabc != 0,
      frame_ref_short_signaling: value.frame_ref_short_signaling,
      refidx: value.refidx,
      hp: value.hp != 0,
      subpel_filter_mode,
      switchable_motion_mode: value.switchable_motion_mode,
      use_ref_frame_mvs: value.use_ref_frame_mvs,
      refresh_context: value.refresh_context,
      tiling: Rav1dFrameHeaderTiling::from(value.tiling),
      quant: Rav1dFrameHeaderQuant::from(value.quant),
      segmentation: Rav1dFrameHeaderSegmentation::from(value.segmentation),
      delta: Rav1dFrameHeaderDelta::from(value.delta),
      all_lossless: value.all_lossless != 0,
      loopfilter: Rav1dFrameHeaderLoopFilter::from(value.loopfilter),
      cdef: Rav1dFrameHeaderCdef::from(value.cdef),
      restoration,
      txfm_mode,
      switchable_comp_refs: value.switchable_comp_refs,
      skip_mode: Rav1dFrameSkipMode {
        allowed: value.skip_mode_allowed,
        enabled: value.skip_mode_enabled,
        refs: value.skip_mode_refs,
      },
      warp_motion: value.warp_motion,
      reduced_txtp_set: value.reduced_txtp_set,
      gmv,
    })
  }
}

fn checked_global_motion(
  wire: [Dav1dWarpedMotionParams; DAV1D_REFS_PER_FRAME],
) -> Rav1dResult<[Rav1dWarpedMotionParams; RAV1D_REFS_PER_FRAME]> {
  let mut output = std::array::from_fn(|_slot| Rav1dWarpedMotionParams::default());

  for (destination, packed) in output.iter_mut().zip(wire) {
    let motion = checked_header_field(packed)?;
    *destination = motion;
  }

  Ok(output)
}

impl From<Rav1dFrameHeader> for Dav1dFrameHeader {
  fn from(value: Rav1dFrameHeader) -> Self {
    Self {
      film_grain: Dav1dFrameHeaderFilmGrain::from(value.film_grain),
      frame_type: u32::from(value.frame_type),
      width: value.size.width,
      height: value.size.height,
      frame_offset: value.frame_offset,
      temporal_id: value.temporal_id,
      spatial_id: value.spatial_id,
      show_existing_frame: value.show_existing_frame,
      existing_frame_idx: value.existing_frame_idx,
      frame_id: value.frame_id,
      frame_presentation_delay: value.frame_presentation_delay,
      show_frame: value.show_frame,
      showable_frame: value.showable_frame,
      error_resilient_mode: value.error_resilient_mode,
      disable_cdf_update: value.disable_cdf_update,
      allow_screen_content_tools: u8::from(value.allow_screen_content_tools),
      force_integer_mv: u8::from(value.force_integer_mv),
      frame_size_override: u8::from(value.frame_size_override),
      primary_ref_frame: value.primary_ref_frame,
      buffer_removal_time_present: value.buffer_removal_time_present,
      operating_points: value
        .operating_points
        .map(Dav1dFrameHeaderOperatingPoint::from),
      refresh_frame_flags: value.refresh_frame_flags,
      render_width: value.size.render_width,
      render_height: value.size.render_height,
      super_res: Dav1dFrameHeaderSuperRes::from(value.size.super_res),
      have_render_size: value.size.have_render_size,
      allow_intrabc: u8::from(value.allow_intrabc),
      frame_ref_short_signaling: value.frame_ref_short_signaling,
      refidx: value.refidx,
      hp: u8::from(value.hp),
      subpel_filter_mode: u8::from(value.subpel_filter_mode),
      switchable_motion_mode: value.switchable_motion_mode,
      use_ref_frame_mvs: value.use_ref_frame_mvs,
      refresh_context: value.refresh_context,
      tiling: Dav1dFrameHeaderTiling::from(value.tiling),
      quant: Dav1dFrameHeaderQuant::from(value.quant),
      segmentation: Dav1dFrameHeaderSegmentation::from(value.segmentation),
      delta: Dav1dFrameHeaderDelta::from(value.delta),
      all_lossless: u8::from(value.all_lossless),
      loopfilter: Dav1dFrameHeaderLoopFilter::from(value.loopfilter),
      cdef: Dav1dFrameHeaderCdef::from(value.cdef),
      restoration: Dav1dFrameHeaderRestoration::from(value.restoration),
      txfm_mode: u32::from(value.txfm_mode),
      switchable_comp_refs: value.switchable_comp_refs,
      skip_mode_allowed: value.skip_mode.allowed,
      skip_mode_enabled: value.skip_mode.enabled,
      skip_mode_refs: value.skip_mode.refs,
      warp_motion: value.warp_motion,
      reduced_txtp_set: value.reduced_txtp_set,
      gmv: value.gmv.map(Dav1dWarpedMotionParams::from),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn frame_header_round_trip_checks_frame_and_motion_tags() -> Rav1dResult {
    let mut frame = Rav1dFrameHeader {
      size: Rav1dFrameSize {
        width: [144, 160],
        height: 96,
        ..Default::default()
      },
      frame_type: Rav1dFrameType::Inter,
      allow_screen_content_tools: true,
      subpel_filter_mode: Rav1dFilterMode::Sharp8Tap,
      txfm_mode: Rav1dTxfmMode::Switchable,
      ..Default::default()
    };
    let [motion, ..] = &mut frame.gmv;
    motion.r#type = Rav1dWarpedMotionType::Affine;
    motion.matrix = [32, -16, 0x10000, 64, -32, 0x10000];
    let Ok(()) = motion.abcd.set([64, 32, -32, 0]);

    let mut wire = Dav1dFrameHeader::from(frame);
    let round_trip = Rav1dFrameHeader::try_from(wire.clone())?;
    assert!(round_trip.frame_type == Rav1dFrameType::Inter);
    assert_eq!(round_trip.size.width, [144, 160]);
    assert_eq!(round_trip.size.height, 96);
    assert!(round_trip.allow_screen_content_tools);
    assert_eq!(round_trip.subpel_filter_mode, Rav1dFilterMode::Sharp8Tap);
    assert!(round_trip.txfm_mode == Rav1dTxfmMode::Switchable);
    let [decoded_motion, ..] = &round_trip.gmv;
    assert!(decoded_motion.r#type == Rav1dWarpedMotionType::Affine);
    assert_eq!(decoded_motion.matrix, [32, -16, 0x10000, 64, -32, 0x10000]);
    assert_eq!(decoded_motion.abcd.get(), Ok([64, 32, -32, 0]));

    let [wire_motion, ..] = &mut wire.gmv;
    wire_motion.r#type = u32::MAX;
    assert!(matches!(
      Rav1dFrameHeader::try_from(wire.clone()),
      Err(Rav1dError::EINVAL)
    ));
    wire.frame_type = u32::MAX;
    assert!(matches!(
      Rav1dFrameHeader::try_from(wire),
      Err(Rav1dError::EINVAL)
    ));

    Ok(())
  }

  #[test]
  fn restoration_header_round_trip_checks_each_plane_tag() -> Rav1dResult {
    let original = Rav1dFrameHeaderRestoration {
      r#type: [
        Rav1dRestorationType::None,
        Rav1dRestorationType::Wiener,
        Rav1dRestorationType::SgrProj(SgrIdx::I0),
      ],
      unit_size: [6, 5],
    };
    let mut wire = Dav1dFrameHeaderRestoration::from(original.clone());
    let round_trip = Rav1dFrameHeaderRestoration::try_from(wire.clone())?;
    assert_eq!(round_trip.r#type, original.r#type);
    assert_eq!(round_trip.unit_size, original.unit_size);

    let [_, invalid, _] = &mut wire.r#type;
    *invalid = u8::MAX;
    assert!(matches!(
      Rav1dFrameHeaderRestoration::try_from(wire),
      Err(Rav1dError::EINVAL)
    ));

    Ok(())
  }

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "this fixture verifies that the rejected transition returns the original shared ownership"
    )
  )]
  fn metadata_freeze_preserves_shared_owner_and_payload() -> Rav1dResult {
    let payload = [0x35_u8, 0x02, 0x44, 0x80];
    let mut pending = Arc::new(Mutex::new(vec![Rav1dITUTT35 {
      country_code: 145,
      country_code_extension_byte: 2,
      payload: Box::from(payload),
    }]));
    let other_owner = Arc::clone(&pending);

    assert!(matches!(
      Rav1dITUTT35::to_immut(&mut pending),
      Err(Rav1dError::EGeneric)
    ));
    assert!(Arc::ptr_eq(&pending, &other_owner));
    drop(other_owner);

    let frozen = Rav1dITUTT35::to_immut(&mut pending)?;
    let (packet, wire) = match (frozen.rav1d.as_ref(), frozen.dav1d.as_ref()) {
      ([packet], [wire]) => (packet, wire),
      _unexpected_packet_count => return Err(Rav1dError::EGeneric),
    };
    assert_eq!(packet.country_code, 145);
    assert_eq!(packet.country_code_extension_byte, 2);
    assert_eq!(packet.payload.as_ref(), &payload);
    assert_eq!(wire.country_code, packet.country_code);
    assert_eq!(
      wire.country_code_extension_byte,
      packet.country_code_extension_byte
    );

    #[cfg(feature = "c-ffi")]
    {
      assert_eq!(wire.payload_size, payload.len());
      assert_eq!(wire.payload.0, packet.payload.as_ptr());
    }

    #[cfg(not(feature = "c-ffi"))]
    assert_eq!(wire.payload.as_ref(), &payload);

    Ok(())
  }
}

#![forbid(unsafe_code)]
use crate::include::common::intops::clip;
use crate::src::align::Align8;
use crate::src::align::Align16;
use crate::src::align::ArrayDefault;
use std::ffi::c_int;
use std::ffi::c_uint;
use std::ffi::c_void;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::ops::Add;
use std::ops::Div;
use std::ops::Mul;
use std::ops::Rem;
use std::ops::Shr;
use zerocopy::FromBytes;
use zerocopy::IntoBytes;

pub trait FromPrimitive<T> {
  fn from_prim(t: T) -> Self;
}

pub trait ToPrimitive<T> {
  fn to_prim(self) -> T;
}

pub trait AsPrimitive {
  fn as_<T>(self) -> T
  where
    Self: ToPrimitive<T>;
}

impl<T, U> ToPrimitive<U> for T
where
  U: FromPrimitive<T>,
{
  fn to_prim(self) -> U {
    FromPrimitive::from_prim(self)
  }
}

impl<U> AsPrimitive for U {
  fn as_<T>(self) -> T
  where
    Self: ToPrimitive<T>,
  {
    self.to_prim()
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(explicit051_no_machine_width, reason = "the numeric adapters include the machine-native slice size and offset conversions")
)]
mod primitive_casts {
  use super::FromPrimitive;

  #[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(explicit080_macro_budget, reason = "the primitive adapter uses one uniform conversion rule for each source and destination type")
  )]
  macro_rules! impl_FromPrimitive {
      ($T:ty => $U:ty) => {
          impl FromPrimitive<$T> for $U {
              #[cfg_attr(
                dylint_lib = "explicit011_no_as_cast",
                expect(explicit011_no_as_cast, reason = "this primitive adapter deliberately preserves Rust cast semantics for every numeric pair")
              )]
              fn from_prim(t: $T) -> $U {
                  t as $U
              }
          }
      };
      ($T:ty => {$($U:ty),*}) => {
          $(impl_FromPrimitive!($T => $U);)*
      };
      ($T:ty => {$($U:ty),*, ...}) => {
          $(impl_FromPrimitive!($T => $U);)*
          impl_FromPrimitive!($T => {u8, u16, u32, u64, u128, usize});
          impl_FromPrimitive!($T => {i8, i16, i32, i64, i128, isize});
          impl_FromPrimitive!($T => {f32, f64});
      };
  }

  impl_FromPrimitive!(u8 => {char, ...});
  impl_FromPrimitive!(u16 => {, ...});
  impl_FromPrimitive!(u32 => {, ...});
  impl_FromPrimitive!(u64 => {, ...});
  impl_FromPrimitive!(u128 => {, ...});
  impl_FromPrimitive!(usize => {, ...});

  impl_FromPrimitive!(i8 => {, ...});
  impl_FromPrimitive!(i16 => {, ...});
  impl_FromPrimitive!(i32 => {, ...});
  impl_FromPrimitive!(i64 => {, ...});
  impl_FromPrimitive!(i128 => {, ...});
  impl_FromPrimitive!(isize => {, ...});

  impl_FromPrimitive!(f32 => {, ...});
  impl_FromPrimitive!(f64 => {, ...});
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BPC {
  BPC8,
  BPC16,
}

impl BPC {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the backend selects its existing bit-depth enum directly from the codec pixel bound")
  )]
  pub fn from_bitdepth_max(bitdepth_max: c_int) -> Self {
    match bitdepth_max {
      255 => Self::BPC8,
      _ => Self::BPC16,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the const mask sizes and DSP interfaces use this fixed bit count directly")
  )]
  pub const fn bitdepth(&self) -> u8 {
    match self {
      Self::BPC8 => 8,
      Self::BPC16 => 16,
    }
  }

  #[doc = ""]
  #[doc = "`T` is generally meant to be `usize` or `isize`."]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "coefficient and pixel row addressing uses the existing scalar output of this stride adapter")
  )]
  pub fn pxstride<T>(&self, n: T) -> T
  where
    T: Copy + Eq + From<u8> + Div<Output = T> + Rem<Output = T>,
  {
    let scale = T::from(match self {
      Self::BPC8 => 1,
      Self::BPC16 => 2,
    });
    assert!(n % scale == T::from(0), "pixel stride must be a whole number of samples");
    n / scale
  }

  #[doc = ""]
  #[doc = "`T` is generally meant to be `usize` or `isize`."]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "coefficient row addressing uses the existing scalar output of this stride adapter")
  )]
  pub fn coef_stride<T>(&self, n: T) -> T
  where
    T: Copy + Eq + From<u8> + Div<Output = T> + Rem<Output = T>,
  {
    let scale = T::from(match self {
      Self::BPC8 => 2,
      Self::BPC16 => 4,
    });
    assert!(n % scale == T::from(0), "coefficient stride must be a whole number of samples");
    n / scale
  }
}

pub trait BitDepth: Clone + Copy {
  const BPC: BPC;
  const BITDEPTH: u8 = Self::BPC.bitdepth();

  type Pixel: Copy
    + Ord
    + Add<Output = Self::Pixel>
    + Mul<Output = Self::Pixel>
    + Shr<u8, Output = Self::Pixel>
    + From<u8>
    + Into<u16>
    + Into<i32>
    + TryFrom<i32>
    + FromPrimitive<u16>
    + FromPrimitive<c_int>
    + FromPrimitive<c_uint>
    + ToPrimitive<i16>
    + ToPrimitive<c_int>
    + ToPrimitive<c_uint>
    + FromBytes
    + IntoBytes
    + zerocopy::KnownLayout
    + zerocopy::Immutable;

  type AlignPixelX8: Copy;

  type Coef: Copy
    + From<i16>
    + Into<i32>
    + FromPrimitive<i16>
    + FromPrimitive<u16>
    + FromPrimitive<c_int>
    + FromPrimitive<c_uint>
    + ToPrimitive<c_int>
    + ToPrimitive<c_uint>
    + Add<Output = Self::Coef>
    + FromBytes
    + IntoBytes
    + zerocopy::KnownLayout
    + zerocopy::Immutable
    + Display;

  type Entry: Copy
    + Default
    + ArrayDefault
    + FromPrimitive<i16>
    + FromPrimitive<c_int>
    + ToPrimitive<c_int>
    + IntoBytes
    + FromBytes
    + zerocopy::KnownLayout
    + zerocopy::Immutable;

  type Scaling: AsRef<[u8]> + AsMut<[u8]> + ArrayDefault + Copy;
  const SCALING_BITS: u32;
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(explicit051_no_machine_width, reason = "this associated constant is the Rust array length of the per-depth scaling lookup")
  )]
  const SCALING_SIZE: usize = 1_usize.wrapping_shl(Self::SCALING_BITS);

  type BitDepthMax;

  type DisplayPixel: Display;

  fn new(bitdepth_max: Self::BitDepthMax) -> Self;

  #[doc = "construct a [`BitDepth`], [`BitDepth::from_c`] is a uniform way to"]
  #[doc = "construct a [`BitDepth`] from its C representation, a [`c_int`]."]
  #[doc = ""]
  #[doc = "Since [`BitDepth`]-dependent `fn` ptr types use type erasure, they"]
  #[doc = "always pass the `bitdepth_max` last argument (even for `8bpc``fn`s"]
  #[doc = "where it's superfluous (it's constant)), so we need to convert from"]
  #[doc = "that `bitdepth_max: c_int` arg back to a [`BitDepth`]."]
  fn from_c(bitdepth_max: c_int) -> Self;

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the public DSP erasure converts each bit depth directly to its existing C bound")
  )]
  fn into_c(self) -> c_int {
    <Self::Pixel as Into<c_int>>::into(self.bitdepth_max())
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the existing DSP copy adapter receives the already checked native slice span")
  )]
  #[cfg_attr(
    dylint_lib = "explicit004_no_panic",
    expect(explicit004_no_panic, reason = "the existing DSP copy contract refuses a disputed slice span before changing the destination")
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(explicit051_no_machine_width, reason = "the DSP copy receives an existing native Rust slice length")
  )]
  #[cfg_attr(
    dylint_lib = "explicit056_no_hidden_panic_api",
    expect(
      explicit056_no_hidden_panic_api,
      reason = "both prefixes have the requested length and equality is checked before the native slice copy"
    )
  )]
  fn pixel_copy(dest: &mut [Self::Pixel], src: &[Self::Pixel], n: usize) {
    match (dest.get_mut(..n), src.get(..n)) {
      (Some(destination), Some(source)) => {
        assert!(destination.len() == source.len());
        destination.copy_from_slice(source);
      }
      _outside_rows => panic!("pixel copy exceeds the source or destination span"),
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the existing DSP fill adapter receives the already checked native slice span")
  )]
  #[cfg_attr(
    dylint_lib = "explicit004_no_panic",
    expect(explicit004_no_panic, reason = "the existing DSP fill contract refuses a disputed slice span before changing the destination")
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(explicit051_no_machine_width, reason = "the DSP fill receives an existing native Rust slice length")
  )]
  fn pixel_set(dest: &mut [Self::Pixel], val: Self::Pixel, n: usize) {
    match dest.get_mut(..n) {
      Some(destination) => destination.fill(val),
      None => panic!("pixel fill exceeds the destination span"),
    }
  }

  fn display(pixel: Self::Pixel) -> Self::DisplayPixel;

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the clipping primitive supplies a bounded pixel directly to the existing DSP arithmetic")
  )]
  fn iclip_pixel<T>(&self, pixel: T) -> Self::Pixel
  where
    T: Copy + Ord + TryInto<Self::Pixel> + ToPrimitive<Self::Pixel>,
    Self::Pixel: Into<T>,
  {
    clip(pixel, Self::Pixel::from(0_u8), self.bitdepth_max())
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the DSP trait exposes the existing scalar stride result for checked picture layouts")
  )]
  fn pxstride<T>(n: T) -> T
  where
    T: Copy + Eq + From<u8> + Div<Output = T> + Rem<Output = T>,
  {
    Self::BPC.pxstride(n)
  }

  fn bitdepth(&self) -> u8;

  fn bitdepth_max(&self) -> Self::Pixel;

  fn get_intermediate_bits(&self) -> u8;

  const PREP_BIAS: i16;

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the fixed bias is used directly within established fixed-point sample arithmetic")
  )]
  fn sub_prep_bias(pixel: i32) -> i16 {
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(
      ::core::primitive::i32::wrapping_sub(pixel, i32::from(Self::PREP_BIAS)),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    }
  }
}

#[derive(Clone, Copy)]
pub struct BitDepth8 {
  bitdepth_max: <Self as BitDepth>::BitDepthMax,
}

impl BitDepth for BitDepth8 {
  const BPC: BPC = BPC::BPC8;

  type Pixel = u8;

  type AlignPixelX8 = Align8<[Self::Pixel; 0]>;

  type Coef = i16;

  type Entry = i8;

  type Scaling = [u8; Self::SCALING_SIZE];
  const SCALING_BITS: u32 = 8;

  type BitDepthMax = ();

  type DisplayPixel = DisplayPixel8;

  fn new(bitdepth_max: Self::BitDepthMax) -> Self {
    Self { bitdepth_max }
  }

  fn from_c(_bitdepth_max: c_int) -> Self {
    Self::new(())
  }

  fn display(pixel: Self::Pixel) -> Self::DisplayPixel {
    DisplayPixel8(pixel)
  }

  fn bitdepth(&self) -> u8 {
    Self::BITDEPTH
  }

  fn bitdepth_max(&self) -> Self::Pixel {
    255
  }

  fn get_intermediate_bits(&self) -> u8 {
    4
  }

  const PREP_BIAS: i16 = 0;
}

#[derive(Clone, Copy)]
pub struct BitDepth16 {
  bitdepth_max: <Self as BitDepth>::BitDepthMax,
}

impl BitDepth for BitDepth16 {
  const BPC: BPC = BPC::BPC16;

  type Pixel = u16;

  type AlignPixelX8 = Align16<[Self::Pixel; 0]>;

  type Coef = i32;

  type Entry = i16;

  type Scaling = [u8; Self::SCALING_SIZE];
  const SCALING_BITS: u32 = 12;

  type BitDepthMax = Self::Pixel;

  type DisplayPixel = DisplayPixel16;

  fn new(bitdepth_max: Self::BitDepthMax) -> Self {
    Self { bitdepth_max }
  }

  fn from_c(bitdepth_max: c_int) -> Self {
    Self::new(bitdepth_max.as_())
  }

  fn display(pixel: Self::Pixel) -> Self::DisplayPixel {
    DisplayPixel16(pixel)
  }

  fn bitdepth(&self) -> u8 {
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
      ::core::primitive::u32::wrapping_sub(Self::Pixel::BITS, self.bitdepth_max.leading_zeros()),
    ) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    }
  }

  fn bitdepth_max(&self) -> Self::Pixel {
    self.bitdepth_max
  }

  #[doc = "- 2 for 12 bits/component."]
  fn get_intermediate_bits(&self) -> u8 {
    ::core::primitive::u8::wrapping_sub(14, self.bitdepth())
  }

  #[doc = "Subtract a bias to ensure the output fits in [`i16`]."]
  const PREP_BIAS: i16 = 8192;
}

pub struct DisplayPixel8(<BitDepth8 as BitDepth>::Pixel);

impl Display for DisplayPixel8 {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    write!(f, "{:02x}", self.0)
  }
}

pub struct DisplayPixel16(<BitDepth16 as BitDepth>::Pixel);

impl Display for DisplayPixel16 {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    write!(f, "{:03x}", self.0)
  }
}

#[repr(transparent)]
pub struct DynPixel(c_void);

#[repr(transparent)]
pub struct DynCoef(c_void);

#[repr(transparent)]
pub struct DynEntry(c_void);

#[repr(transparent)]
pub struct DynScaling([u8; 1]);

pub type LeftPixelRow<Pixel> = [Pixel; 4];
pub type LeftPixelRow2px<Pixel> = [Pixel; 2];

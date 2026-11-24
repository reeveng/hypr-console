//! A font file is big-endian bytes, and a scalar is a value written as a fixed run of them.
//!
//! A table borrowed out of a font is a run of [`BigEndian`] values: the bytes as the file holds
//! them, decoded when [`BigEndian::get`] is asked and not before, so a table can be laid over the
//! file without being copied out of it. [`Scalar`] is the decoding and the encoding, and
//! [`FixedSize`] is how many bytes a value takes in the file, which is not always its size in
//! memory -- an [`crate::Offset24`] is three bytes there and four here.
//!
//! The widths are the ones a font writes a scalar in -- one, two, three, four and eight bytes, and
//! sixteen for the identifier an incrementally transferred font is matched by -- and each is a byte
//! array whose length is said as a `u64`, because it is a fact about the file format rather than
//! about the machine. Which arrays those are is sealed: the only way to be a scalar is to be one of
//! them, and upstream's any-length impl is gone because a width nothing writes is not one to offer. What `BigEndian` compares by is the value rather than the bytes, because a
//! signed value's bytes do not sort the way it does.

use console_core_never::Never;

pub trait Scalar: Sized {
  type Raw: sealed::BeByteArray;

  fn from_raw(raw: Self::Raw) -> Result<Self, Never>;

  fn to_raw(self) -> Result<Self::Raw, Never>;

  fn read(slice: &[u8]) -> Result<Option<Self>, Never> {
    let Ok(raw) = <Self::Raw as sealed::BeByteArray>::from_slice(slice);

    Ok(raw.map(|raw_2| match Self::from_raw(raw_2) {
      Ok(value) => value,
    }))
  }
}

pub trait FixedSize: Sized {
  const RAW_BYTE_LEN: u64;
}

mod sealed {
  use console_core_never::Never;

  #[cfg(not(feature = "bytemuck"))]
  pub trait BeByteArray: Copy + AsRef<[u8]> {
    const LEN: u64;

    fn from_slice(slice: &[u8]) -> Result<Option<Self>, Never>;
  }

  #[cfg(feature = "bytemuck")]
  pub trait BeByteArray:
    Copy + AsRef<[u8]> + bytemuck::AnyBitPattern + bytemuck::Zeroable
  {
    const LEN: u64;

    fn from_slice(slice: &[u8]) -> Result<Option<Self>, Never>;
  }

  fn exactly<const N: usize>(slice: &[u8]) -> Result<Option<[u8; N]>, Never> {
    Ok(match slice.as_chunks::<N>() {
      ([whole], []) => Some(*whole),
      (_, _) => None,
    })
  }

  impl BeByteArray for [u8; 1] {
    const LEN: u64 = 1;

    fn from_slice(slice: &[u8]) -> Result<Option<Self>, Never> {
      exactly(slice)
    }
  }

  impl BeByteArray for [u8; 2] {
    const LEN: u64 = 2;

    fn from_slice(slice: &[u8]) -> Result<Option<Self>, Never> {
      exactly(slice)
    }
  }

  impl BeByteArray for [u8; 3] {
    const LEN: u64 = 3;

    fn from_slice(slice: &[u8]) -> Result<Option<Self>, Never> {
      exactly(slice)
    }
  }

  impl BeByteArray for [u8; 4] {
    const LEN: u64 = 4;

    fn from_slice(slice: &[u8]) -> Result<Option<Self>, Never> {
      exactly(slice)
    }
  }

  impl BeByteArray for [u8; 8] {
    const LEN: u64 = 8;

    fn from_slice(slice: &[u8]) -> Result<Option<Self>, Never> {
      exactly(slice)
    }
  }

  impl BeByteArray for [u8; 16] {
    const LEN: u64 = 16;

    fn from_slice(slice: &[u8]) -> Result<Option<Self>, Never> {
      exactly(slice)
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
  feature = "bytemuck",
  derive(bytemuck::Zeroable),
  zeroable(bound = "T: Scalar + Copy")
)]
#[repr(transparent)]
pub struct BigEndian<T: Scalar>(T::Raw);

// SAFETY: the transparent wrapper has one byte-array field, which has no padding, no pointers,
// and no invalid bit patterns.
#[cfg(feature = "bytemuck")]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the transparent wrapper's only field implements AnyBitPattern through the sealed byte-array bound"
  )
)]
unsafe impl<T> bytemuck::AnyBitPattern for BigEndian<T> where T: Scalar + Copy + 'static {}

impl<T: Scalar> BigEndian<T> {
  pub const fn new(raw: T::Raw) -> Result<BigEndian<T>, Never> {
    Ok(BigEndian(raw))
  }

  pub fn from_slice(slice: &[u8]) -> Result<Option<Self>, Never> {
    let Ok(raw) = <T::Raw as sealed::BeByteArray>::from_slice(slice);

    Ok(raw.map(Self))
  }

  #[inline(always)]
  pub fn get(&self) -> Result<T, Never> {
    T::from_raw(self.0)
  }

  pub fn set(&mut self, value: T) -> Result<(), Never> {
    let Ok(raw) = value.to_raw();

    self.0 = raw;

    Ok(())
  }

  pub fn be_bytes(&self) -> Result<&[u8], Never> {
    Ok(self.0.as_ref())
  }
}

impl<T: Scalar> From<T> for BigEndian<T> {
  #[inline]
  fn from(val: T) -> Self {
    let Ok(raw) = val.to_raw();

    BigEndian(raw)
  }
}

impl<T: Scalar + Default> Default for BigEndian<T> {
  fn default() -> Self {
    Self::from(T::default())
  }
}

impl<T: Scalar + Copy + PartialEq> PartialEq<T> for BigEndian<T> {
  fn eq(&self, other: &T) -> bool {
    let Ok(held) = self.get();

    held == *other
  }
}

impl<T: Scalar + Copy + PartialOrd + PartialEq> PartialOrd for BigEndian<T>
where
  <T as Scalar>::Raw: PartialEq,
{
  fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
    let Ok(held) = self.get();
    let Ok(theirs) = other.get();

    held.partial_cmp(&theirs)
  }
}

impl<T: Scalar + Copy + Ord + Eq> Ord for BigEndian<T>
where
  <T as Scalar>::Raw: Eq,
{
  fn cmp(&self, other: &Self) -> core::cmp::Ordering {
    let Ok(held) = self.get();
    let Ok(theirs) = other.get();

    held.cmp(&theirs)
  }
}

impl<T: Scalar> FixedSize for T {
  const RAW_BYTE_LEN: u64 = <T::Raw as sealed::BeByteArray>::LEN;
}

impl<T: Scalar> FixedSize for BigEndian<T> {
  const RAW_BYTE_LEN: u64 = T::RAW_BYTE_LEN;
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "a dozen one-field types are each a scalar by being the scalar they wrap, and this is the one place that says so"
  )
)]
macro_rules! newtype_scalar {
  ($ty:ident, $raw:ty) => {
    impl $crate::raw::Scalar for $ty {
      type Raw = $raw;

      fn to_raw(self) -> Result<$raw, console_core_never::Never> {
        $crate::raw::Scalar::to_raw(self.0)
      }

      #[inline(always)]
      fn from_raw(raw: $raw) -> Result<Self, console_core_never::Never> {
        let Ok(value) = $crate::raw::Scalar::from_raw(raw);

        Ok(Self(value))
      }
    }
  };
}

pub(crate) use newtype_scalar;

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the seven integer widths a font writes are each a scalar by their own big-endian bytes, and one line apiece says which"
  )
)]
macro_rules! int_scalar {
  ($ty:ty, $raw:ty) => {
    impl crate::raw::Scalar for $ty {
      type Raw = $raw;

      fn to_raw(self) -> Result<$raw, console_core_never::Never> {
        Ok(self.to_be_bytes())
      }

      #[inline(always)]
      fn from_raw(raw: $raw) -> Result<$ty, console_core_never::Never> {
        Ok(Self::from_be_bytes(raw))
      }
    }
  };
}

int_scalar!(u8, [u8; 1]);
int_scalar!(i8, [u8; 1]);
int_scalar!(u16, [u8; 2]);
int_scalar!(i16, [u8; 2]);
int_scalar!(u32, [u8; 4]);
int_scalar!(i32, [u8; 4]);
int_scalar!(i64, [u8; 8]);

impl Scalar for crate::Uint24 {
  type Raw = [u8; 3];

  fn to_raw(self) -> Result<[u8; 3], Never> {
    self.to_be_bytes()
  }

  #[inline(always)]
  fn from_raw(raw: [u8; 3]) -> Result<crate::Uint24, Never> {
    Self::from_be_bytes(raw)
  }
}

impl Scalar for crate::Int24 {
  type Raw = [u8; 3];

  fn to_raw(self) -> Result<[u8; 3], Never> {
    self.to_be_bytes()
  }

  #[inline(always)]
  fn from_raw(raw: [u8; 3]) -> Result<crate::Int24, Never> {
    Self::from_be_bytes(raw)
  }
}

impl<T: core::fmt::Debug + Scalar + Copy> core::fmt::Debug for BigEndian<T> {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    let Ok(held) = self.get();

    held.fmt(f)
  }
}

impl<T: core::fmt::Display + Scalar + Copy> core::fmt::Display for BigEndian<T> {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    let Ok(held) = self.get();

    held.fmt(f)
  }
}

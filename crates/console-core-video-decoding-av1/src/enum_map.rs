#![forbid(unsafe_code)]
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use std::marker::PhantomData;
use std::ops::Index;
use std::ops::IndexMut;

pub trait EnumKey<const N: usize>: Sized + Copy {
  fn ordinal(self) -> u8;
}

pub trait DefaultValue {
  const DEFAULT: Self;
}

impl<T> DefaultValue for Option<T> {
  const DEFAULT: Self = None;
}

pub struct EnumMap<K, V, const N: usize>
where
  K: EnumKey<N>,
{
  array: [V; N],
  _phantom: PhantomData<K>,
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "each DSP table uses its enum variant count as the const generic, keeping the table length synchronized with its keys"
  )
)]
macro_rules! enum_map_ty {
    ($K:ty, $V:ty) => {
        $crate::src::enum_map::EnumMap<$K, $V, { <$K as ::strum::EnumCount>::COUNT }>
    }
}

pub(crate) use enum_map_ty;

impl<K, V, const N: usize> EnumMap<K, V, N>
where
  K: EnumKey<N>,
{
  pub(crate) fn checked(&self, key: K) -> Rav1dResult<&V> {
    let Ok(slot) = console_core_number_conversion::index(key.ordinal());
    self.array.get(slot).ok_or(EINVAL)
  }

  pub const fn new(array: [V; N]) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        array,
        _phantom: PhantomData,
      }
    })
  }
}

impl<K, V, const N: usize> EnumMap<K, V, N>
where
  K: EnumKey<N>,
  V: DefaultValue,
{
  pub(crate) const fn default() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        array: [V::DEFAULT; N],
        _phantom: PhantomData,
      }
    })
  }
}

impl<K, V, const N: usize> Index<K> for EnumMap<K, V, N>
where
  K: EnumKey<N>,
{
  type Output = V;

  #[cfg_attr(
    dylint_lib = "explicit014_no_index_slice",
    expect(explicit014_no_index_slice, reason = "each EnumKey implementation maps its closed set of variants to the contiguous slots of this exact array")
  )]
  fn index(&self, index: K) -> &Self::Output {
    let Ok(slot) = console_core_number_conversion::index(index.ordinal());
    &self.array[slot]
  }
}

impl<K, V, const N: usize> IndexMut<K> for EnumMap<K, V, N>
where
  K: EnumKey<N>,
{
  #[cfg_attr(
    dylint_lib = "explicit014_no_index_slice",
    expect(explicit014_no_index_slice, reason = "the mutable DSP table uses the same closed enum keys and array length as its immutable lookup")
  )]
  fn index_mut(&mut self, index: K) -> &mut Self::Output {
    let Ok(slot) = console_core_number_conversion::index(index.ordinal());
    &mut self.array[slot]
  }
}

use console_core_never::Never;
use console_core_number_conversion::index;
use std::iter::{Copied, Zip};
use std::marker::PhantomData;
use std::slice;
use std::vec;

pub(crate) trait Variant: Copy + 'static {
  const ALL: &'static [Self];

  fn position(self) -> u32;
}

#[derive(Clone, Debug)]
pub(crate) struct ByVariant<K, V> {
  values: Vec<V>,
  keys: PhantomData<K>,
}

impl<K: Variant, V> ByVariant<K, V> {
  pub(crate) fn from_fn(mut value: impl FnMut(K) -> V) -> Result<Self, Never> {
    let mut values = Vec::with_capacity(K::ALL.len());

    for &key in K::ALL {
      values.push(value(key));
    }

    Ok(ByVariant {
      values,
      keys: PhantomData,
    })
  }

  pub(crate) fn get(&self, key: K) -> Result<Option<&V>, Never> {
    let Ok(at) = index(key.position());

    Ok(self.values.get(at))
  }

  pub(crate) fn get_mut(&mut self, key: K) -> Result<Option<&mut V>, Never> {
    let Ok(at) = index(key.position());

    Ok(self.values.get_mut(at))
  }
}

impl<K: Variant, V> IntoIterator for ByVariant<K, V> {
  type Item = (K, V);
  type IntoIter = Zip<Copied<slice::Iter<'static, K>>, vec::IntoIter<V>>;

  fn into_iter(self) -> Self::IntoIter {
    K::ALL.iter().copied().zip(self.values)
  }
}

impl<'a, K: Variant, V> IntoIterator for &'a ByVariant<K, V> {
  type Item = (K, &'a V);
  type IntoIter = Zip<Copied<slice::Iter<'static, K>>, slice::Iter<'a, V>>;

  fn into_iter(self) -> Self::IntoIter {
    K::ALL.iter().copied().zip(self.values.iter())
  }
}

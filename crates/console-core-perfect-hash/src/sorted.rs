//! Static dictionaries with unique entries in key order. Lookups borrow the key and allocate nothing.
use console_core_never::Never;
use core::borrow::Borrow;

pub struct SortedMap<K: 'static, V: 'static>(pub &'static [(K, V)]);

impl<K, V: Copy> SortedMap<K, V> {
  pub fn get<Q: Ord + ?Sized>(&self, key: &Q) -> Result<Option<V>, Never>
  where
    K: Borrow<Q>,
  {
    Ok(
      match self
        .0
        .binary_search_by(|(stored, _)| stored.borrow().cmp(key))
      {
        Ok(position) => self.0.get(position).map(|(_, value)| *value),
        Err(_insertion_point) => None,
      },
    )
  }
}

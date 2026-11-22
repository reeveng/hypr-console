use crate::Hash;
use crate::hash;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use core::marker::PhantomData;
use core::num::NonZeroU32;

pub struct Map<K, V>
where
  K: ?Sized,
  V: 'static,
{
  key: u64,
  disps: &'static [(u32, u32)],
  map: &'static [V],
  _phantom: PhantomData<fn(&K) -> V>,
}

impl<K: ?Sized, V: 'static> Map<K, V> {
  pub const fn new(
    key: u64,
    disps: &'static [(u32, u32)],
    map: &'static [V],
  ) -> Result<Self, Never> {
    Ok(Self {
      key,
      disps,
      map,
      _phantom: PhantomData,
    })
  }
}

impl<K, V> Map<K, V>
where
  K: ?Sized + Hash,
  V: 'static,
{
  #[inline]
  pub fn candidate(&self, key: &K) -> Result<Option<&'static V>, Never> {
    let Ok(hashes) = hash::hash(key, self.key);
    let Ok(displaced) = hash::get_unwrapped_index(&hashes, self.disps);
    let Ok(slots) = fitted::<_, u32>(self.map.len());

    Ok(match (displaced, NonZeroU32::new(slots)) {
      (Some(spread), Some(many)) => {
        let Ok(slot) = index(spread % many);

        self.map.get(slot)
      }
      (None, _) | (Some(_), None) => None,
    })
  }
}

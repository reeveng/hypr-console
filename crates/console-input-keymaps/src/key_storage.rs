use crate::Keycode;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use hashbrown::HashMap;
use std::collections::VecDeque;
use std::iter::Enumerate;

const DENSE_KEYCODE_LIMIT: u32 = 0xfff;

#[derive(Debug, Clone)]
pub(crate) struct KeyStorage<T> {
  storage: Storage<T>,
}

#[derive(Debug, Clone)]
enum Storage<T> {
  Vec(VecStorage<T>),
  Map(HashMap<Keycode, T>),
}

impl<T> KeyStorage<T> {
  pub(crate) fn new(mut map: HashMap<Keycode, T>) -> Result<Self, Never> {
    let max = match map.keys().copied().max() {
      Some(highest) => highest.0,
      None => 0,
    };

    match ::core::primitive::bool::then_some(max > DENSE_KEYCODE_LIMIT, ()) {
      ::core::option::Option::Some(()) => {
        map.shrink_to_fit();
        return Ok(Self {
          storage: Storage::Map(map),
        });
      }
      ::core::option::Option::None => {}
    }

    let Ok(slots) = index(max.saturating_add(1));
    let mut empty = Vec::new();
    empty.resize_with(slots.min(0x1000), || None);

    let data = map
      .into_iter()
      .fold(empty, |mut filling, (keycode, value)| {
        let Ok(at) = index(keycode.0);

        match filling.get_mut(at) {
          Some(slot) => *slot = Some(Box::new(value)),
          None => {}
        }

        filling
      });

    Ok(Self {
      storage: Storage::Vec(VecStorage { data }),
    })
  }

  #[inline]
  pub(crate) fn get(&self, key: Keycode) -> Result<Option<&T>, Never> {
    Ok(match &self.storage {
      Storage::Vec(v) => {
        let Ok(at) = index(key.0);

        match v.data.get(at) {
          Some(slot) => slot.as_deref(),
          None => None,
        }
      }
      Storage::Map(m) => m.get(&key),
    })
  }
}

#[derive(Debug, Clone)]
struct VecStorage<T> {
  data: Vec<Option<Box<T>>>,
}

pub(crate) enum Iter<'a, T> {
  Vec {
    iter: Enumerate<std::slice::Iter<'a, Option<Box<T>>>>,
  },
  Map {
    iter: VecDeque<(Keycode, &'a T)>,
  },
}

impl<'a, T> Iterator for Iter<'a, T> {
  type Item = (Keycode, &'a T);

  fn next(&mut self) -> Option<Self::Item> {
    match self {
      Iter::Vec { iter } => iter.find_map(|(position, slot)| {
        let Ok(code) = fitted::<_, u32>(position);

        slot.as_deref().map(|held| (Keycode(code), held))
      }),
      Iter::Map { iter } => iter.pop_front(),
    }
  }
}

impl<'a, T> IntoIterator for &'a KeyStorage<T> {
  type Item = (Keycode, &'a T);
  type IntoIter = Iter<'a, T>;

  fn into_iter(self) -> Self::IntoIter {
    match &self.storage {
      Storage::Vec(a) => Iter::Vec {
        iter: a.data.iter().enumerate(),
      },
      Storage::Map(a) => {
        let mut iter: Vec<_> = a.iter().map(|k| (*k.0, k.1)).collect();
        iter.sort_unstable_by_key(|k| k.0);
        Iter::Map {
          iter: VecDeque::from(iter),
        }
      }
    }
  }
}

use console_core_never::Never;
#[rustfmt::skip]
#[allow(clippy::upper_case_acronyms, reason = "a meaning is spelled as the XKB keyword it stands for")]
mod generated;

use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use console_core_perfect_hash::Map as PhfMap;
pub(crate) use generated::Meaning;
use hashbrown::HashMap;
use hashbrown::hash_map::Entry;

#[derive(Default)]
pub(crate) struct MeaningCache {
  sensitive: HashMap<Interned, Meaning>,
  insensitive: HashMap<Interned, Meaning>,
}

pub(crate) struct Keywords<T: 'static> {
  pub(crate) words: &'static [(Meaning, T)],
}

impl<T: Copy> Keywords<T> {
  pub(crate) fn get(&self, meaning: Meaning) -> Result<Option<T>, Never> {
    Ok(
      self
        .words
        .iter()
        .find(|(word, _)| *word == meaning)
        .map(|(_, value)| *value),
    )
  }
}

impl MeaningCache {
  pub(crate) fn get_case_sensitive(
    &mut self,
    interner: &Interner,
    name: Interned,
  ) -> Result<Meaning, Never> {
    Ok(match self.sensitive.entry(name) {
      Entry::Occupied(e) => *e.get(),
      Entry::Vacant(e) => {
        let Ok(val) = interner.get(name);
        let Ok(bytes) = val.as_bytes();
        let Ok(found) = generated::STRING_TO_MEANING.candidate(bytes);

        let meaning = match found {
          Some(listed) => *listed,
          None => Meaning::__Unknown,
        };

        let Ok(spelled) = meaning.name();
        let res = match ::core::primitive::bool::then_some(spelled.as_bytes() == bytes, ()) {
          ::core::option::Option::Some(()) => meaning,
          ::core::option::Option::None => Meaning::__Unknown,
        };
        e.insert(res);
        res
      }
    })
  }

  pub(crate) fn get_case_insensitive(
    &mut self,
    interner: &Interner,
    name: Interned,
  ) -> Result<Meaning, Never> {
    Ok(match self.insensitive.entry(name) {
      Entry::Occupied(e) => *e.get(),
      Entry::Vacant(e) => {
        let Ok(val) = interner.get(name);
        let Ok(bytes) = val.as_bytes();
        let lowered = bytes.to_ascii_lowercase();
        let Ok(found) = generated::LOWERCASE_TO_MEANING.candidate(lowered.as_slice());

        let meaning = match found {
          Some(listed) => *listed,
          None => Meaning::__Unknown,
        };

        let Ok(lowercase) = meaning.lowercase();

        let res = match ::core::primitive::bool::then_some(
          lowercase.as_bytes() == lowered.as_slice(),
          (),
        ) {
          ::core::option::Option::Some(()) => meaning,
          ::core::option::Option::None => Meaning::__Unknown,
        };
        e.insert(res);
        res
      }
    })
  }
}

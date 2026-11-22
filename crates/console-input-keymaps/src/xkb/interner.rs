use crate::xkb::code::Code;
use crate::xkb::code_slice::CodeSlice;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use console_input_keymaps_derive::CloneWithDelta;
use hashbrown::DefaultHashBuilder;
use indexmap::IndexSet;
use std::fmt::Debug;
use std::fmt::Formatter;

pub(crate) struct Interner {
  names: IndexSet<CodeSlice<'static>, DefaultHashBuilder>,
  nameless: CodeSlice<'static>,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, CloneWithDelta)]
pub(crate) struct Interned(u32);

impl Debug for Interned {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    Debug::fmt(&self.0, f)
  }
}

impl Debug for Interner {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_map()
      .entries((0_u32..).zip(self.names.iter()))
      .finish()
  }
}

impl Default for Interner {
  fn default() -> Self {
    let Ok(nothing) = Code::owning(Vec::new());
    let Ok(nameless) = CodeSlice::new_owned(nothing);

    Self {
      names: IndexSet::default(),
      nameless,
    }
  }
}

impl Interner {
  pub(crate) fn intern(&mut self, code: &CodeSlice<'_>) -> Result<Interned, Never> {
    let Ok(bytes) = code.as_bytes();

    Ok(match self.names.get_index_of(bytes) {
      Some(found) => {
        let Ok(id) = fitted::<_, u32>(found);
        Interned(id)
      }
      None => {
        let Ok(id) = fitted::<_, u32>(self.names.len());
        let Ok(owned) = code.to_owned();
        self.names.insert(owned);
        Interned(id)
      }
    })
  }

  pub(crate) fn get(&self, id: Interned) -> Result<&CodeSlice<'static>, Never> {
    let Ok(at) = index(id.0);

    Ok(match self.names.get_index(at) {
      Some(name) => name,
      None => &self.nameless,
    })
  }

  pub(crate) fn get_existing(&self, name: &[u8]) -> Result<Option<Interned>, Never> {
    Ok(self.names.get_index_of(name).map(|found| {
      let Ok(id) = fitted::<_, u32>(found);
      Interned(id)
    }))
  }
}

use core::ops::Range;

use crate::{AttrsOwned, HashMap, ShapeGlyph};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ShapeRunKey {
  pub text: String,
  pub default_attrs: AttrsOwned,
  pub attrs_spans: Vec<(Range<u32>, AttrsOwned)>,
}

#[derive(Clone, Default)]
pub struct ShapeRunCache {
  age: u64,
  cache: HashMap<ShapeRunKey, (u64, Vec<ShapeGlyph>)>,
}

impl ShapeRunCache {
  pub fn get(
    &mut self,
    key: &ShapeRunKey,
  ) -> Result<Option<&Vec<ShapeGlyph>>, console_core_never::Never> {
    {
      Ok(self.cache.get_mut(key).map(|(age, glyphs)| {
        *age = self.age;
        &*glyphs
      }))
    }
  }

  pub fn insert(
    &mut self,
    key: ShapeRunKey,
    glyphs: Vec<ShapeGlyph>,
  ) -> Result<(), console_core_never::Never> {
    {
      self.cache.insert(key, (self.age, glyphs));

      Ok(())
    }
  }

  pub fn trim(&mut self, keep_ages: u64) -> Result<(), console_core_never::Never> {
    {
      self
        .cache
        .retain(|_key, (age, _glyphs)| u64::saturating_add(*age, keep_ages) >= self.age);

      self.age = (self.age).saturating_add(1);

      Ok(())
    }
  }
}

impl core::fmt::Debug for ShapeRunCache {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_tuple("ShapeRunCache").finish()
  }
}

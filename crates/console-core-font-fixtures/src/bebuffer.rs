//! Big-endian scalar buffers with named locations for fixture patching.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use font_types::Scalar;
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferError {
  MissingTag(String),
  InvalidOffset(u64),
  InsufficientSpace { available: u64, required: u64 },
}

impl std::fmt::Display for BufferError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{self:?}")
  }
}

impl std::error::Error for BufferError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Emptiness {
  Empty,
  NonEmpty,
}

#[derive(Debug, Clone, Default)]
pub struct BeBuffer {
  data: Vec<u8>,
  tagged_locations: HashMap<String, u64>,
}

impl BeBuffer {
  #[allow(
    clippy::len_without_is_empty,
    reason = "emptiness answers an Emptiness, which is not the bool clippy looks for"
  )]
  pub fn len(&self) -> Result<u64, Never> {
    fitted(self.data.len())
  }

  pub fn emptiness(&self) -> Result<Emptiness, Never> {
    Ok(match self.data.as_slice() {
      [] => Emptiness::Empty,
      [_first, ..] => Emptiness::NonEmpty,
    })
  }

  pub fn as_slice(&self) -> Result<&[u8], Never> {
    Ok(&self.data)
  }

  pub fn push_with_tag(&mut self, item: impl Scalar, tag: &str) -> Result<(), Never> {
    let Ok(offset) = self.len();
    let _ = self.tagged_locations.insert(tag.to_string(), offset);
    self.extend([item]);
    Ok(())
  }

  pub fn offset_for(&self, tag: &str) -> Result<u64, BufferError> {
    self
      .tagged_locations
      .get(tag)
      .copied()
      .ok_or_else(|| BufferError::MissingTag(tag.to_owned()))
  }

  pub fn write_at(&mut self, tag: &str, item: impl Scalar) -> Result<(), BufferError> {
    let offset = self.offset_for(tag)?;
    let Ok(position) = index(offset);
    let data = self
      .data
      .get_mut(position..)
      .ok_or(BufferError::InvalidOffset(offset))?;
    let Ok(raw) = item.to_raw();
    let new_data: &[u8] = raw.as_ref();
    let Ok(available) = fitted::<_, u64>(data.len());
    let Ok(required) = fitted::<_, u64>(new_data.len());

    match available.cmp(&required) {
      Ordering::Less => {
        return Err(BufferError::InsufficientSpace {
          available,
          required,
        });
      }
      Ordering::Equal | Ordering::Greater => {}
    }

    for (left, right) in data.iter_mut().zip(new_data) {
      *left = *right;
    }

    Ok(())
  }

  pub fn data(&self) -> Result<&[u8], Never> {
    self.as_slice()
  }
}

impl<T: Scalar> Extend<T> for BeBuffer {
  fn extend<I: IntoIterator<Item = T>>(&mut self, items: I) {
    for item in items {
      let Ok(raw) = item.to_raw();

      self.data.extend(raw.as_ref());
    }
  }
}

impl std::ops::Deref for BeBuffer {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    &self.data
  }
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "four scalar, tag, array and iterator forms share serialization for hundreds of heterogeneous font fixtures through the exported buffer macro"
  )
)]
#[macro_export]
macro_rules! be_buffer_add {
    ($b:ident, {$v:tt : $tag:literal}) => {
        let Ok(()) = $b.push_with_tag($v, $tag);
    };
    ($b:ident, (..$v:expr)) => {
        $b.extend($v);
    };
    ($b:ident, [$($v:expr),+]) => {
        $b.extend([$($v),*]);
    };
    ($b:ident, $v:tt) => {
        $b.extend([$v]);
    };
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "hundreds of fixtures combine different scalar widths and named patch locations; the macro writes these fields once in binary order instead of repeating mutable serialization statements"
  )
)]
#[macro_export]
macro_rules! be_buffer {
    ( $( $x:tt ),+ $(,)? ) => {
        {
            let mut builder = $crate::bebuffer::BeBuffer::default();
            $(
                $crate::be_buffer_add!(builder, $x);
            )*
            builder
        }
    };
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn be_buffer_macro() {
    let data: &[u8] = &be_buffer! { 1u8, 2u16, 3u32 };
    assert_eq!([1, 0, 2, 0, 0, 0, 3], data);
  }

  #[test]
  fn be_buffer_macro_array() {
    let data: &[u8] = &be_buffer! { 1u8, [2u8, 3, 4, 5, 6] };
    assert_eq!([1, 2, 3, 4, 5, 6], data);
  }

  #[test]
  fn be_buffer_macro_tagged() -> Result<(), BufferError> {
    let builder = be_buffer! { 1u8, {2u16: "foo"}, {(u32::from(3u16)): "bar"} };
    let data: &[u8] = &builder;
    assert_eq!([1, 0, 2, 0, 0, 0, 3], data);
    let foo = builder.offset_for("foo")?;
    let bar = builder.offset_for("bar")?;
    assert_eq!(foo, 1);
    assert_eq!(bar, 3);
    Ok(())
  }

  #[test]
  fn a_buffer_with_nothing_pushed_is_empty() {
    assert_eq!(BeBuffer::default().emptiness(), Ok(Emptiness::Empty));
    assert_eq!(be_buffer! { 1u8 }.emptiness(), Ok(Emptiness::NonEmpty));
  }

  #[test]
  fn missing_tag_is_reported() {
    let mut builder = BeBuffer::default();
    assert_eq!(
      builder.write_at("absent", 1u32),
      Err(BufferError::MissingTag(String::from("absent")))
    );
  }

  #[test]
  fn short_write_preserves_buffer() {
    let mut builder = be_buffer! { {0xaau8: "end"} };
    assert_eq!(
      builder.write_at("end", 42u32),
      Err(BufferError::InsufficientSpace {
        available: 1,
        required: 4
      })
    );
    assert_eq!(&*builder, &[0xaa]);
  }
}

//! Arrays.

use crate::ObjectReadMode;
use crate::object::macros::object;
use crate::object::r#ref::MaybeRef;
use crate::object::{Object, ObjectIdentifier, ObjectLike};
use crate::reader::Reader;
use crate::reader::{Readable, ReaderContext, ReaderExt, Skippable};
use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};
use core::marker::PhantomData;

#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "An empty PDF array is valid by default; TryFrom checks the source object's variant"
  )
)]
#[derive(Clone)]
pub struct Array<'a> {
  data: &'a [u8],
  ctx: ReaderContext<'a>,
}

impl PartialEq for Array<'_> {
  fn eq(&self, other: &Self) -> bool {
    self.data == other.data
  }
}

impl<'a> Array<'a> {
  pub fn raw_iter(&self) -> core::result::Result<ArrayIter<'a>, console_core_never::Never> {
    let Ok(array_iter) = ArrayIter::new(self.data, &self.ctx);

    Ok(array_iter)
  }

  #[allow(
    private_bounds,
    reason = "users shouldn't be able to implement `ObjectLike` for custom objects."
  )]
  pub fn iter<T>(
    &self,
  ) -> core::result::Result<ResolvedArrayIter<'a, T>, console_core_never::Never>
  where
    T: ObjectLike<'a>,
  {
    let Ok(resolved_array_iter) = ResolvedArrayIter::new(self.data, &self.ctx);

    Ok(resolved_array_iter)
  }

  pub fn flex_iter(&self) -> core::result::Result<FlexArrayIter<'a>, console_core_never::Never> {
    let Ok(flex_array_iter) = FlexArrayIter::new(self.data, &self.ctx);

    Ok(flex_array_iter)
  }

  pub fn data(&self) -> core::result::Result<&'a [u8], console_core_never::Never> {
    Ok(self.data)
  }
}

impl Debug for Array<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    let mut debug_list = f.debug_list();

    let Ok(items) = self.raw_iter();

    for item in items {
      let _debug_builder = debug_list.entry(&item);
    }

    debug_list.finish()
  }
}

object!(Array<'a>, Array);

impl Skippable for Array<'_> {
  fn skip(r: &mut Reader<'_>, mode: ObjectReadMode) -> Option<()> {
    let Ok(opened) = r.forward_tag(b"[");

    opened?;

    let Ok(tail) = r.tail();
    let remaining = tail?;

    for _byte in remaining {
      r.skip_white_spaces_and_comments();

      let Ok(explicit_value) = r.forward_tag(b"]");
      match explicit_value {
        Some(()) => {
          return Some(());
        }
        None => match mode {
          ObjectReadMode::ContentStream => {
            r.skip::<Object<'_>>(mode)?;
          }
          ObjectReadMode::Document => {
            r.skip::<MaybeRef<Object<'_>>>(mode)?;
          }
        },
      }
    }

    None
  }
}

impl Default for Array<'_> {
  fn default() -> Self {
    let Ok(ctx) = ReaderContext::dummy();

    Self { data: &[], ctx }
  }
}

impl<'a> Readable<'a> for Array<'a> {
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let Ok(mode) = ctx.read_mode();
    let bytes = r.skip::<Array<'_>>(mode)?;
    let without_open = bytes.strip_prefix(b"[")?;
    let inner = without_open.strip_suffix(b"]")?;

    Some(Self {
      data: inner,
      ctx: ctx.clone(),
    })
  }
}

pub struct ArrayIter<'a> {
  reader: Reader<'a>,
  ctx: ReaderContext<'a>,
}

impl<'a> ArrayIter<'a> {
  fn new(
    data: &'a [u8],
    ctx: &ReaderContext<'a>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(reader) = Reader::new(data);

    Ok(Self {
      reader,
      ctx: ctx.clone(),
    })
  }
}

impl<'a> Iterator for ArrayIter<'a> {
  type Item = MaybeRef<Object<'a>>;

  fn next(&mut self) -> Option<Self::Item> {
    self.reader.skip_white_spaces_and_comments();

    self
      .reader
      .read_with_context::<MaybeRef<Object<'_>>>(&self.ctx)
  }
}

pub struct ResolvedArrayIter<'a, T> {
  flex_iter: FlexArrayIter<'a>,
  phantom_data: PhantomData<T>,
}

impl<'a, T> ResolvedArrayIter<'a, T> {
  fn new(
    data: &'a [u8],
    ctx: &ReaderContext<'a>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(flex_iter) = FlexArrayIter::new(data, ctx);

    Ok(Self {
      flex_iter,
      phantom_data: PhantomData,
    })
  }
}

impl<'a, T> Iterator for ResolvedArrayIter<'a, T>
where
  T: ObjectLike<'a>,
{
  type Item = T;

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Resolved array entries follow the existing depth and cycle limit on indirect references"
    )
  )]
  fn next(&mut self) -> Option<Self::Item> {
    let Ok(next) = self.flex_iter.next::<T>();

    next
  }
}

pub struct FlexArrayIter<'a> {
  reader: Reader<'a>,
  ctx: ReaderContext<'a>,
}

impl<'a> FlexArrayIter<'a> {
  fn new(
    data: &'a [u8],
    ctx: &ReaderContext<'a>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(reader) = Reader::new(data);

    Ok(Self {
      reader,
      ctx: ctx.clone(),
    })
  }

  #[allow(
    private_bounds,
    reason = "users shouldn't be able to implement `ObjectLike` for custom objects."
  )]
  #[allow(
    clippy::should_implement_trait,
    reason = "next answers a Result as every infallible function here does, which is not the signature Iterator asks for"
  )]
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "An indirect array element appends its object ID to the same bounded read context"
    )
  )]
  pub fn next<T: ObjectLike<'a>>(
    &mut self,
  ) -> core::result::Result<Option<T>, console_core_never::Never> {
    self.reader.skip_white_spaces_and_comments();
    let next = match self.reader.read_with_context::<MaybeRef<T>>(&self.ctx) {
      Some(value) => value,
      None => return Ok(None),
    };

    match next {
      MaybeRef::Ref(reference) => {
        let Ok(xref) = self.ctx.xref();

        xref.get_with::<T>(ObjectIdentifier::from(reference), &self.ctx)
      }
      MaybeRef::NotRef(value) => Ok(Some(value)),
    }
  }
}

impl<'a, T: ObjectLike<'a> + Copy + Default, const C: usize> TryFrom<Array<'a>> for [T; C] {
  type Error = ();

  fn try_from(value: Array<'a>) -> Result<Self, Self::Error> {
    let Ok(mut iter) = value.iter::<T>();

    let mut elements = [T::default(); C];

    for slot in &mut elements {
      let item = iter.next().ok_or(())?;

      *slot = item;
    }

    match ::core::primitive::bool::then_some(iter.next().is_some(), ()) {
      ::core::option::Option::Some(()) => {
        warn!("found excess elements in array");

        return Err(());
      }
      ::core::option::Option::None => {}
    }

    Ok(elements)
  }
}

impl<'a, T: ObjectLike<'a> + Copy + Default, const C: usize> TryFrom<Object<'a>> for [T; C]
where
  [T; C]: TryFrom<Array<'a>, Error = ()>,
{
  type Error = ();

  fn try_from(value: Object<'a>) -> Result<Self, Self::Error> {
    match value {
      Object::Array(a) => a.try_into(),
      Object::Null(_)
      | Object::Boolean(_)
      | Object::Number(_)
      | Object::String(_)
      | Object::Name(_)
      | Object::Dict(_)
      | Object::Stream(_) => Err(()),
    }
  }
}

impl<'a, T: ObjectLike<'a> + Copy + Default, const C: usize> Readable<'a> for [T; C] {
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let array = Array::read(r, ctx)?;

    match array.try_into() {
      Ok(items) => Some(items),
      Err(_invalid_array_for_target) => None,
    }
  }
}

impl<'a, T: ObjectLike<'a> + Copy + Default, const C: usize> ObjectLike<'a> for [T; C] {}

impl<'a, T: ObjectLike<'a>> TryFrom<Array<'a>> for Vec<T> {
  type Error = ();

  fn try_from(value: Array<'a>) -> Result<Self, Self::Error> {
    let Ok(iter) = value.iter::<T>();

    Ok(iter.collect())
  }
}

impl<'a, T: ObjectLike<'a>> TryFrom<Object<'a>> for Vec<T> {
  type Error = ();

  fn try_from(value: Object<'a>) -> Result<Self, Self::Error> {
    match value {
      Object::Array(a) => a.try_into(),
      Object::Null(_)
      | Object::Boolean(_)
      | Object::Number(_)
      | Object::String(_)
      | Object::Name(_)
      | Object::Dict(_)
      | Object::Stream(_) => Err(()),
    }
  }
}

impl<'a, T: ObjectLike<'a>> Readable<'a> for Vec<T> {
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let array = Array::read(r, ctx)?;

    match array.try_into() {
      Ok(items) => Some(items),
      Err(_invalid_array_for_target) => None,
    }
  }
}

impl<'a, T: ObjectLike<'a>> ObjectLike<'a> for Vec<T> {}

#[cfg(test)]
mod tests {
  use alloc::format;
  use alloc::{vec, vec::Vec};

  use crate::ObjectReadMode;
  use crate::object::Array;
  use crate::object::Object;
  use crate::object::r#ref::{MaybeRef, ObjRef};
  use crate::reader::Reader;
  use crate::reader::{ReaderContext, ReaderExt};
  use crate::test_support::{Required, TestFailure};
  use crate::xref::XRef;

  #[test]
  fn debug_arrays_close_their_lists() -> Result<(), TestFailure> {
    let empty = Array::default();
    let Ok(mut reader) = Reader::new(b"[true false]");
    let Ok(context) = ReaderContext::dummy();
    let array = reader.read_with_context::<Array<'_>>(&context).required()?;

    assert_eq!(format!("{empty:?}"), "[]");
    assert_eq!(format!("{array:?}"), "[Boolean(true), Boolean(false)]");

    Ok(())
  }

  fn array_impl(data: &[u8]) -> Result<Option<Vec<Object<'_>>>, console_core_never::Never> {
    let Ok(mut reader) = Reader::new(data);
    let Ok(xref) = XRef::dummy();
    let Ok(context) = ReaderContext::new(xref, ObjectReadMode::Document);

    Ok(reader.read_with_context::<Array<'_>>(&context).map(|a| {
      let Ok(objects) = a.iter::<Object<'_>>();
      objects.collect::<Vec<_>>()
    }))
  }

  fn array_ref_impl(
    data: &[u8],
  ) -> Result<Option<Vec<MaybeRef<Object<'_>>>>, console_core_never::Never> {
    let Ok(mut reader) = Reader::new(data);
    let Ok(xref) = XRef::dummy();
    let Ok(context) = ReaderContext::new(xref, ObjectReadMode::Document);

    Ok(reader.read_with_context::<Array<'_>>(&context).map(|a| {
      let Ok(entries) = a.raw_iter();
      entries.collect::<Vec<_>>()
    }))
  }

  #[test]
  fn empty_array_1() {
    assert_eq!(array_impl(b"[]"), Ok(Some(vec![])));
  }

  #[test]
  fn empty_array_2() {
    assert_eq!(array_impl(b"[   \n]"), Ok(Some(vec![])));
  }

  #[test]
  fn array_1() {
    let Ok(found) = array_impl(b"[34]");
    assert!(matches!(found.as_deref(), Some([Object::Number(_), ..])));
  }

  #[test]
  fn array_2() {
    let Ok(found) = array_impl(b"[true  ]");
    assert!(matches!(found.as_deref(), Some([Object::Boolean(_), ..])));
  }

  #[test]
  fn array_3() {
    let Ok(found) = array_impl(b"[true \n false 34.564]");
    assert!(matches!(
      found.as_deref(),
      Some([
        Object::Boolean(_),
        Object::Boolean(_),
        Object::Number(_),
        ..
      ])
    ));
  }

  #[test]
  fn array_4() {
    let Ok(found) = array_impl(b"[(A string.) << /Hi 34.35 >>]");
    assert!(matches!(
      found.as_deref(),
      Some([Object::String(_), Object::Dict(_), ..])
    ));
  }

  #[test]
  fn array_5() {
    let Ok(found) = array_impl(b"[[32]  345.6]");
    assert!(matches!(
      found.as_deref(),
      Some([Object::Array(_), Object::Number(_), ..])
    ));
  }

  #[test]
  fn array_with_ref() {
    let Ok(found) = array_ref_impl(b"[345 34 5 R 34.0]");
    assert!(matches!(
      found.as_deref(),
      Some([
        MaybeRef::NotRef(Object::Number(_)),
        MaybeRef::Ref(ObjRef {
          obj_number: 34,
          gen_number: 5
        }),
        MaybeRef::NotRef(Object::Number(_)),
        ..
      ])
    ));
  }

  #[test]
  fn malformed_ref_is_not_valid_array() {
    assert_eq!(array_ref_impl(b"[8 0R]"), Ok(None));
  }

  #[test]
  fn array_with_single_number_is_valid() {
    assert!(matches!(array_ref_impl(b"[345345345]"), Ok(Some(_))));
  }

  #[test]
  fn array_with_comment() {
    let Ok(found) = array_impl(b"[true % A comment \n false]");
    assert!(matches!(
      found.as_deref(),
      Some([Object::Boolean(_), Object::Boolean(_), ..])
    ));
  }

  #[test]
  fn array_with_trailing() {
    let Ok(found) = array_impl(b"[(Hi) /Test]trialing data");
    assert!(matches!(
      found.as_deref(),
      Some([Object::String(_), Object::Name(_), ..])
    ));
  }
}

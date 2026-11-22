//! Object references.

use crate::ObjectReadMode;
use crate::object::ObjectIdentifier;
use crate::object::ObjectLike;
use crate::reader::Reader;
use crate::reader::{Readable, ReaderContext, ReaderExt, Skippable};
use core::fmt::{Debug, Formatter};

#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "TryFrom rejects a direct object; the public fields hold the decoded PDF object and generation numbers"
  )
)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash)]
pub struct ObjRef {
  pub obj_number: i32,

  pub gen_number: i32,
}

impl ObjRef {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public constructor uses the PDF object number and generation number order"
    )
  )]
  pub fn new(
    obj_number: i32,
    gen_number: i32,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      obj_number,
      gen_number,
    })
  }
}

impl From<ObjRef> for ObjectIdentifier {
  fn from(value: ObjRef) -> Self {
    let Ok(object_identifier) = Self::new(value.obj_number, value.gen_number);

    object_identifier
  }
}

impl From<ObjectIdentifier> for ObjRef {
  fn from(value: ObjectIdentifier) -> Self {
    let Ok(obj_ref) = Self::new(value.obj_number, value.gen_number);

    obj_ref
  }
}

impl Skippable for ObjRef {
  fn skip(r: &mut Reader<'_>, _: ObjectReadMode) -> Option<()> {
    r.skip::<i32>(ObjectReadMode::Document)?;
    r.skip_white_spaces();
    r.skip::<i32>(ObjectReadMode::Document)?;
    r.skip_white_spaces();
    let Ok(forward_tag) = r.forward_tag(b"R");
    forward_tag?;

    Some(())
  }
}

impl Readable<'_> for ObjRef {
  fn read(r: &mut Reader<'_>, _: &ReaderContext<'_>) -> Option<Self> {
    let obj_ref = r.read_without_context::<i32>()?;
    r.skip_white_spaces();
    let gen_num = r.read_without_context::<i32>()?;
    r.skip_white_spaces();
    let Ok(forward_tag) = r.forward_tag(b"R");
    forward_tag?;

    Some({
      let Ok(explicit_value) = Self::new(obj_ref, gen_num);
      explicit_value
    })
  }
}

#[derive(PartialEq, Eq)]
pub enum MaybeRef<T> {
  Ref(ObjRef),

  NotRef(T),
}

impl<T> MaybeRef<T> {
  pub fn as_obj_ref(&self) -> core::result::Result<Option<ObjRef>, console_core_never::Never> {
    Ok(match self {
      Self::Ref(r) => Some(*r),
      Self::NotRef(_) => None,
    })
  }
}

#[allow(
  private_bounds,
  reason = "users shouldn't be able to implement `ObjectLike` for custom objects."
)]
impl<'a, T> MaybeRef<T>
where
  T: ObjectLike<'a>,
{
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Indirect object resolution records each reference and refuses chains past the fixed depth"
    )
  )]
  pub(crate) fn resolve(
    self,
    ctx: &ReaderContext<'a>,
  ) -> core::result::Result<Option<T>, console_core_never::Never> {
    Ok(match self {
      Self::Ref(r) => {
        let Ok(xref) = ctx.xref();
        let Ok(with) = xref.get_with::<T>(
          <crate::object::ObjectIdentifier as ::core::convert::From<
            crate::object::r#ref::ObjRef,
          >>::from(r),
          ctx,
        );

        with
      }
      Self::NotRef(t) => Some(t),
    })
  }
}

impl<T> TryFrom<MaybeRef<T>> for ObjRef {
  type Error = ();

  fn try_from(value: MaybeRef<T>) -> Result<Self, Self::Error> {
    match value {
      MaybeRef::Ref(r) => Ok(r),
      MaybeRef::NotRef(_) => Err(()),
    }
  }
}

impl<T> Debug for MaybeRef<T>
where
  T: Debug,
{
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Ref(r) => write!(f, "{r:?}"),
      Self::NotRef(nr) => write!(f, "{nr:?}"),
    }
  }
}

impl<T> Skippable for MaybeRef<T>
where
  T: Skippable,
{
  fn skip(r: &mut Reader<'_>, mode: ObjectReadMode) -> Option<()> {
    match r.skip::<ObjRef>(mode) {
      Some(_reference) => Some(()),
      None => r.skip::<T>(mode).map(|_| ()),
    }
  }
}

impl<'a, T> Readable<'a> for MaybeRef<T>
where
  T: Readable<'a>,
{
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    match r.read::<ObjRef>(ctx) {
      Some(obj) => Some(Self::Ref(obj)),
      None => {
        let read = r.read::<T>(ctx)?;

        Some(Self::NotRef(read))
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::object::ObjRef;
  use crate::reader::{Readable, Reader, ReaderExt};

  fn read<'a, T: Readable<'a>>(data: &'a [u8]) -> Result<Option<T>, console_core_never::Never> {
    let Ok(mut reader) = Reader::new(data);

    Ok(reader.read_without_context::<T>())
  }

  #[test]
  fn ref_1() {
    let Ok(expected) = ObjRef::new(34, 1);
    assert_eq!(read::<ObjRef>(b"34 1 R"), Ok(Some(expected)));
  }

  #[test]
  fn ref_trailing() {
    let Ok(expected) = ObjRef::new(256, 0);
    assert_eq!(read::<ObjRef>(b"256 0 R (hi)"), Ok(Some(expected)));
  }

  #[test]
  fn ref_invalid_1() {
    assert_eq!(read::<ObjRef>(b"256 R"), Ok(None));
  }

  #[test]
  fn ref_invalid_2() {
    assert_eq!(read::<ObjRef>(b"256 257"), Ok(None));
  }
}

use crate::object::r#ref::MaybeRef;
use crate::object::{Object, ObjectLike};
use crate::reader::Reader;
use crate::reader::{Readable, ReaderContext, ReaderExt};
use console_core_never::Never;
use core::fmt::Debug;

fn read_element<'a, T: ObjectLike<'a>>(
  reader: &mut Reader<'a>,
  context: &ReaderContext<'a>,
) -> Result<Option<T>, Never> {
  reader.skip_white_spaces_and_comments();

  match reader.read::<MaybeRef<T>>(context) {
    Some(value) => value.resolve(context),
    None => Ok(None),
  }
}

impl<'a, T, U> Readable<'a> for (T, U)
where
  T: ObjectLike<'a>,
  U: ObjectLike<'a>,
{
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let Ok(parsed_t) = read_element::<T>(r, ctx);
    let t = parsed_t?;
    let Ok(parsed_u) = read_element::<U>(r, ctx);
    let u = parsed_u?;
    Some((t, u))
  }
}

impl<'a, T, U> TryFrom<Object<'a>> for (T, U)
where
  T: ObjectLike<'a>,
  U: ObjectLike<'a>,
{
  type Error = ();

  fn try_from(_: Object<'a>) -> Result<Self, Self::Error> {
    Err(())
  }
}

impl<'a, T, U> ObjectLike<'a> for (T, U)
where
  T: ObjectLike<'a> + Debug + Clone,
  U: ObjectLike<'a> + Debug + Clone,
{
}

impl<'a, T, U, V> Readable<'a> for (T, U, V)
where
  T: ObjectLike<'a>,
  U: ObjectLike<'a>,
  V: ObjectLike<'a>,
{
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let Ok(parsed_t) = read_element::<T>(r, ctx);
    let t = parsed_t?;
    let Ok(parsed_u) = read_element::<U>(r, ctx);
    let u = parsed_u?;
    let Ok(parsed_v) = read_element::<V>(r, ctx);
    let v = parsed_v?;
    Some((t, u, v))
  }
}

impl<'a, T, U, V> TryFrom<Object<'a>> for (T, U, V)
where
  T: ObjectLike<'a>,
  U: ObjectLike<'a>,
  V: ObjectLike<'a>,
{
  type Error = ();

  fn try_from(_: Object<'a>) -> Result<Self, Self::Error> {
    Err(())
  }
}

impl<'a, T, U, V> ObjectLike<'a> for (T, U, V)
where
  T: ObjectLike<'a> + Debug + Clone,
  U: ObjectLike<'a> + Debug + Clone,
  V: ObjectLike<'a> + Debug + Clone,
{
}

impl<'a, T, U, V, W> Readable<'a> for (T, U, V, W)
where
  T: ObjectLike<'a>,
  U: ObjectLike<'a>,
  V: ObjectLike<'a>,
  W: ObjectLike<'a>,
{
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let Ok(parsed_t) = read_element::<T>(r, ctx);
    let t = parsed_t?;
    let Ok(parsed_u) = read_element::<U>(r, ctx);
    let u = parsed_u?;
    let Ok(parsed_v) = read_element::<V>(r, ctx);
    let v = parsed_v?;
    let Ok(parsed_w) = read_element::<W>(r, ctx);
    let w = parsed_w?;
    Some((t, u, v, w))
  }
}

impl<'a, T, U, V, W> TryFrom<Object<'a>> for (T, U, V, W)
where
  T: ObjectLike<'a>,
  U: ObjectLike<'a>,
  V: ObjectLike<'a>,
  W: ObjectLike<'a>,
{
  type Error = ();

  fn try_from(_: Object<'a>) -> Result<Self, Self::Error> {
    Err(())
  }
}

impl<'a, T, U, V, W> ObjectLike<'a> for (T, U, V, W)
where
  T: ObjectLike<'a> + Debug + Clone,
  U: ObjectLike<'a> + Debug + Clone,
  V: ObjectLike<'a> + Debug + Clone,
  W: ObjectLike<'a> + Debug + Clone,
{
}

impl<'a, T, U, V, W, X> Readable<'a> for (T, U, V, W, X)
where
  T: ObjectLike<'a>,
  U: ObjectLike<'a>,
  V: ObjectLike<'a>,
  W: ObjectLike<'a>,
  X: ObjectLike<'a>,
{
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let Ok(parsed_t) = read_element::<T>(r, ctx);
    let t = parsed_t?;
    let Ok(parsed_u) = read_element::<U>(r, ctx);
    let u = parsed_u?;
    let Ok(parsed_v) = read_element::<V>(r, ctx);
    let v = parsed_v?;
    let Ok(parsed_w) = read_element::<W>(r, ctx);
    let w = parsed_w?;
    let Ok(parsed_x) = read_element::<X>(r, ctx);
    let x = parsed_x?;
    Some((t, u, v, w, x))
  }
}

impl<'a, T, U, V, W, X> TryFrom<Object<'a>> for (T, U, V, W, X)
where
  T: ObjectLike<'a>,
  U: ObjectLike<'a>,
  V: ObjectLike<'a>,
  W: ObjectLike<'a>,
  X: ObjectLike<'a>,
{
  type Error = ();

  fn try_from(_: Object<'a>) -> Result<Self, Self::Error> {
    Err(())
  }
}

impl<'a, T, U, V, W, X> ObjectLike<'a> for (T, U, V, W, X)
where
  T: ObjectLike<'a> + Debug + Clone,
  U: ObjectLike<'a> + Debug + Clone,
  V: ObjectLike<'a> + Debug + Clone,
  W: ObjectLike<'a> + Debug + Clone,
  X: ObjectLike<'a> + Debug + Clone,
{
}

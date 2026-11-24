//! Rectangles.

use crate::object::Array;
use crate::object::{Object, ObjectLike};
use crate::reader::{Readable, ReaderContext, ReaderExt};

use crate::reader::Reader;

use console_core_point::Point;

pub type Rectangle = console_core_rectangle::Bounds<f64>;

impl<'a> Readable<'a> for Rectangle {
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let arr = r.read::<Array<'_>>(ctx)?;

    {
      let Ok(explicit_value) = from_arr(&arr);
      explicit_value
    }
  }
}

fn from_arr(
  array: &Array<'_>,
) -> core::result::Result<Option<Rectangle>, console_core_never::Never> {
  let Ok(mut iter) = array.iter::<f32>();
  let x0 = f64::from(match iter.next() {
    Some(value) => value,
    None => return Ok(None),
  });
  let y0 = f64::from(match iter.next() {
    Some(value) => value,
    None => return Ok(None),
  });
  let x1 = f64::from(match iter.next() {
    Some(value) => value,
    None => return Ok(None),
  });
  let y1 = f64::from(match iter.next() {
    Some(value) => value,
    None => return Ok(None),
  });

  let Ok(rect) = Rectangle::from_points(Point { x: x0, y: y0 }, Point { x: x1, y: y1 });

  Ok(Some(rect))
}

impl TryFrom<Object<'_>> for Rectangle {
  type Error = ();

  fn try_from(value: Object<'_>) -> Result<Self, Self::Error> {
    match value {
      Object::Array(arr) => ({
        let Ok(explicit_value) = from_arr(&arr);
        explicit_value
      })
      .ok_or(()),
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

impl ObjectLike<'_> for Rectangle {}

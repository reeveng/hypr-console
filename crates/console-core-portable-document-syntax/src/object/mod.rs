//! Parsing and reading from PDF objects.

pub use self::bool::PdfBoolean;
use crate::ObjectReadMode;
pub use crate::object::array::Array;
pub use crate::object::date::DateTime;
pub use crate::object::dict::Dict;
pub use crate::object::name::Name;
use crate::object::name::{NameSyntax, skip_name_like};
pub use crate::object::null::Null;
pub use crate::object::number::Number;
pub use crate::object::rect::Rectangle;
pub use crate::object::r#ref::{MaybeRef, ObjRef};
pub use crate::object::stream::Stream;
pub use crate::object::string::String;
use crate::reader::Reader;
use crate::reader::{Readable, ReaderContext, ReaderExt, Skippable};
use console_core_never::Never;
use console_core_number_conversion::index;
use core::fmt::Debug;

mod bool;
mod date;
mod null;
mod number;
mod rect;
mod r#ref;
mod string;
mod tuple;

pub(crate) mod indirect;
pub(crate) mod name;

pub mod array;
pub mod dict;
pub mod stream;

pub(crate) trait ObjectLike<'a>:
  TryFrom<Object<'a>> + Readable<'a> + Debug + Clone
{
}

pub(crate) trait ObjectRefLike<'a>: Sized {
  fn cast_ref<'b>(obj: &'b Object<'a>) -> Option<&'b Self>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Object<'a> {
  Null(Null),

  Boolean(PdfBoolean),

  Number(Number),

  String(String<'a>),

  Name(Name<'a>),

  Dict(Dict<'a>),

  Array(Array<'a>),

  Stream(Stream<'a>),
}

impl<'a> Object<'a> {
  pub(crate) fn cast<T>(self) -> core::result::Result<Option<T>, console_core_never::Never>
  where
    T: ObjectLike<'a>,
  {
    Ok(match self.try_into() {
      Ok(object) => Some(object),
      Err(_different_object_kind) => None,
    })
  }

  pub(crate) fn cast_ref<T>(&self) -> core::result::Result<Option<&T>, console_core_never::Never>
  where
    T: ObjectRefLike<'a>,
  {
    Ok(T::cast_ref(self))
  }

  #[inline(always)]
  pub fn into_dict(self) -> core::result::Result<Option<Dict<'a>>, console_core_never::Never> {
    let Ok(cast) = self.cast();

    Ok(cast)
  }

  #[inline(always)]
  pub fn into_name(self) -> core::result::Result<Option<Name<'a>>, console_core_never::Never> {
    let Ok(cast) = self.cast();

    Ok(cast)
  }

  #[inline(always)]
  pub fn into_null(self) -> core::result::Result<Option<Null>, console_core_never::Never> {
    let Ok(cast) = self.cast();

    Ok(cast)
  }

  #[inline(always)]
  pub fn into_bool(self) -> core::result::Result<Option<PdfBoolean>, console_core_never::Never> {
    let Ok(cast) = self.cast();

    Ok(cast)
  }

  #[inline(always)]
  pub fn into_string(
    self,
  ) -> core::result::Result<Option<String<'a>>, console_core_never::Never> {
    let Ok(cast) = self.cast();

    Ok(cast)
  }

  #[inline(always)]
  pub fn into_stream(
    self,
  ) -> core::result::Result<Option<Stream<'a>>, console_core_never::Never> {
    let Ok(cast) = self.cast();

    Ok(cast)
  }

  #[inline(always)]
  pub fn into_array(self) -> core::result::Result<Option<Array<'a>>, console_core_never::Never> {
    let Ok(cast) = self.cast();

    Ok(cast)
  }

  #[inline(always)]
  pub fn into_u8(self) -> core::result::Result<Option<u8>, console_core_never::Never> {
    let Ok(cast) = self.cast();

    Ok(cast)
  }

  #[inline(always)]
  pub fn into_u16(self) -> core::result::Result<Option<u16>, console_core_never::Never> {
    let Ok(cast) = self.cast();

    Ok(cast)
  }

  #[inline(always)]
  pub fn into_f32(self) -> core::result::Result<Option<f32>, console_core_never::Never> {
    let Ok(cast) = self.cast();

    Ok(cast)
  }

  #[inline(always)]
  pub fn into_i32(self) -> core::result::Result<Option<i32>, console_core_never::Never> {
    let Ok(cast) = self.cast();

    Ok(cast)
  }

  #[inline(always)]
  pub fn into_number(self) -> core::result::Result<Option<Number>, console_core_never::Never> {
    let Ok(cast) = self.cast();

    Ok(cast)
  }
}

impl<'a> ObjectLike<'a> for Object<'a> {}

impl<'a> ObjectRefLike<'a> for Object<'a> {
  fn cast_ref(obj: &Self) -> Option<&Self> {
    Some(obj)
  }
}

impl Skippable for Object<'_> {
  fn skip(r: &mut Reader<'_>, mode: ObjectReadMode) -> Option<()> {
    let Ok(peek_byte) = r.peek_byte();

    let checked_value = peek_byte?;

    match checked_value {
      b'n' => Null::skip(r, mode),
      b't' | b'f' => PdfBoolean::skip(r, mode),
      b'/' => Name::skip(r, mode),
      b'<' => {
        let Ok(peek_bytes) = r.peek_bytes(2);

        let checked_value_2 = peek_bytes?;

        match checked_value_2 {
          b"<<" => Dict::skip(r, mode),
          _ => String::skip(r, mode),
        }
      }
      b'(' => String::skip(r, mode),
      b'.' | b'+' | b'-' | b'0'..=b'9' => Number::skip(r, mode),
      b'[' => Array::skip(r, mode),

      _ => {
        let Ok(explicit_value) = skip_name_like(r, NameSyntax::Bare);
        explicit_value
      }
    }
  }
}

impl<'a> Readable<'a> for Object<'a> {
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let Ok(peek_byte) = r.peek_byte();
    let next_byte = peek_byte?;

    match next_byte {
      b'n' => Null::read(r, ctx).map(Self::Null),
      b't' | b'f' => PdfBoolean::read(r, ctx).map(Self::Boolean),
      b'/' => Name::read(r, ctx).map(Self::Name),
      b'<' => {
        let Ok(prefix) = r.peek_bytes(2);

        match prefix {
          Some(b"<<") => {
            let Ok(object) = read_dictionary_or_stream(r, ctx);
            object
          }
          Some(_) => String::read(r, ctx).map(Self::String),
          None => None,
        }
      }
      b'(' => String::read(r, ctx).map(Self::String),
      b'.' | b'+' | b'-' | b'0'..=b'9' => Number::read(r, ctx).map(Self::Number),
      b'[' => Array::read(r, ctx).map(Self::Array),
      _ => {
        let Ok(skipped) = skip_name_like(r, NameSyntax::Bare);
        skipped?;
        Some(Self::Null(Null))
      }
    }
  }
}

fn read_dictionary_or_stream<'a>(
  reader: &mut Reader<'a>,
  context: &ReaderContext<'a>,
) -> Result<Option<Object<'a>>, Never> {
  let mut probe = reader.clone();
  let dictionary = match Dict::read(&mut probe, context) {
    Some(dictionary) => dictionary,
    None => return Ok(None),
  };
  probe.skip_white_spaces_and_comments();
  let Ok(marker) = probe.forward_tag(b"stream");

  match marker {
    Some(()) => Ok(Stream::read(reader, context).map(Object::Stream)),
    None => {
      let Ok(position) = probe.fixed_offset();
      let Ok(offset) = index(position);
      let Ok(()) = reader.jump(offset);
      Ok(Some(Object::Dict(dictionary)))
    }
  }
}

pub trait FromBytes<'a>: Sized {
  fn from_bytes(b: &'a [u8]) -> Option<Self>;
}

impl<'a, T: Readable<'a>> FromBytes<'a> for T {
  fn from_bytes(b: &'a [u8]) -> Option<Self> {
    {
      let Ok(explicit_value) = Self::from_bytes_impl(b);
      explicit_value
    }
  }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct ObjectIdentifier {
  pub obj_number: i32,

  pub gen_number: i32,
}

impl ObjectIdentifier {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public constructor follows the PDF object number and generation number order"
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

impl Readable<'_> for ObjectIdentifier {
  fn read(r: &mut Reader<'_>, _: &ReaderContext<'_>) -> Option<Self> {
    let obj_number = r.read_without_context::<i32>()?;
    r.skip_white_spaces_and_comments();
    let gen_number = r.read_without_context::<i32>()?;
    r.skip_white_spaces_and_comments();
    let Ok(forward_tag) = r.forward_tag(b"obj");
    forward_tag?;

    Some(Self {
      obj_number,
      gen_number,
    })
  }
}

impl Skippable for ObjectIdentifier {
  fn skip(r: &mut Reader<'_>, _: ObjectReadMode) -> Option<()> {
    r.skip::<i32>(ObjectReadMode::Document)?;
    r.skip_white_spaces_and_comments();
    r.skip::<i32>(ObjectReadMode::Document)?;
    r.skip_white_spaces_and_comments();
    let Ok(forward_tag) = r.forward_tag(b"obj");
    forward_tag?;

    Some(())
  }
}

pub fn dict_or_stream<'a, 'b>(
  obj: &'b Object<'a>,
) -> core::result::Result<Option<(&'b Dict<'a>, Option<&'b Stream<'a>>)>, console_core_never::Never>
{
  let Ok(explicit_value) = obj.cast_ref::<Stream<'a>>();

  Ok(match explicit_value {
    Some(stream) => {
      let Ok(stream_dict) = stream.dict();

      Some((stream_dict, Some(stream)))
    }
    None => {
      let Ok(cast_ref) = obj.cast_ref::<Dict<'a>>();

      cast_ref.map(|dict| (dict, None))
    }
  })
}

mod macros {
  #[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
      explicit080_macro_budget,
      reason = "Each invocation supplies the Object variant and implements its owned and borrowed conversion plus the parser marker together"
    )
  )]
  macro_rules! object {
        ($t:ident $(<$l:lifetime>),*, $s:ident) => {
            impl<'a> TryFrom<Object<'a>> for $t$(<$l>),* {
                type Error = ();

                fn try_from(value: Object<'a>) -> core::result::Result<Self, Self::Error> {
                    match value {
                        Object::$s(b) => Ok(b),
                        _ => Err(()),
                    }
                }
            }

            impl<'a> crate::object::ObjectLike<'a> for $t$(<$l>),* {}

            impl<'a> crate::object::ObjectRefLike<'a> for $t$(<$l>),* {
                fn cast_ref<'b>(obj: &'b crate::object::Object<'a>) -> Option<&'b Self> {
                    match obj {
                        crate::object::Object::$s(b) => Some(b),
                        _ => None,
                    }
                }
            }
        };
    }

  pub(crate) use object;
}

#[cfg(test)]
mod tests {
  use crate::object::{Array, Dict, Name, Null, Number, Object, Stream, String};
  use crate::reader::Reader;
  use crate::reader::{ReaderContext, ReaderExt};

  fn object_impl(data: &[u8]) -> Result<Option<Object<'_>>, console_core_never::Never> {
    let Ok(mut r) = Reader::new(data);
    let Ok(context) = ReaderContext::dummy();

    Ok(r.read_with_context::<Object<'_>>(&context))
  }

  #[test]
  fn null() {
    assert!(matches!(object_impl(b"null"), Ok(Some(Object::Null(_)))));
  }

  #[test]
  fn bool() {
    assert!(matches!(object_impl(b"true"), Ok(Some(Object::Boolean(_)))));
  }

  #[test]
  fn number() {
    assert!(matches!(object_impl(b"34.5"), Ok(Some(Object::Number(_)))));
  }

  #[test]
  fn string_1() {
    assert!(matches!(object_impl(b"(Hi)"), Ok(Some(Object::String(_)))));
  }

  #[test]
  fn string_2() {
    assert!(matches!(object_impl(b"<34>"), Ok(Some(Object::String(_)))));
  }

  #[test]
  fn name() {
    assert!(matches!(object_impl(b"/Name"), Ok(Some(Object::Name(_)))));
  }

  #[test]
  fn dict() {
    assert!(matches!(
      object_impl(b"<</Entry 45>>"),
      Ok(Some(Object::Dict(_)))
    ));
  }

  #[test]
  fn array() {
    assert!(matches!(object_impl(b"[45]"), Ok(Some(Object::Array(_)))));
  }

  #[test]
  fn stream() {
    assert!(matches!(
      object_impl(b"<< /Length 3 >> stream\nabc\nendstream"),
      Ok(Some(Object::Stream(_)))
    ));
  }

  #[test]
  #[cfg(target_pointer_width = "64")]
  fn object_sizes() {
    assert_eq!(size_of::<Object<'_>>(), 32);
    assert_eq!(size_of::<Array<'_>>(), 32);
    assert_eq!(size_of::<Dict<'_>>(), 8);
    assert_eq!(size_of::<Name<'_>>(), 24);
    assert_eq!(size_of::<Null>(), 0);
    assert_eq!(size_of::<Number>(), 16);
    assert_eq!(size_of::<Stream<'_>>(), 24);
    assert_eq!(size_of::<String<'_>>(), 24);
  }
}

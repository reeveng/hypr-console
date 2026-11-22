//! Streams.

use crate::ObjectReadMode;
use crate::crypto::DecryptionTarget;
use crate::filter::Filter;
use crate::object;
use crate::object::Dict;
use crate::object::Name;
use crate::object::dict::keys::{DECODE_PARMS, DP, F, FILTER, LENGTH, TYPE};
use crate::object::{Array, ObjectIdentifier};
use crate::object::{Object, ObjectLike, ObjectRefLike};
use crate::reader::Reader;
use crate::reader::{Readable, ReaderContext, ReaderExt, Skippable};
use crate::trivia::is_white_space_character;
use crate::util::{OptionLog, find_needle};
use alloc::borrow::Cow;
use alloc::vec::Vec;
use console_core_never::Never;
use console_core_number_conversion::index;
use core::fmt::{Debug, Formatter};

#[derive(Clone)]
pub struct Stream<'a> {
  dict: Dict<'a>,
  data: &'a [u8],
}

impl PartialEq for Stream<'_> {
  fn eq(&self, other: &Self) -> bool {
    self.dict == other.dict && self.data == other.data
  }
}

#[doc = "Interpretation of the decoded image samples."]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum ImageSamples {
  #[default]
  Direct,
  PaletteIndices,
}

#[derive(Clone, PartialEq, Default)]
pub struct ImageDecodeParams {
  pub samples: ImageSamples,

  pub bpc: Option<u8>,

  pub num_components: Option<u8>,

  pub target_dimension: Option<(u32, u32)>,

  pub width: u32,

  pub height: u32,
}

impl<'a> Stream<'a> {
  pub(crate) fn new(
    data: &'a [u8],
    dict: Dict<'a>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { dict, data })
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Filter abbreviations and canonical values are resolved under the indirect-object depth limit"
    )
  )]
  fn field<T: ObjectLike<'a>>(&self, [short, full]: [&[u8]; 2]) -> Result<Option<T>, Never> {
    let Ok(short_value) = self.dict.get::<T>(short);

    match short_value {
      Some(value) => Ok(Some(value)),
      None => self.dict.get::<T>(full),
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "An indirect Filter or DecodeParms field retains the containing object's bounded read context"
    )
  )]
  fn filters_and_params(&self) -> Result<Vec<(Filter, Dict<'a>)>, Never> {
    let Ok(single_name) = self.field::<Name<'_>>([F, FILTER]);

    match single_name {
      Some(name) => {
        let Ok(filter) = Filter::from_name(name);

        match filter {
          Some(filter_2) => {
            let Ok(parameters) = self.field::<Dict<'_>>([DP, DECODE_PARMS]);
            let params = match parameters {
              Some(dictionary) => dictionary,
              None => Dict::default(),
            };
            return Ok(alloc::vec![(filter_2, params)]);
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    let Ok(filter_names) = self.field::<Array<'_>>([F, FILTER]);
    let names = match filter_names {
      Some(names) => names,
      None => return Ok(Vec::new()),
    };
    let Ok(names_iter) = names.iter::<Name<'_>>();
    let Ok(parameter_array) = self.field::<Array<'_>>([DP, DECODE_PARMS]);
    let mut parameters = parameter_array.map(|array| {
      let Ok(values) = array.iter::<Object<'_>>();
      values
    });
    let mut filters = Vec::new();

    for name in names_iter {
      let Ok(filter) = Filter::from_name(name);
      let parameters_value = parameters.as_mut().and_then(Iterator::next);

      match filter {
        Some(filter_3) => {
          let dictionary = match parameters_value {
            Some(Object::Dict(dictionary)) => dictionary,
            Some(
              Object::Null(_)
              | Object::Boolean(_)
              | Object::Number(_)
              | Object::String(_)
              | Object::Name(_)
              | Object::Array(_)
              | Object::Stream(_),
            )
            | None => Dict::default(),
          };
          filters.push((filter_3, dictionary));
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(filters)
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "A stream type reference uses indirect resolution with a fixed depth"
    )
  )]
  pub fn raw_data(&self) -> core::result::Result<Cow<'a, [u8]>, console_core_never::Never> {
    let Ok(ctx) = self.dict.ctx();

    let Ok(xref) = ctx.xref();
    let Ok(needs_decryption) = xref.needs_decryption(ctx);

    Ok(
      match ::core::primitive::bool::then_some(
        matches!(needs_decryption, crate::NeedsDecryptionAnswer::Yes)
          && match ({
            let Ok(explicit_value) = self.dict.get::<object::String<'_>>(TYPE);
            explicit_value
          })
          .map(|t| t.as_ref() != b"XRef")
          {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => true,
          },
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let Ok(ctx_xref) = ctx.xref();
          let Ok(obj_id) = self.obj_id();
          let Ok(decrypt) = ctx_xref.decrypt(obj_id, self.data, DecryptionTarget::Stream);

          Cow::Owned(match decrypt {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => Vec::new(),
          })
        }
        ::core::option::Option::None => Cow::Borrowed(self.data),
      },
    )
  }

  pub fn dict(&self) -> core::result::Result<&Dict<'a>, console_core_never::Never> {
    Ok(&self.dict)
  }

  pub fn obj_id(&self) -> core::result::Result<ObjectIdentifier, console_core_never::Never> {
    let Ok(obj_id) = self.dict.obj_id();

    Ok(match obj_id {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => {
        let Ok(object_identifier) = ObjectIdentifier::new(0, 0);

        object_identifier
      }
    })
  }

  pub fn filters(&self) -> core::result::Result<Vec<Filter>, console_core_never::Never> {
    let Ok(filters_and_params) = self.filters_and_params();

    Ok(
      filters_and_params
        .into_iter()
        .map(|(filter, _params)| filter)
        .collect(),
    )
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Decoded globals may reference another stream through the bounded indirect reader"
    )
  )]
  pub fn decoded(&self) -> Result<Cow<'a, [u8]>, DecodeFailure> {
    self
      .decoded_image(&ImageDecodeParams::default())
      .map(|r| r.data)
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Filters may load indirect parameters or globals under the reference-depth limit"
    )
  )]
  pub fn decoded_image(
    &self,
    image_params: &ImageDecodeParams,
  ) -> Result<FilterResult<'a>, DecodeFailure> {
    let Ok(data) = self.raw_data();
    let Ok(filters) = self.filters_and_params();
    let mut input = FilterResult {
      data,
      image_data: None,
    };

    for (filter, params) in filters {
      let next = filter.apply(input.data.as_ref(), &params, image_params)?;
      input = next;
    }

    Ok(input)
  }
}

impl Debug for Stream<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "Stream (len: {:?})", self.data.len())
  }
}

impl Skippable for Stream<'_> {
  fn skip(_: &mut Reader<'_>, _: ObjectReadMode) -> Option<()> {
    warn!("attempted to skip a stream object");

    None
  }
}

impl<'a> Readable<'a> for Stream<'a> {
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let dict = r.read_with_context::<Dict<'_>>(ctx)?;

    let Ok(contains_key) = dict.contains_key(F);

    match contains_key {
      crate::ContainsKeyAnswer::Yes => {
        warn!("encountered stream referencing external file, which is unsupported");

        return None;
      }
      crate::ContainsKeyAnswer::No => {}
    }

    let Ok(start) = r.fixed_offset();
    let Ok(proper) = parse_proper(r, &dict);

    match proper {
      Some(stream) => Some(stream),
      None => {
        warn!("failed to parse stream, trying to parse it manually");
        let Ok(offset) = index(start);
        let Ok(()) = r.jump(offset);
        let Ok(recovered) = parse_fallback(r, &dict);
        recovered.error_none("was unable to manually parse the stream")
      }
    }
  }
}

#[derive(Debug, Copy, Clone)]

pub enum DecodeFailure {
  ImageDecode,

  StreamDecode,

  Decryption,

  Unknown,
}

#[derive(Debug, Copy, Clone)]
pub enum ImageColorSpace {
  Gray,

  Rgb,

  Cmyk,

  Unknown(u8),
}

pub struct ImageData {
  pub alpha: Option<Vec<u8>>,

  pub color_space: Option<ImageColorSpace>,

  pub bits_per_component: u8,

  pub width: u32,

  pub height: u32,
}

pub struct FilterResult<'a> {
  pub data: Cow<'a, [u8]>,

  pub image_data: Option<ImageData>,
}

impl FilterResult<'_> {
  pub(crate) fn from_data(
    data: Vec<u8>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      data: Cow::Owned(data),
      image_data: None,
    })
  }
}

fn read_stream_start(r: &mut Reader<'_>) -> Result<Option<()>, Never> {
  r.skip_white_spaces_and_comments();
  let Ok(marker) = r.forward_tag(b"stream");

  match marker {
    Some(()) => {}
    None => return Ok(None),
  }

  for ending in [b"\n".as_slice(), b"\r\n", b"\r"] {
    let Ok(newline) = r.forward_tag(ending);

    match newline {
      Some(()) => return Ok(Some(())),
      None => {}
    }
  }

  Ok(None)
}

fn parse_proper<'a>(r: &mut Reader<'a>, dict: &Dict<'a>) -> Result<Option<Stream<'a>>, Never> {
  let Ok(length_field) = dict.get::<u32>(LENGTH);
  let length = match length_field {
    Some(length) => length,
    None => return Ok(None),
  };
  let Ok(opened) = read_stream_start(r);

  match opened {
    Some(()) => {}
    None => return Ok(None),
  }

  let Ok(position) = index(length);
  let Ok(body) = r.read_bytes(position);
  let data = match body {
    Some(bytes) => bytes,
    None => return Ok(None),
  };
  r.skip_white_spaces();
  let Ok(ended) = r.forward_tag(b"endstream");

  match ended {
    Some(()) => {
      let Ok(stream) = Stream::new(data, dict.clone());
      Ok(Some(stream))
    }
    None => Ok(None),
  }
}

fn parse_fallback<'a>(r: &mut Reader<'a>, dict: &Dict<'a>) -> Result<Option<Stream<'a>>, Never> {
  let Ok(opened) = read_stream_start(r);

  match opened {
    Some(()) => {}
    None => return Ok(None),
  }

  let Ok(remaining) = r.tail();
  let tail = match remaining {
    Some(bytes) => bytes,
    None => return Ok(None),
  };
  let Ok(endstream) = find_needle(tail, b"endstream");
  let offset = match endstream {
    Some(offset) => offset,
    None => return Ok(None),
  };
  let Ok(position) = index(offset);
  let Ok(body) = r.read_bytes(position);
  let bytes = match body {
    Some(bytes) => bytes,
    None => return Ok(None),
  };
  let data = match bytes.iter().rposition(|byte| {
    let Ok(whitespace) = is_white_space_character(*byte);
    whitespace == crate::IsWhiteSpaceCharacterAnswer::No
  }) {
    Some(last) => match bytes.get(..=last) {
      Some(trimmed) => trimmed,
      None => return Ok(None),
    },
    None => &[],
  };
  r.skip_white_spaces();
  let Ok(ended) = r.forward_tag(b"endstream");

  match ended {
    Some(()) => {
      let Ok(stream) = Stream::new(data, dict.clone());
      Ok(Some(stream))
    }
    None => Ok(None),
  }
}

impl<'a> TryFrom<Object<'a>> for Stream<'a> {
  type Error = ();

  fn try_from(value: Object<'a>) -> Result<Self, Self::Error> {
    match value {
      Object::Stream(s) => Ok(s),
      Object::Null(_)
      | Object::Boolean(_)
      | Object::Number(_)
      | Object::String(_)
      | Object::Name(_)
      | Object::Dict(_)
      | Object::Array(_) => Err(()),
    }
  }
}

impl<'a> ObjectLike<'a> for Stream<'a> {}

impl<'a> ObjectRefLike<'a> for Stream<'a> {
  fn cast_ref<'b>(obj: &'b Object<'a>) -> Option<&'b Self> {
    match obj {
      Object::Stream(stream) => Some(stream),
      Object::Null(_)
      | Object::Boolean(_)
      | Object::Number(_)
      | Object::String(_)
      | Object::Name(_)
      | Object::Dict(_)
      | Object::Array(_) => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::object::Stream;
  use crate::reader::Reader;
  use crate::reader::{ReaderContext, ReaderExt};
  use crate::test_support::{Required, TestFailure};

  fn read_stream(data: &[u8]) -> Result<Option<Stream<'_>>, console_core_never::Never> {
    let Ok(mut r) = Reader::new(data);
    let Ok(context) = ReaderContext::dummy();

    Ok(r.read_with_context::<Stream<'_>>(&context))
  }

  #[test]
  fn stream() -> Result<(), TestFailure> {
    let Ok(read) = read_stream(b"<< /Length 10 >> stream\nabcdefghij\nendstream");
    let found = read.required()?;

    assert_eq!(found.data, b"abcdefghij");

    Ok(())
  }

  #[test]
  fn stream_fallback() -> Result<(), TestFailure> {
    let Ok(read) = read_stream(b"<< /Length 999 >> stream\nabcdefghij\nendstream");
    let found = read.required()?;

    assert_eq!(found.data, b"abcdefghij");

    Ok(())
  }

  #[test]
  fn dictionary_is_not_a_later_objects_stream() {
    let data =
      b"<< /Yes 5 0 R /Off 6 0 R >>\nendobj\n11 0 obj << /Length 5 >> stream\nhello\nendstream";
    assert!(matches!(read_stream(data), Ok(None)));
  }
}

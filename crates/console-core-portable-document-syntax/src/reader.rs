//! Reading bytes and PDF objects from data.

use crate::answers::{ObjectReadMode, ObjectStorage};
use crate::object::ObjectIdentifier;
use crate::sync::Arc;
use crate::trivia::{Comment, is_eol_character, is_white_space_character};
use crate::xref::XRef;
use alloc::vec::Vec;
use console_core_never::Never;
use console_core_number_conversion::index;

pub use crate::byte_reader::Reader;

pub trait ReaderExt<'a> {
  fn read<T: Readable<'a>>(&mut self, ctx: &ReaderContext<'a>) -> Option<T>;

  fn read_with_context<T: Readable<'a>>(&mut self, ctx: &ReaderContext<'a>) -> Option<T>;

  fn read_without_context<T: Readable<'a>>(&mut self) -> Option<T>;

  fn skip<T: Skippable>(&mut self, mode: ObjectReadMode) -> Option<&'a [u8]>;

  fn skip_white_spaces(&mut self);

  fn read_white_space(&mut self) -> Option<()>;

  fn skip_eol_characters(&mut self);

  fn skip_white_spaces_and_comments(&mut self);
}

impl<'a> ReaderExt<'a> for Reader<'a> {
  #[inline]
  fn read<T: Readable<'a>>(&mut self, ctx: &ReaderContext<'a>) -> Option<T> {
    let Ok(old_offset) = self.fixed_offset();

    match T::read(self, ctx) {
      Some(object) => Some(object),
      None => {
        let Ok(offset) = index(old_offset);
        let Ok(()) = self.jump(offset);
        None
      }
    }
  }

  #[inline]
  fn read_with_context<T: Readable<'a>>(&mut self, ctx: &ReaderContext<'a>) -> Option<T> {
    self.read::<T>(ctx)
  }

  #[inline]
  fn read_without_context<T: Readable<'a>>(&mut self) -> Option<T> {
    let Ok(dummy_ref) = ReaderContext::dummy_ref();

    self.read::<T>(dummy_ref)
  }

  #[inline]
  fn skip<T: Skippable>(&mut self, mode: ObjectReadMode) -> Option<&'a [u8]> {
    let Ok(old_offset) = self.fixed_offset();
    let Ok(start) = index(old_offset);

    match T::skip(self, mode) {
      Some(()) => self.data.get(start..self.offset),
      None => {
        let Ok(()) = self.jump(start);
        None
      }
    }
  }

  #[inline]
  fn skip_white_spaces(&mut self) {
    let Ok(()) = self.forward_while(|byte| {
      let Ok(space) = is_white_space_character(byte);
      space == crate::IsWhiteSpaceCharacterAnswer::Yes
    });
  }

  #[inline]
  fn read_white_space(&mut self) -> Option<()> {
    let Ok(peek_byte) = self.peek_byte();
    let checked_value = peek_byte?;
    let Ok(whitespace) = is_white_space_character(checked_value);

    match whitespace {
      crate::IsWhiteSpaceCharacterAnswer::Yes => {
        let Ok(byte) = self.read_byte();

        let checked_value_2 = byte?;

        match checked_value_2 {
          b'\r' => {
            let Ok(_line_feed) = self.forward_tag(b"\n");
          }
          _ => {}
        }

        Some(())
      }
      crate::IsWhiteSpaceCharacterAnswer::No => None,
    }
  }

  #[inline]
  fn skip_eol_characters(&mut self) {
    let Ok(()) = self.forward_while(|byte| {
      let Ok(end_of_line) = is_eol_character(byte);
      end_of_line == crate::IsEolCharacterAnswer::Yes
    });
  }

  #[inline]
  fn skip_white_spaces_and_comments(&mut self) {
    let Ok(tail) = self.tail();
    let remaining = match tail {
      Some(bytes) => bytes,
      None => return,
    };

    for _ in remaining {
      let Ok(next) = self.peek_byte();
      let byte = match next {
        Some(byte) => byte,
        None => return,
      };
      let Ok(whitespace) = is_white_space_character(byte);

      match (whitespace, byte) {
        (crate::IsWhiteSpaceCharacterAnswer::Yes, _) => self.skip_white_spaces(),
        (crate::IsWhiteSpaceCharacterAnswer::No, b'%') => {
          let _comment = Comment::skip(self, ObjectReadMode::ContentStream);
        }
        (crate::IsWhiteSpaceCharacterAnswer::No, _) => return,
      }
    }
  }
}

#[derive(Clone, Debug)]
struct ReaderContextData<'a> {
  xref: &'a XRef,
  storage: ObjectStorage,
  obj_number: Option<ObjectIdentifier>,
  parent_chain: Vec<ObjectIdentifier>,
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Parsed objects share an immutable reader snapshot and keep their compact public sizes"
  )
)]
pub struct ReaderContext<'a> {
  mode: ObjectReadMode,
  data: Option<Arc<ReaderContextData<'a>>>,
}

impl<'a> ReaderContext<'a> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Parsed objects share an immutable reader snapshot without cloning its reference chain"
    )
  )]
  pub(crate) fn new(xref: &'a XRef, mode: ObjectReadMode) -> Result<Self, Never> {
    Ok(Self {
      mode,
      data: Some(Arc::new(ReaderContextData {
        xref,
        storage: ObjectStorage::Standalone,
        obj_number: None,
        parent_chain: Vec::new(),
      })),
    })
  }

  pub fn dummy() -> Result<Self, Never> {
    Ok(Self {
      mode: ObjectReadMode::Document,
      data: None,
    })
  }

  pub(crate) fn dummy_ref() -> Result<&'static Self, Never> {
    Ok(&Self {
      mode: ObjectReadMode::ContentStream,
      data: None,
    })
  }

  #[inline]
  pub(crate) fn xref(&self) -> Result<&'a XRef, Never> {
    match &self.data {
      Some(data) => Ok(data.xref),
      None => XRef::dummy(),
    }
  }

  #[inline]
  pub(crate) fn read_mode(&self) -> Result<ObjectReadMode, Never> {
    Ok(self.mode)
  }

  #[inline]
  pub(crate) fn object_storage(&self) -> Result<ObjectStorage, Never> {
    Ok(match &self.data {
      Some(data) => data.storage,
      None => ObjectStorage::Standalone,
    })
  }

  #[inline]
  pub(crate) fn obj_number(&self) -> Result<Option<ObjectIdentifier>, Never> {
    Ok(match &self.data {
      Some(data) => data.obj_number,
      None => None,
    })
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Copy-on-write changes the local snapshot without modifying the containing object"
    )
  )]
  pub(crate) fn set_obj_number(&mut self, id: ObjectIdentifier) -> Result<(), Never> {
    match &mut self.data {
      Some(data) => Arc::make_mut(data).obj_number = Some(id),
      None => {}
    }

    Ok(())
  }

  #[inline]
  pub(crate) fn set_read_mode(&mut self, mode: ObjectReadMode) -> Result<(), Never> {
    self.mode = mode;
    Ok(())
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Copy-on-write changes storage for this nested reader only"
    )
  )]
  pub(crate) fn set_object_storage(&mut self, storage: ObjectStorage) -> Result<(), Never> {
    match &mut self.data {
      Some(data) => Arc::make_mut(data).storage = storage,
      None => {}
    }

    Ok(())
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Copy-on-write preserves the containing snapshot while adding one bounded parent"
    )
  )]
  pub(crate) fn push_parent(
    &mut self,
    id: ObjectIdentifier,
  ) -> Result<(), IndirectReferenceError> {
    const MAX_INDIRECT_DEPTH: u64 = 128;

    let data = match &mut self.data {
      Some(data) => data,
      None => return Ok(()),
    };

    match ::core::primitive::bool::then_some(data.parent_chain.contains(&id), ()) {
      ::core::option::Option::Some(()) => {
        return Err(IndirectReferenceError::Cycle);
      }
      ::core::option::Option::None => {}
    }

    let Ok(depth) = console_core_number_conversion::fixed_index(data.parent_chain.len());

    match ::core::primitive::bool::then_some(depth >= MAX_INDIRECT_DEPTH, ()) {
      ::core::option::Option::Some(()) => {
        return Err(IndirectReferenceError::DepthLimit);
      }
      ::core::option::Option::None => {}
    }

    Arc::make_mut(data).parent_chain.push(id);
    Ok(())
  }
}

pub(crate) enum IndirectReferenceError {
  Cycle,
  DepthLimit,
}

pub trait Readable<'a>: Sized {
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self>;

  fn from_bytes_impl(
    b: &'a [u8],
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(mut r) = Reader::new(b);

    let Ok(dummy) = ReaderContext::dummy();

    Ok(Self::read(&mut r, &dummy))
  }
}

pub trait Skippable {
  fn skip(r: &mut Reader<'_>, mode: ObjectReadMode) -> Option<()>;
}

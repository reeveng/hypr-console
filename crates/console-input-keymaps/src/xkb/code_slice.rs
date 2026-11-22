use crate::xkb::code::Code;
use crate::xkb::code::display_text;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use std::borrow::Borrow;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;
use std::ops::Range;

#[derive(Clone)]
enum OwnedOrBorrowed<'a> {
  Owned(Code),
  Borrowed(&'a Code),
}

#[derive(Clone)]
pub(crate) struct CodeSlice<'a> {
  code: OwnedOrBorrowed<'a>,
  range: Range<u32>,
}

impl<'a> CodeSlice<'a> {
  pub(crate) fn new_ref(code: &'a Code, range: Range<u32>) -> Result<Self, Never> {
    Ok(Self {
      code: OwnedOrBorrowed::Borrowed(code),
      range,
    })
  }

  pub(crate) fn new_owned(code: Code) -> Result<Self, Never> {
    let Ok(length) = fitted::<_, u32>(code.len());

    Ok(Self {
      code: OwnedOrBorrowed::Owned(code),
      range: 0..length,
    })
  }

  pub(crate) fn code(&self) -> Result<&Code, Never> {
    Ok(match &self.code {
      OwnedOrBorrowed::Owned(c) => c,
      OwnedOrBorrowed::Borrowed(c) => c,
    })
  }

  pub(crate) fn as_bytes(&self) -> Result<&[u8], Never> {
    let Ok(code) = self.code();
    let Ok(start) = index(self.range.start);
    let Ok(end) = index(self.range.end);

    Ok(match code.get(start..end) {
      Some(bytes) => bytes,
      None => &[],
    })
  }

  pub(crate) fn to_owned(&self) -> Result<CodeSlice<'static>, Never> {
    let Ok(bytes) = self.as_bytes();
    let Ok(code) = Code::owning(bytes.to_vec());
    CodeSlice::new_owned(code)
  }

  pub(crate) fn slice(&self, range: Range<u32>) -> Result<CodeSlice<'_>, Never> {
    let Ok(code) = self.code();

    CodeSlice::new_ref(
      code,
      Range {
        start: self.range.start.saturating_add(range.start),
        end: self.range.start.saturating_add(range.end),
      },
    )
  }
}

impl AsRef<[u8]> for CodeSlice<'_> {
  fn as_ref(&self) -> &[u8] {
    self
  }
}

impl Deref for CodeSlice<'_> {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    let Ok(as_bytes) = self.as_bytes();
    as_bytes
  }
}

impl Borrow<[u8]> for CodeSlice<'_> {
  fn borrow(&self) -> &[u8] {
    let Ok(as_bytes) = self.as_bytes();
    as_bytes
  }
}

impl Debug for CodeSlice<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let Ok(text) = display_text(self);
    Debug::fmt(&text, f)
  }
}

impl PartialEq for CodeSlice<'_> {
  fn eq(&self, other: &Self) -> bool {
    let Ok(bytes) = self.as_bytes();
    let Ok(other_bytes) = other.as_bytes();

    bytes == other_bytes
  }
}

impl Eq for CodeSlice<'_> {}

impl Hash for CodeSlice<'_> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    let Ok(bytes) = self.as_bytes();

    bytes.hash(state);
  }
}

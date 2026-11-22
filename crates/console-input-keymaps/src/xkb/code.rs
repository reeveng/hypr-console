use crate::xkb::code_slice::CodeSlice;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::borrow::Cow;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::ops::Deref;

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Code {
  code: Vec<u8>,
}

impl Code {
  pub(crate) fn owning(code: Vec<u8>) -> Result<Self, Never> {
    Ok(Self { code })
  }

  pub(crate) fn as_bytes(&self) -> Result<&[u8], Never> {
    Ok(self.code.as_slice())
  }

  pub(crate) fn to_slice(&self) -> Result<CodeSlice<'_>, Never> {
    let Ok(length) = fitted::<_, u32>(self.len());
    CodeSlice::new_ref(self, 0..length)
  }
}

impl Deref for Code {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    let Ok(as_bytes) = self.as_bytes();
    as_bytes
  }
}

impl Debug for Code {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let Ok(text) = display_text(self);
    Debug::fmt(&text, f)
  }
}

pub(crate) fn display_text(bytes: &[u8]) -> Result<Cow<'_, str>, Never> {
  match std::str::from_utf8(bytes) {
    Ok(valid) => Ok(Cow::Borrowed(valid)),
    Err(_invalid) => {
      let mut text = String::with_capacity(bytes.len());

      for chunk in bytes.utf8_chunks() {
        text.push_str(chunk.valid());

        match chunk.invalid() {
          [] => {}
          [_, ..] => text.push(char::REPLACEMENT_CHARACTER),
        }
      }

      Ok(Cow::Owned(text))
    }
  }
}

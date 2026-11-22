//! Comments and white spaces.

use crate::reader::Reader;
use crate::reader::{Readable, ReaderContext, ReaderExt, Skippable};
use crate::{
  IsEolCharacterAnswer, IsRegularCharacterAnswer, IsWhiteSpaceCharacterAnswer, ObjectReadMode,
};

#[inline(always)]
pub(crate) fn is_white_space_character(
  byte: u8,
) -> Result<IsWhiteSpaceCharacterAnswer, console_core_never::Never> {
  Ok(match byte {
    0x00 | 0x09 | 0x0a | 0x0c | 0x0d | b' ' => IsWhiteSpaceCharacterAnswer::Yes,
    _ => IsWhiteSpaceCharacterAnswer::No,
  })
}

#[inline(always)]
pub(crate) fn is_regular_character(
  byte: u8,
) -> Result<IsRegularCharacterAnswer, console_core_never::Never> {
  Ok(match byte {
    0x00 | 0x09 | 0x0a | 0x0c | 0x0d | b' ' | b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{'
    | b'}' | b'/' | b'%' => IsRegularCharacterAnswer::No,
    _ => IsRegularCharacterAnswer::Yes,
  })
}

#[inline(always)]
pub(crate) fn is_eol_character(
  char: u8,
) -> core::result::Result<IsEolCharacterAnswer, console_core_never::Never> {
  Ok(match char {
    0x0a | 0x0d => IsEolCharacterAnswer::Yes,
    _ => IsEolCharacterAnswer::No,
  })
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub(crate) struct Comment<'a>(pub(crate) &'a [u8]);

impl Skippable for Comment<'_> {
  fn skip(r: &mut Reader<'_>, _: ObjectReadMode) -> Option<()> {
    let Ok(forward_tag) = r.forward_tag(b"%");
    forward_tag?;
    let Ok(()) = r.forward_while(|b| {
      !matches!(
        {
          let Ok(explicit_value) = is_eol_character(b);
          explicit_value
        },
        IsEolCharacterAnswer::Yes
      )
    });

    Some(())
  }
}

impl<'a> Readable<'a> for Comment<'a> {
  fn read(r: &mut Reader<'a>, _: &ReaderContext<'_>) -> Option<Self> {
    let bytes = r.skip::<Comment<'_>>(ObjectReadMode::Document)?;
    let bytes_2 = bytes.strip_prefix(b"%")?;

    Some(Comment(bytes_2))
  }
}

use std::{error::Error, path::Path};

use console_core_never::Never;

use super::UpdateError;

#[derive(Clone)]
pub struct PreviousRelease {
  pub target: String,
  pub label: String,
}

#[derive(Debug)]
pub enum UndoFault {
  InvalidTarget,
  Absent,
  Changed,
  Pending,
}

impl std::fmt::Display for UndoFault {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    to.write_str(match self {
      Self::InvalidTarget => "invalid undo target",
      Self::Absent => "there is no previous release",
      Self::Changed => "the previous release changed. Open Console Updates again",
      Self::Pending => "a release switch is already pending",
    })
  }
}

impl Error for UndoFault {}

pub fn undo_target(target: &str) -> Result<&str, UndoFault> {
  let inner = target
    .strip_prefix("undo:")
    .ok_or(UndoFault::InvalidTarget)?;
  let (slot, revision) = inner.split_once(':').ok_or(UndoFault::InvalidTarget)?;
  let _slot = ReleaseSlot::parse(slot).map_err(|_| UndoFault::InvalidTarget)?;

  match hex::<40>(revision) {
    Ok(Hex::Valid) => Ok(slot),
    Ok(Hex::Invalid) => Err(UndoFault::InvalidTarget),
  }
}

pub struct ReleaseSlot<'a>(&'a str);

impl<'a> ReleaseSlot<'a> {
  pub fn parse(slot: &'a str) -> Result<Self, UpdateError> {
    let parts = slot
      .strip_prefix("release-")
      .ok_or("bad release storage name")?;
    let (process, time) = parts.split_once('-').ok_or("bad release storage name")?;
    process.parse::<u32>()?;
    time.parse::<u128>()?;

    Ok(Self(slot))
  }
}

impl AsRef<Path> for ReleaseSlot<'_> {
  fn as_ref(&self) -> &Path {
    Path::new(self.0)
  }
}

pub enum Hex {
  Valid,
  Invalid,
}

pub fn hex<const LENGTH: u8>(text: &str) -> Result<Hex, Never> {
  let valid = u8::try_from(text.len()) == Ok(LENGTH)
    && text
      .bytes()
      .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));

  Ok(match bool::then_some(valid, ()) {
    Some(()) => Hex::Valid,
    None => Hex::Invalid,
  })
}

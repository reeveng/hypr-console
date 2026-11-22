//! Which string in the `name` table: the copyright notice, the family name, the sample text.
//!
//! The specification numbers them as `u16`s and they are a type here so a name's identifier is
//! not taken for a count. The ones up to 255 are reserved for the meanings the specification gives
//! them and the rest are the font's own, which is the [`NameIdKind`] one answers. The predefined
//! ones -- 0 to 25, less 15, which was never assigned -- are named constants that print as their
//! names. Anything above 32767 is not a name identifier at all, which is why stepping past it
//! answers `None`. The default is `0xFFFF`, the identifier no name can have.

use console_core_never::Never;
use core::fmt;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::AnyBitPattern))]
#[repr(transparent)]
pub struct NameId(u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameIdKind {
  Standard,
  FontSpecific,
}

impl NameId {
  const LAST_RESERVED_ID: u16 = 255;
  const LAST_ALLOWED_ID: u16 = 32767;

  pub const COPYRIGHT_NOTICE: Self = Self(0);
  pub const FAMILY_NAME: Self = Self(1);
  pub const SUBFAMILY_NAME: Self = Self(2);
  pub const UNIQUE_ID: Self = Self(3);
  pub const FULL_NAME: Self = Self(4);
  pub const VERSION_STRING: Self = Self(5);
  pub const POSTSCRIPT_NAME: Self = Self(6);
  pub const TRADEMARK: Self = Self(7);
  pub const MANUFACTURER: Self = Self(8);
  pub const DESIGNER: Self = Self(9);
  pub const DESCRIPTION: Self = Self(10);
  pub const VENDOR_URL: Self = Self(11);
  pub const DESIGNER_URL: Self = Self(12);
  pub const LICENSE_DESCRIPTION: Self = Self(13);
  pub const LICENSE_URL: Self = Self(14);
  pub const TYPOGRAPHIC_FAMILY_NAME: Self = Self(16);
  pub const TYPOGRAPHIC_SUBFAMILY_NAME: Self = Self(17);
  pub const COMPATIBLE_FULL_NAME: Self = Self(18);
  pub const SAMPLE_TEXT: Self = Self(19);
  pub const POSTSCRIPT_CID_NAME: Self = Self(20);
  pub const WWS_FAMILY_NAME: Self = Self(21);
  pub const WWS_SUBFAMILY_NAME: Self = Self(22);
  pub const LIGHT_BACKGROUND_PALETTE: Self = Self(23);
  pub const DARK_BACKGROUND_PALETTE: Self = Self(24);
  pub const VARIATIONS_POSTSCRIPT_NAME_PREFIX: Self = Self(25);
  pub const LAST_RESERVED_NAME_ID: Self = Self(Self::LAST_RESERVED_ID);
  pub const LAST_ALLOWED_NAME_ID: Self = Self(Self::LAST_ALLOWED_ID);
}

impl NameId {
  #[inline]
  pub const fn new(raw: u16) -> Result<Self, Never> {
    Ok(Self(raw))
  }

  #[inline]
  pub fn predefined() -> Result<impl Iterator<Item = Self> + Clone, Never> {
    Ok((0..15).chain(16..=25).map(Self))
  }

  #[inline]
  pub const fn to_u16(self) -> Result<u16, Never> {
    Ok(self.0)
  }

  #[inline]
  pub const fn kind(self) -> Result<NameIdKind, Never> {
    Ok(match self.0 {
      ..=Self::LAST_RESERVED_ID => NameIdKind::Standard,
      _ => NameIdKind::FontSpecific,
    })
  }

  #[inline]
  pub const fn checked_add(self, rhs: u16) -> Result<Option<Self>, Never> {
    let stepped = Self(self.0.saturating_add(rhs));

    Ok(match stepped.0 {
      ..=Self::LAST_ALLOWED_ID => Some(stepped),
      _ => None,
    })
  }

  #[inline]
  pub const fn to_be_bytes(self) -> Result<[u8; 2], Never> {
    Ok(self.0.to_be_bytes())
  }
}

impl Default for NameId {
  fn default() -> Self {
    Self(0xFFFF)
  }
}

impl From<u16> for NameId {
  fn from(value: u16) -> Self {
    Self(value)
  }
}

impl fmt::Debug for NameId {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let name = match *self {
      Self::COPYRIGHT_NOTICE => Some("COPYRIGHT_NOTICE"),
      Self::FAMILY_NAME => Some("FAMILY_NAME"),
      Self::SUBFAMILY_NAME => Some("SUBFAMILY_NAME"),
      Self::UNIQUE_ID => Some("UNIQUE_ID"),
      Self::FULL_NAME => Some("FULL_NAME"),
      Self::VERSION_STRING => Some("VERSION_STRING"),
      Self::POSTSCRIPT_NAME => Some("POSTSCRIPT_NAME"),
      Self::TRADEMARK => Some("TRADEMARK"),
      Self::MANUFACTURER => Some("MANUFACTURER"),
      Self::DESIGNER => Some("DESIGNER"),
      Self::DESCRIPTION => Some("DESCRIPTION"),
      Self::VENDOR_URL => Some("VENDOR_URL"),
      Self::DESIGNER_URL => Some("DESIGNER_URL"),
      Self::LICENSE_DESCRIPTION => Some("LICENSE_DESCRIPTION"),
      Self::LICENSE_URL => Some("LICENSE_URL"),
      Self::TYPOGRAPHIC_FAMILY_NAME => Some("TYPOGRAPHIC_FAMILY_NAME"),
      Self::TYPOGRAPHIC_SUBFAMILY_NAME => Some("TYPOGRAPHIC_SUBFAMILY_NAME"),
      Self::COMPATIBLE_FULL_NAME => Some("COMPATIBLE_FULL_NAME"),
      Self::SAMPLE_TEXT => Some("SAMPLE_TEXT"),
      Self::POSTSCRIPT_CID_NAME => Some("POSTSCRIPT_CID_NAME"),
      Self::WWS_FAMILY_NAME => Some("WWS_FAMILY_NAME"),
      Self::WWS_SUBFAMILY_NAME => Some("WWS_SUBFAMILY_NAME"),
      Self::LIGHT_BACKGROUND_PALETTE => Some("LIGHT_BACKGROUND_PALETTE"),
      Self::DARK_BACKGROUND_PALETTE => Some("DARK_BACKGROUND_PALETTE"),
      Self::VARIATIONS_POSTSCRIPT_NAME_PREFIX => Some("VARIATIONS_POSTSCRIPT_NAME_PREFIX"),
      _unnamed => None,
    };

    match name {
      Some(name_2) => f.write_str(name_2),
      None => write!(f, "NameId {}", self.0),
    }
  }
}

impl fmt::Display for NameId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

crate::raw::newtype_scalar!(NameId, [u8; 2]);

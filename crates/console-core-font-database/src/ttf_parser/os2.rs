//! A font's `OS/2` table, read for its weight class, its width class and whether it slants.
//!
//! The table is taken when it is at least as long as its version says, and not
//! asked to be exactly that long, because some fonts count padding in the
//! length their table record gives. A width class outside one to nine is
//! normal. Italic is the first selection bit; oblique is the tenth, and only
//! from version four, where the bit was given that meaning.

use super::parser::Stream;
use console_core_never::Never;
use core::cmp::Ordering;

const WEIGHT_CLASS_OFFSET: u64 = 4;
const WIDTH_CLASS_OFFSET: u64 = 6;
const SELECTION_OFFSET: u64 = 62;
const ITALIC: u16 = 1;
const OBLIQUE: u16 = 1 << 9;
const OBLIQUE_FROM_VERSION: u8 = 4;

#[derive(Clone, Copy, Eq, PartialEq, Debug, Hash, Default)]
pub enum Weight {
  Thin,
  ExtraLight,
  Light,
  #[default]
  Normal,
  Medium,
  SemiBold,
  Bold,
  ExtraBold,
  Black,
  Other(u16),
}

impl Weight {
  #[inline]
  pub fn to_number(self) -> Result<u16, Never> {
    Ok(match self {
      Weight::Thin => 100,
      Weight::ExtraLight => 200,
      Weight::Light => 300,
      Weight::Normal => 400,
      Weight::Medium => 500,
      Weight::SemiBold => 600,
      Weight::Bold => 700,
      Weight::ExtraBold => 800,
      Weight::Black => 900,
      Weight::Other(number) => number,
    })
  }
}

impl From<u16> for Weight {
  #[inline]
  fn from(value: u16) -> Self {
    match value {
      100 => Weight::Thin,
      200 => Weight::ExtraLight,
      300 => Weight::Light,
      400 => Weight::Normal,
      500 => Weight::Medium,
      600 => Weight::SemiBold,
      700 => Weight::Bold,
      800 => Weight::ExtraBold,
      900 => Weight::Black,
      _ => Weight::Other(value),
    }
  }
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug, Hash, Default)]
pub enum Width {
  UltraCondensed,
  ExtraCondensed,
  Condensed,
  SemiCondensed,
  #[default]
  Normal,
  SemiExpanded,
  Expanded,
  ExtraExpanded,
  UltraExpanded,
}

impl Width {
  #[inline]
  pub fn to_number(self) -> Result<u16, Never> {
    Ok(match self {
      Width::UltraCondensed => 1,
      Width::ExtraCondensed => 2,
      Width::Condensed => 3,
      Width::SemiCondensed => 4,
      Width::Normal => 5,
      Width::SemiExpanded => 6,
      Width::Expanded => 7,
      Width::ExtraExpanded => 8,
      Width::UltraExpanded => 9,
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub enum Style {
  #[default]
  Normal,

  Italic,

  Oblique,
}

#[derive(Clone, Copy)]
pub struct Table<'a> {
  pub version: u8,
  data: &'a [u8],
}

impl<'a> Table<'a> {
  pub fn parse(data: &'a [u8]) -> Result<Option<Self>, Never> {
    let Ok(mut s) = Stream::new(data);
    let Ok(read_version) = s.read::<u16>();
    let (table_len, version) = match read_version {
      Some(0) => (78u64, 0u8),
      Some(1) => (86, 1),
      Some(2) => (96, 2),
      Some(3) => (96, 3),
      Some(4) => (96, 4),
      Some(5) => (100, 5),
      Some(_) | None => return Ok(None),
    };
    let Ok(data_len) = console_core_number_conversion::fitted::<_, u64>(data.len());

    Ok(match data_len.cmp(&table_len) {
      Ordering::Less => None,
      Ordering::Equal | Ordering::Greater => Some(Table { version, data }),
    })
  }

  #[inline]
  pub fn weight(&self) -> Result<Weight, Never> {
    let Ok(weight_class) = self.word_at(WEIGHT_CLASS_OFFSET);

    Ok(Weight::from(weight_class))
  }

  #[inline]
  pub fn width(&self) -> Result<Width, Never> {
    let Ok(width_class) = self.word_at(WIDTH_CLASS_OFFSET);

    Ok(match width_class {
      1 => Width::UltraCondensed,
      2 => Width::ExtraCondensed,
      3 => Width::Condensed,
      4 => Width::SemiCondensed,
      5 => Width::Normal,
      6 => Width::SemiExpanded,
      7 => Width::Expanded,
      8 => Width::ExtraExpanded,
      9 => Width::UltraExpanded,
      _ => Width::Normal,
    })
  }

  pub fn style(&self) -> Result<Style, Never> {
    let Ok(selection) = self.word_at(SELECTION_OFFSET);

    Ok(
      match (selection & ITALIC, selection & OBLIQUE, self.version) {
        (ITALIC, _, _) => Style::Italic,
        (0, OBLIQUE, OBLIQUE_FROM_VERSION..) => Style::Oblique,
        _ => Style::Normal,
      },
    )
  }

  fn word_at(&self, offset: u64) -> Result<u16, Never> {
    let Ok(word) = Stream::read_at::<u16>(self.data, offset);

    Ok(match word {
      Some(word_2) => word_2,
      None => 0,
    })
  }
}

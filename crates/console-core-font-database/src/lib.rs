//! The font faces a machine has, and which of them serves a CSS family and style.
//!
//! A font file is one face or a collection of them, and what is kept is the face:
//! its families, its PostScript name, its style, weight and stretch, and whether
//! it is monospaced, read once when it is loaded. Nothing else of a face is parsed
//! here; a caller that wants its outlines asks for its bytes. A query names
//! families in order, and the first of them that any face carries is the family
//! the answer comes from, so a later family is a fallback rather than a vote.
//!
//! Faces are read from bytes handed in, from a file, from a directory walked
//! whole, or from wherever fontconfig says this machine keeps its fonts. A face
//! read from a file keeps only the path and reads the file again whenever its
//! bytes are asked for. `make_shared_face_data` keeps one mapping instead, and a
//! caller of it promises that the file is not changed for as long as any face
//! shares that mapping.
//!
//! [`Database::new_in`] clears the caller's storage and uses it for faces.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod faces;
#[cfg(feature = "fs")]
mod files;
mod language;
mod matching;
#[cfg(feature = "fs")]
mod system;
mod table;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

pub use language::Language;
pub use table::{FaceSlot, FaceTableFull, ID};

use alloc::{borrow::Cow, sync::Arc};
use console_core_never::Never;
use table::FaceTable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emptiness {
  Empty,
  NonEmpty,
}

impl From<bool> for Emptiness {
  fn from(empty: bool) -> Self {
    match bool::then_some(empty, ()) {
      Some(()) => Self::Empty,
      None => Self::NonEmpty,
    }
  }
}

#[derive(Clone, Debug)]
pub struct Database<S = Vec<FaceSlot>> {
  faces: FaceTable<S>,
  family_serif: Cow<'static, str>,
  family_sans_serif: Cow<'static, str>,
  family_cursive: Cow<'static, str>,
  family_fantasy: Cow<'static, str>,
  family_monospace: Cow<'static, str>,
}

impl Default for Database {
  fn default() -> Self {
    let Ok(database) = Self::new();

    database
  }
}

impl Database {
  #[inline]
  pub fn new() -> Result<Self, Never> {
    Self::from_slots(Vec::new())
  }

  pub fn load_font_data(&mut self, data: Vec<u8>) -> Result<(), Never> {
    let Ok(bytes) = shared_font_data(data);
    let Ok(_loaded) = self.load_font_source(Source::Binary(bytes));

    Ok(())
  }

  pub fn load_font_source(&mut self, source: Source) -> Result<Vec<ID>, Never> {
    let Ok(read) = source.with_data(|data| {
      let Ok(faces) = faces::read(&source, data, &"source");

      faces
    });
    let mut loaded = Vec::new();

    for face in read.into_iter().flatten() {
      match self.push_face_info(face) {
        Ok(id) => loaded.push(id),
        Err(full) => {
          console_core_logging::warn!("loading font source: {full}");
          break;
        }
      }
    }

    Ok(loaded)
  }

  pub fn push_face_info(&mut self, info: FaceInfo) -> Result<ID, FaceTableFull> {
    self.faces.insert(info)
  }
}

impl<'a> Database<&'a mut [FaceSlot]> {
  pub fn new_in(slots: &'a mut [FaceSlot]) -> Result<Self, Never> {
    slots.fill_with(|| FaceSlot::EMPTY);

    Self::from_slots(slots)
  }

  pub fn push_face_info(&mut self, info: FaceInfo) -> Result<ID, FaceTableFull> {
    self.faces.try_insert(info)
  }
}

impl<S: AsMut<[FaceSlot]>> Database<S> {
  pub fn remove_face(&mut self, id: ID) -> Result<(), Never> {
    let Ok(_removed) = self.faces.remove(id);

    Ok(())
  }
}

impl<Storage> Database<Storage> {
  fn from_slots(slots: Storage) -> Result<Self, Never> {
    let Ok(faces) = FaceTable::new(slots);

    Ok(Self {
      faces,
      family_serif: Cow::Borrowed("Times New Roman"),
      family_sans_serif: Cow::Borrowed("Arial"),
      family_cursive: Cow::Borrowed("Comic Sans MS"),
      family_fantasy: Cow::Borrowed("Impact"),
      family_monospace: Cow::Borrowed("Courier New"),
    })
  }

  #[inline]
  pub fn is_empty(&self) -> Result<Emptiness, Never> {
    let Ok(length) = self.faces.len();

    Ok(Emptiness::from(length == 0))
  }

  #[allow(
    clippy::len_without_is_empty,
    reason = "is_empty answers an Emptiness, which is not the bool clippy looks for"
  )]
  #[inline]
  pub fn len(&self) -> Result<u32, Never> {
    self.faces.len()
  }

  pub fn set_serif_family<S>(&mut self, family: S) -> Result<(), Never>
  where
    String: From<S>,
  {
    self.family_serif = Cow::Owned(String::from(family));

    Ok(())
  }

  pub fn set_sans_serif_family<S>(&mut self, family: S) -> Result<(), Never>
  where
    String: From<S>,
  {
    self.family_sans_serif = Cow::Owned(String::from(family));

    Ok(())
  }

  pub fn set_cursive_family<S>(&mut self, family: S) -> Result<(), Never>
  where
    String: From<S>,
  {
    self.family_cursive = Cow::Owned(String::from(family));

    Ok(())
  }

  pub fn set_fantasy_family<S>(&mut self, family: S) -> Result<(), Never>
  where
    String: From<S>,
  {
    self.family_fantasy = Cow::Owned(String::from(family));

    Ok(())
  }

  pub fn set_monospace_family<S>(&mut self, family: S) -> Result<(), Never>
  where
    String: From<S>,
  {
    self.family_monospace = Cow::Owned(String::from(family));

    Ok(())
  }

  pub fn family_name<'a>(&'a self, family: &'a Family) -> Result<&'a str, Never> {
    Ok(match family {
      Family::Name(name) => name,
      Family::Serif => self.family_serif.as_ref(),
      Family::SansSerif => self.family_sans_serif.as_ref(),
      Family::Cursive => self.family_cursive.as_ref(),
      Family::Fantasy => self.family_fantasy.as_ref(),
      Family::Monospace => self.family_monospace.as_ref(),
    })
  }
}

impl<S: AsRef<[FaceSlot]>> Database<S> {
  pub fn query(&self, query: &Query) -> Result<Option<ID>, Never> {
    let mut preference = alloc::collections::BTreeMap::new();

    for (family, place) in query.families.iter().zip(0u64..) {
      let Ok(name) = self.family_name(family);

      preference.entry(name).or_insert(place);
    }

    let Ok(faces) = self.faces.iter();
    let carried = faces.filter_map(|face| {
      let earliest = face
        .families
        .iter()
        .filter_map(|(name, _)| preference.get(name.as_str()).copied())
        .min();

      earliest.map(|place| (place, face))
    });
    let best = carried.min_by_key(|(place, face)| {
      let Ok(rank) = matching::Rank::of(face, query);

      (*place, rank)
    });

    Ok(best.map(|(_, face)| face.id))
  }

  #[inline]
  pub fn faces(&self) -> Result<impl Iterator<Item = &FaceInfo> + '_, Never> {
    self.faces.iter()
  }

  pub fn face(&self, id: ID) -> Result<Option<&FaceInfo>, Never> {
    self.faces.get(id)
  }

  pub fn face_source(&self, id: ID) -> Result<Option<(Source, u32)>, Never> {
    let Ok(face) = self.face(id);

    Ok(face.map(|info| (info.source.clone(), info.index)))
  }

  pub fn with_face_data<P, T>(&self, id: ID, p: P) -> Result<Option<T>, Never>
  where
    P: FnOnce(&[u8], u32) -> T,
  {
    let Ok(found) = self.face_source(id);
    let (source, face_index) = match found {
      Some(found_2) => found_2,
      None => return Ok(None),
    };

    source.with_data(|data| p(data, face_index))
  }
}

#[derive(Clone, Debug)]
pub struct FaceInfo {
  pub id: ID,

  pub source: Source,

  pub index: u32,

  pub families: Vec<(String, Language)>,

  pub post_script_name: String,

  pub style: Style,

  pub weight: Weight,

  pub stretch: Stretch,

  pub pitch: Pitch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pitch {
  Fixed,
  Proportional,
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "faces and layout clients can retain the same immutable font bytes independently without copying a whole font collection"
  )
)]
pub type SharedFontData = Arc<dyn AsRef<[u8]> + Sync + Send>;

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "constructs the shared immutable storage used by in-memory font collections and explicitly pinned file mappings"
  )
)]
fn shared_font_data(
  data: impl AsRef<[u8]> + Sync + Send + 'static,
) -> Result<SharedFontData, Never> {
  Ok(Arc::new(data))
}

#[derive(Clone)]
pub enum Source {
  Binary(SharedFontData),

  #[cfg(feature = "fs")]
  File(std::path::PathBuf),

  #[cfg(all(feature = "fs", feature = "memmap"))]
  SharedFile(std::path::PathBuf, SharedFontData),
}

impl core::fmt::Debug for Source {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Self::Binary(data) => f
        .debug_tuple("SharedBinary")
        .field(&data.as_ref().as_ref())
        .finish(),
      #[cfg(feature = "fs")]
      Self::File(path) => f.debug_tuple("File").field(path).finish(),
      #[cfg(all(feature = "fs", feature = "memmap"))]
      Self::SharedFile(path, data) => f
        .debug_tuple("SharedFile")
        .field(path)
        .field(&data.as_ref().as_ref())
        .finish(),
    }
  }
}

impl Source {
  fn with_data<P, T>(&self, p: P) -> Result<Option<T>, Never>
  where
    P: FnOnce(&[u8]) -> T,
  {
    match self {
      #[cfg(feature = "fs")]
      Source::File(path) => files::with_file_data(path, p),
      Source::Binary(data) => Ok(Some(p(data.as_ref().as_ref()))),
      #[cfg(all(feature = "fs", feature = "memmap"))]
      Source::SharedFile(_, data) => Ok(Some(p(data.as_ref().as_ref()))),
    }
  }
}

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq, Hash)]
pub struct Query<'a> {
  pub families: &'a [Family<'a>],

  pub weight: Weight,

  pub stretch: Stretch,

  pub style: Style,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family<'a> {
  Name(&'a str),

  Serif,

  SansSerif,

  Cursive,

  Fantasy,

  Monospace,
}

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Hash)]
pub struct Weight(pub u16);

impl Default for Weight {
  #[inline]
  fn default() -> Weight {
    Weight::NORMAL
  }
}

impl Weight {
  pub const THIN: Weight = Weight(100);

  pub const EXTRA_LIGHT: Weight = Weight(200);

  pub const LIGHT: Weight = Weight(300);

  pub const NORMAL: Weight = Weight(400);

  pub const MEDIUM: Weight = Weight(500);

  pub const SEMIBOLD: Weight = Weight(600);

  pub const BOLD: Weight = Weight(700);

  pub const EXTRA_BOLD: Weight = Weight(800);

  pub const BLACK: Weight = Weight(900);
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug, Hash, Default)]
pub enum Stretch {
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

impl Stretch {
  #[inline]
  pub fn to_number(self) -> Result<u16, Never> {
    Ok(match self {
      Stretch::UltraCondensed => 1,
      Stretch::ExtraCondensed => 2,
      Stretch::Condensed => 3,
      Stretch::SemiCondensed => 4,
      Stretch::Normal => 5,
      Stretch::SemiExpanded => 6,
      Stretch::Expanded => 7,
      Stretch::ExtraExpanded => 8,
      Stretch::UltraExpanded => 9,
    })
  }

  pub(crate) fn from_width_class(class: u16) -> Result<Self, Never> {
    Ok(match class {
      1 => Self::UltraCondensed,
      2 => Self::ExtraCondensed,
      3 => Self::Condensed,
      4 => Self::SemiCondensed,
      6 => Self::SemiExpanded,
      7 => Self::Expanded,
      8 => Self::ExtraExpanded,
      9 => Self::UltraExpanded,
      _ => Self::Normal,
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

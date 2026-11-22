//! The maps a PDF reads its text through: a code in a string to the glyph it
//! selects, and a code in a font to the characters it stands for.
//!
//! A cmap is either a PostScript program a PDF carries or one of the
//! predefined ones, which are carried here in a binary form of their own. Both
//! are read into the same thing: a [`CMap`] is a [`Metadata`] and the files
//! that make it up, a file and the files it names with `usecmap` below it, and
//! a lookup asks each in turn until one of them has decided. A file that names
//! another is read before the one it names, and the names are followed in a
//! loop that stops at sixteen deep, so a cycle ends as a fault rather than as
//! a stack that ran out.
//!
//! The crate began as hayro-cmap, by Laurenz Stampfl under MIT or Apache-2.0,
//! carried in `forks/` for the PDF reader, and it came into the tree under the
//! rules. What nobody read was left behind: the cmap's own name, the
//! codespace ranges (still checked to be well formed, as the fork did, and
//! then dropped, since the fork never asked them either), a name for each of
//! the predefined maps, the Unicode maps a family points at, and the feature
//! that left the bundle out. A predefined map is asked for by its name in
//! bytes, the way a PDF writes it, and `usecmap` does not have to be told
//! which of them it is.
//!
//! The fork answered a base cmap's `usecmap` before it had read the rest of
//! the file naming it, and answered every `usecmap` in a file; this reads the
//! file whole and follows the last one. A text cmap that names a base counts
//! one level of nesting and so does a binary one, where the fork counted only
//! the text ones.

mod bcmap;
mod layer;
mod parse;

pub use crate::bcmap::load_embedded;

use std::fmt;

use console_core_never::Never;
use console_core_postscript_tokens::ScanError;

use crate::layer::{Layer, Lookup, Span};

pub type Cid = u32;

const MAX_NESTING_DEPTH: u32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
  Scan(ScanError),
  Truncated,
  BadNumber,
  BadCode,
  BadSegment,
  BadBundle,
  Unresolved,
  TooDeep,
}

impl fmt::Display for ParseError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ParseError::Scan(fault) => write!(to, "the cmap is not readable PostScript: {fault}"),
      ParseError::Truncated => write!(to, "the cmap ends where more was wanted"),
      ParseError::BadNumber => write!(to, "a number in the cmap is out of range"),
      ParseError::BadCode => write!(to, "a code in the cmap is no string of one to four bytes"),
      ParseError::BadSegment => write!(to, "a segment of the binary cmap is not one this reads"),
      ParseError::BadBundle => write!(to, "the bundle of predefined cmaps is damaged"),
      ParseError::Unresolved => write!(to, "the cmap names a base that nobody could give"),
      ParseError::TooDeep => write!(to, "the cmaps name each other past sixteen deep"),
    }
  }
}

impl std::error::Error for ParseError {}

impl From<ScanError> for ParseError {
  fn from(fault: ScanError) -> ParseError {
    ParseError::Scan(fault)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CMapName<'a> {
  IdentityH,
  IdentityV,
  Custom(&'a [u8]),
}

impl<'a> CMapName<'a> {
  pub fn from_bytes(name: &'a [u8]) -> Result<CMapName<'a>, Never> {
    Ok(match name {
      b"Identity-H" => CMapName::IdentityH,
      b"Identity-V" => CMapName::IdentityV,
      _other => CMapName::Custom(name),
    })
  }

  pub fn to_bytes(&self) -> Result<&[u8], Never> {
    Ok(match self {
      CMapName::IdentityH => b"Identity-H",
      CMapName::IdentityV => b"Identity-V",
      CMapName::Custom(name) => name,
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BfString {
  Char(char),
  String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WritingMode {
  #[default]
  Horizontal,
  Vertical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CidFamily {
  AdobeJapan1,
  AdobeGB1,
  AdobeCNS1,
  AdobeKorea1,
  AdobeIdentity,
  Custom {
    registry: Vec<u8>,
    ordering: Vec<u8>,
  },
}

impl CidFamily {
  pub fn from_registry_ordering(registry: &[u8], ordering: &[u8]) -> Result<CidFamily, Never> {
    Ok(match (registry, ordering) {
      (b"Adobe", b"Japan1") => CidFamily::AdobeJapan1,
      (b"Adobe", b"GB1") => CidFamily::AdobeGB1,
      (b"Adobe", b"CNS1") => CidFamily::AdobeCNS1,
      (b"Adobe", b"Korea1") => CidFamily::AdobeKorea1,
      (b"Adobe", b"Identity") => CidFamily::AdobeIdentity,
      (_registry, _ordering) => CidFamily::Custom {
        registry: registry.to_vec(),
        ordering: ordering.to_vec(),
      },
    })
  }

  pub fn ucs2_cmap(&self) -> Result<Option<CMapName<'static>>, Never> {
    Ok(match self {
      CidFamily::AdobeJapan1 => Some(CMapName::Custom(b"Adobe-Japan1-UCS2")),
      CidFamily::AdobeGB1 => Some(CMapName::Custom(b"Adobe-GB1-UCS2")),
      CidFamily::AdobeCNS1 => Some(CMapName::Custom(b"Adobe-CNS1-UCS2")),
      CidFamily::AdobeKorea1 => Some(CMapName::Custom(b"Adobe-Korea1-UCS2")),
      CidFamily::AdobeIdentity | CidFamily::Custom { .. } => None,
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterCollection {
  pub family: CidFamily,
  pub supplement: i32,
}

#[derive(Debug, Clone)]
pub struct Metadata {
  pub character_collection: Option<CharacterCollection>,
  pub writing_mode: Option<WritingMode>,
}

pub(crate) struct Layered {
  pub(crate) metadata: Metadata,
  pub(crate) layer: Layer,
  pub(crate) base: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct CMap {
  pub metadata: Metadata,
  layers: Vec<Layer>,
}

fn layered(data: &[u8]) -> Result<Layered, ParseError> {
  match data.strip_prefix(bcmap::MAGIC) {
    Some(_binary_body) => bcmap::read(data),
    None => parse::read(data),
  }
}

impl CMap {
  pub fn parse<'a>(
    data: &[u8],
    get_cmap: impl Fn(CMapName<'_>) -> Option<&'a [u8]>,
  ) -> Result<CMap, ParseError> {
    let top = layered(data)?;
    let mut layers = vec![top.layer];
    let mut pending = top.base;

    for _nesting in 1..MAX_NESTING_DEPTH {
      let name = match pending.take() {
        Some(name) => name,
        None => break,
      };
      let Ok(wanted) = CMapName::from_bytes(&name);
      let base = match get_cmap(wanted) {
        Some(base) => base,
        None => return Err(ParseError::Unresolved),
      };
      let below = layered(base)?;

      layers.push(below.layer);
      pending = below.base;
    }

    match pending {
      None => Ok(CMap {
        metadata: top.metadata,
        layers,
      }),
      Some(_deeper) => Err(ParseError::TooDeep),
    }
  }

  pub fn identity_h() -> Result<CMap, Never> {
    CMap::identity(WritingMode::Horizontal)
  }

  pub fn identity_v() -> Result<CMap, Never> {
    CMap::identity(WritingMode::Vertical)
  }

  fn identity(writing_mode: WritingMode) -> Result<CMap, Never> {
    let mut layer = Layer::default();
    let Ok(()) = layer.add_cid(
      2,
      Span {
        start: 0,
        end: 0xFFFF,
        value: 0,
      },
    );

    Ok(CMap {
      metadata: Metadata {
        character_collection: Some(CharacterCollection {
          family: CidFamily::AdobeIdentity,
          supplement: 0,
        }),
        writing_mode: Some(writing_mode),
      },
      layers: vec![layer],
    })
  }

  pub fn lookup_cid_code(&self, code: u32, byte_len: u8) -> Result<Option<Cid>, Never> {
    match byte_len {
      1..=4 => {}
      _other => return Ok(None),
    }

    for layer in &self.layers {
      let Ok(found) = layer.cid_code(code, byte_len);

      match found {
        Lookup::Settled(cid) => return Ok(cid),
        Lookup::Unsettled => {}
      }
    }

    Ok(None)
  }

  pub fn lookup_bf_string(&self, code: u32) -> Result<Option<BfString>, Never> {
    for layer in &self.layers {
      let Ok(found) = layer.bf_string(code);

      match found {
        Lookup::Settled(string) => return Ok(string),
        Lookup::Unsettled => {}
      }
    }

    Ok(None)
  }
}

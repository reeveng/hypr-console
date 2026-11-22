//! What a face says about itself: its families and PostScript name, its style, weight and stretch.
//!
//! The family is the typographic family where the font names one and the
//! family name where it does not, which is what keeps "Arial" rather than
//! "Arial Bold". A name record is read when it is Unicode, and a Macintosh
//! Roman record is read too when no Unicode record is American English, which
//! is the oldest way a font says its name; American English is put first when
//! there is one. The PostScript name is the first record of its kind in an
//! encoding this reads, and a face whose first such record cannot be read is
//! refused, as a face with no family is.
//!
//! The style comes from `OS/2` and is italic when the face is upright there but
//! its `post` table gives it a slant. Monospaced is a non-zero fixed-pitch word
//! in `post`, and a `post` table too short to hold the word or the slant reads
//! as having them, which is how the upstream reader answered and so how faces
//! were found before it was read here.

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::ttf_parser::{self, Language, PlatformId, name_id};
use crate::{FaceInfo, ID, Pitch, Source, Stretch, Style, Weight};
use console_core_never::Never;

#[derive(Debug)]
pub(crate) enum Refusal {
  MalformedFont,

  UnnamedFont,
}

impl core::fmt::Display for Refusal {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Refusal::MalformedFont => write!(f, "malformed font"),
      Refusal::UnnamedFont => write!(f, "font doesn't have a family name"),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NameEncoding {
  Unicode,
  MacintoshRoman,
  Unsupported,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slant {
  Upright,
  Slanted,
}

struct Naming {
  families: Vec<(String, Language)>,
  post_script_name: String,
}

#[derive(Clone, Copy, Debug)]
struct Post {
  pitch: Pitch,
  slant: Slant,
}

pub(crate) fn read(
  source: &Source,
  data: &[u8],
  origin: &dyn core::fmt::Display,
) -> Result<Vec<FaceInfo>, Never> {
  let Ok(collection) = ttf_parser::fonts_in_collection(data);
  let many = match collection {
    Some(many) => many,
    None => 1,
  };
  let mut faces = Vec::new();

  for index in 0..many {
    match parse_face_info(source.clone(), data, index) {
      Ok(face) => faces.push(face),
      Err(refusal) => {
        console_core_logging::warn!(
          "Failed to load a font face {} from {} cause {}.",
          index,
          origin,
          refusal
        )
      }
    }
  }

  Ok(faces)
}

fn parse_face_info(source: Source, data: &[u8], index: u32) -> Result<FaceInfo, Refusal> {
  let raw_face =
    ttf_parser::RawFace::parse(data, index).map_err(|_malformed| Refusal::MalformedFont)?;
  let Ok(named) = parse_names(&raw_face);
  let naming = named.ok_or(Refusal::UnnamedFont)?;
  let Ok((upright_style, weight, stretch)) = parse_os2(&raw_face);
  let Ok(post) = parse_post(&raw_face);
  let style = match (upright_style, post.slant) {
    (Style::Normal, Slant::Slanted) => Style::Italic,
    (Style::Normal, Slant::Upright)
    | (Style::Italic | Style::Oblique, Slant::Upright | Slant::Slanted) => upright_style,
  };
  let Ok(id) = ID::dummy();

  Ok(FaceInfo {
    id,
    source,
    index,
    families: naming.families,
    post_script_name: naming.post_script_name,
    style,
    weight,
    stretch,
    pitch: post.pitch,
  })
}

fn parse_names(raw_face: &ttf_parser::RawFace) -> Result<Option<Naming>, Never> {
  let Ok(name_tag) = ttf_parser::Tag::from_bytes(b"name");
  let Ok(table) = raw_face.table(name_tag);
  let name_data = match table {
    Some(name_data) => name_data,
    None => return Ok(None),
  };
  let Ok(parsed) = ttf_parser::name::Table::parse(name_data);
  let names = match parsed {
    Some(name_table) => name_table.names,
    None => return Ok(None),
  };
  let Ok(mut families) = families_of(&names);

  let through_english = match families
    .iter()
    .position(|(_, language)| *language == Language::English_UnitedStates)
  {
    Some(at) => families.get_mut(..=at),
    None => None,
  };

  match through_english {
    Some([first, .., preferred]) => core::mem::swap(first, preferred),
    Some([] | [_]) | None => {}
  }

  match families.as_slice() {
    [] => return Ok(None),
    [_, ..] => {}
  }

  let post_script = names.into_iter().find(|name| {
    let Ok(encoding) = encoding_of(name);

    name.name_id == name_id::POST_SCRIPT_NAME && encoding != NameEncoding::Unsupported
  });
  let post_script_name = match post_script {
    Some(name) => name_to_unicode(&name)?,
    None => None,
  };

  Ok(post_script_name.map(|post_script_name_2| Naming {
    families,
    post_script_name: post_script_name_2,
  }))
}

fn families_of(names: &ttf_parser::name::Names) -> Result<Vec<(String, Language)>, Never> {
  let Ok(typographic) = collect_families(name_id::TYPOGRAPHIC_FAMILY, names);

  match typographic.as_slice() {
    [] => collect_families(name_id::FAMILY, names),
    [_, ..] => Ok(typographic),
  }
}

fn collect_families(
  name_id: u16,
  names: &ttf_parser::name::Names,
) -> Result<Vec<(String, Language)>, Never> {
  let mut families = Vec::new();

  for name in names.into_iter().filter(|name| name.name_id == name_id) {
    let Ok(encoding) = encoding_of(&name);

    match encoding {
      NameEncoding::Unicode => {
        let Ok(family) = family_of(&name);

        families.extend(family);
      }
      NameEncoding::MacintoshRoman | NameEncoding::Unsupported => {}
    }
  }

  let english = families
    .iter()
    .find(|(_, language)| *language == Language::English_UnitedStates);

  match english {
    Some(_american_english_family) => {}
    None => {
      let roman = names
        .into_iter()
        .filter(|name| name.name_id == name_id)
        .find_map(|name| {
          let Ok(encoding) = encoding_of(&name);

          match encoding {
            NameEncoding::MacintoshRoman => {
              let Ok(family) = family_of(&name);

              family
            }
            NameEncoding::Unicode | NameEncoding::Unsupported => None,
          }
        });

      families.extend(roman);
    }
  }

  Ok(families)
}

fn family_of(name: &ttf_parser::name::Name) -> Result<Option<(String, Language)>, Never> {
  let Ok(decoded) = name_to_unicode(name);
  let Ok(language) = name.language();

  Ok(decoded.map(|family| (family, language)))
}

fn encoding_of(name: &ttf_parser::name::Name) -> Result<NameEncoding, Never> {
  let Ok(unicode) = name.unicode_encoding();

  Ok(match unicode {
    ttf_parser::name::UnicodeEncoding::Unicode => NameEncoding::Unicode,
    ttf_parser::name::UnicodeEncoding::Other => match (name.platform_id, name.encoding_id) {
      (PlatformId::Macintosh, 0) => NameEncoding::MacintoshRoman,
      (PlatformId::Macintosh, 1..)
      | (PlatformId::Unicode | PlatformId::Iso | PlatformId::Windows | PlatformId::Custom, _) => {
        NameEncoding::Unsupported
      }
    },
  })
}

pub(crate) fn name_to_unicode(name: &ttf_parser::name::Name) -> Result<Option<String>, Never> {
  let Ok(encoding) = encoding_of(name);
  let units: Vec<u16> = match encoding {
    NameEncoding::Unicode => {
      let Ok(units) = ttf_parser::LazyArray16::<u16>::new(name.name);

      units.into_iter().collect()
    }
    NameEncoding::MacintoshRoman => name
      .name
      .iter()
      .map(|byte| {
        let Ok(unit) = console_core_macintosh_roman::decode(*byte);

        unit
      })
      .collect(),
    NameEncoding::Unsupported => return Ok(None),
  };

  Ok(match String::from_utf16(&units) {
    Ok(decoded) => Some(decoded),
    Err(fault) => {
      console_core_logging::debug!("Skipping an invalid font name: {}", fault);

      None
    }
  })
}

fn parse_os2(raw_face: &ttf_parser::RawFace) -> Result<(Style, Weight, Stretch), Never> {
  let Ok(os2_tag) = ttf_parser::Tag::from_bytes(b"OS/2");
  let Ok(table) = raw_face.table(os2_tag);
  let parsed = match table {
    Some(os2_data) => ttf_parser::os2::Table::parse(os2_data)?,
    None => None,
  };
  let os2 = match parsed {
    Some(os2) => os2,
    None => return Ok((Style::Normal, Weight::NORMAL, Stretch::Normal)),
  };
  let Ok(slope) = os2.style();
  let style = match slope {
    ttf_parser::Style::Normal => Style::Normal,
    ttf_parser::Style::Italic => Style::Italic,
    ttf_parser::Style::Oblique => Style::Oblique,
  };
  let Ok(weight_class) = os2.weight();
  let Ok(weight) = weight_class.to_number();
  let Ok(stretch) = os2.width();

  Ok((style, Weight(weight), stretch))
}

fn parse_post(raw_face: &ttf_parser::RawFace) -> Result<Post, Never> {
  let Ok(post_tag) = ttf_parser::Tag::from_bytes(b"post");
  let Ok(table) = raw_face.table(post_tag);
  let data = match table {
    Some(data) => data,
    None => {
      return Ok(Post {
        pitch: Pitch::Proportional,
        slant: Slant::Upright,
      });
    }
  };
  let pitch = match data.get(12..16) {
    Some([0, 0, 0, 0]) => Pitch::Proportional,
    Some(_) | None => Pitch::Fixed,
  };
  let slant = match data.get(4..8) {
    Some([0, 0, 0, 0]) => Slant::Upright,
    Some(_) | None => Slant::Slanted,
  };

  Ok(Post { pitch, slant })
}

#[cfg(test)]
mod macintosh_roman_tests {
  use super::{name_to_unicode, ttf_parser};

  #[test]
  fn macintosh_font_names_use_the_shared_encoding() {
    let name = ttf_parser::name::Name {
      platform_id: ttf_parser::PlatformId::Macintosh,
      encoding_id: 0,
      language_id: 0,
      name_id: ttf_parser::name_id::FAMILY,
      name: b"Caf\x8e \x11",
    };

    let Ok(unicode) = name_to_unicode(&name);

    assert_eq!(unicode.as_deref(), Some("Café ⌘"));
  }
}

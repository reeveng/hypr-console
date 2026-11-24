//! What a face says about itself: its families and PostScript name, its style, weight and stretch.
//!
//! An invalid record ends the scan, preserving the database's original reader behavior.
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

use console_core_font_reading::tables::{
  name::NameId, os2::SelectionFlags, post::Post as PostTable,
};
use console_core_font_reading::{FileRef, FontRef, Presence, TableProvider, TopLevelTable};
use console_core_number_conversion::fixed_index;

use crate::{FaceInfo, ID, Language, Pitch, Source, Stretch, Style, Weight};
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
  let mut faces = Vec::new();
  let font = match FileRef::new(data) {
    Ok(font) => font,
    Err(_malformed) => {
      let Ok(()) = warn_refusal(0, origin, &Refusal::MalformedFont);

      return Ok(faces);
    }
  };
  let Ok(fonts) = font.fonts();

  for (index, font_2) in fonts.enumerate() {
    let Ok(position) = fixed_index(index);
    let Ok(index_2) = console_core_number_conversion::fitted::<_, u32>(position);
    let parsed = font_2
      .map_err(|_malformed| Refusal::MalformedFont)
      .and_then(|face| parse_face_info(source.clone(), &face, index_2));

    match parsed {
      Ok(face) => faces.push(face),
      Err(refusal) => {
        let Ok(()) = warn_refusal(position, origin, &refusal);
      }
    }
  }

  Ok(faces)
}

fn warn_refusal(
  index: u64,
  origin: &dyn core::fmt::Display,
  refusal: &Refusal,
) -> Result<(), Never> {
  console_core_logging::warn!(
    "Failed to load a font face {} from {} cause {}.",
    index,
    origin,
    refusal
  );

  Ok(())
}

fn parse_face_info(
  source: Source,
  raw_face: &FontRef<'_>,
  index: u32,
) -> Result<FaceInfo, Refusal> {
  let Ok(named) = parse_names(raw_face);
  let naming = named.ok_or(Refusal::UnnamedFont)?;
  let Ok((upright_style, weight, stretch)) = parse_os2(raw_face);
  let Ok(post) = parse_post(raw_face);
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

#[derive(Clone, Copy)]
struct FaceName<'a> {
  identifier: NameId,
  encoding: NameEncoding,
  language: Language,
  bytes: &'a [u8],
}

fn parse_names(raw_face: &FontRef<'_>) -> Result<Option<Naming>, Never> {
  let table = match raw_face.name() {
    Ok(table) => table,
    Err(_missing_or_malformed) => return Ok(None),
  };
  let Ok(version) = table.version();
  let Ok(storage_range) = match version {
    0 => table.name_record_byte_range(),
    1 => table.lang_tag_record_byte_range(),
    _unknown => return Ok(None),
  };
  let Ok(records) = table.name_record();
  let Ok(data) = table.offset_data();
  let Ok(storage_offset) = table.storage_offset();
  let Ok(storage) = data.split_off(u64::from(storage_offset).max(storage_range.end));
  let storage_3 = match storage {
    Some(storage_2) => storage_2,
    None => return Ok(None),
  };
  let mut names = records.iter().map_while(|record| {
    let Ok(platform) = record.platform_id();
    let Ok(encoding_id) = record.encoding_id();
    let Ok(language_id) = record.language_id();
    let Ok(identifier) = record.name_id();
    let encoding = match (platform, encoding_id) {
      (0, _) | (3, 0 | 1) => NameEncoding::Unicode,
      (1, 0) => NameEncoding::MacintoshRoman,
      (1..=4, _) => NameEncoding::Unsupported,
      _unknown => return None,
    };
    let Ok(language) = match (platform, encoding_id, language_id) {
      (3, _, language) => Language::windows_language(language),
      (1, 0, 0) => Ok(Language::English_UnitedStates),
      _other => Ok(Language::Unknown),
    };
    let name = match record.string(storage_3) {
      Ok(name) => name,
      Err(_outside_storage) => return None,
    };
    let Ok(bytes) = name.as_bytes();

    Some(FaceName {
      identifier,
      encoding,
      language,
      bytes,
    })
  });
  let Ok(mut families) = families_of(names.clone());

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

  let post_script = names.find(|name| {
    name.identifier == NameId::POSTSCRIPT_NAME && name.encoding != NameEncoding::Unsupported
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

fn families_of<'a>(
  names: impl Iterator<Item = FaceName<'a>> + Clone,
) -> Result<Vec<(String, Language)>, Never> {
  let Ok(typographic) = collect_families(NameId::TYPOGRAPHIC_FAMILY_NAME, names.clone());

  match typographic.as_slice() {
    [] => collect_families(NameId::FAMILY_NAME, names),
    [_, ..] => Ok(typographic),
  }
}

fn collect_families<'a>(
  identifier: NameId,
  names: impl Iterator<Item = FaceName<'a>> + Clone,
) -> Result<Vec<(String, Language)>, Never> {
  let matching = names.filter(|name| name.identifier == identifier);
  let mut families = Vec::new();

  for name in matching
    .clone()
    .filter(|name| name.encoding == NameEncoding::Unicode)
  {
    let Ok(decoded) = name_to_unicode(&name);

    families.extend(decoded.map(|family| (family, name.language)));
  }

  match families
    .iter()
    .find(|(_, language)| *language == Language::English_UnitedStates)
  {
    Some(_american_english_family) => {}
    None => {
      families.extend(
        matching
          .filter(|name| name.encoding == NameEncoding::MacintoshRoman)
          .find_map(|name| {
            let Ok(decoded) = name_to_unicode(&name);

            decoded.map(|family| (family, name.language))
          }),
      );
    }
  }

  Ok(families)
}

fn name_to_unicode(name: &FaceName<'_>) -> Result<Option<String>, Never> {
  let units: Vec<u16> = match name.encoding {
    NameEncoding::Unicode => name
      .bytes
      .as_chunks::<2>()
      .0
      .iter()
      .copied()
      .map(u16::from_be_bytes)
      .collect(),
    NameEncoding::MacintoshRoman => name
      .bytes
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

fn parse_os2(raw_face: &FontRef<'_>) -> Result<(Style, Weight, Stretch), Never> {
  let defaults = (Style::Normal, Weight::NORMAL, Stretch::Normal);
  let table = match raw_face.os2() {
    Ok(table) => table,
    Err(_missing_or_malformed) => return Ok(defaults),
  };
  let Ok(version) = table.version();
  let minimum_length = match version {
    0 => 78,
    1 => 86,
    2..=4 => 96,
    5 => 100,
    _unknown => return Ok(defaults),
  };
  let Ok(data) = table.offset_data();
  let Ok(length) = data.len();

  match length.cmp(&minimum_length) {
    core::cmp::Ordering::Less => return Ok(defaults),
    core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {}
  }

  let Ok(selection) = table.fs_selection();
  let Ok(italic) = selection.contains(SelectionFlags::ITALIC);
  let Ok(oblique) = selection.contains(SelectionFlags::OBLIQUE);
  let style = match (italic, oblique, version) {
    (Presence::Present, _, _) => Style::Italic,
    (Presence::Absent, Presence::Present, 4..) => Style::Oblique,
    _upright => Style::Normal,
  };
  let Ok(weight) = table.us_weight_class();
  let Ok(width) = table.us_width_class();
  let Ok(stretch) = Stretch::from_width_class(width);

  Ok((style, Weight(weight), stretch))
}

fn parse_post(raw_face: &FontRef<'_>) -> Result<Post, Never> {
  let default = Post {
    pitch: Pitch::Proportional,
    slant: Slant::Upright,
  };
  let data = match raw_face.table_data(PostTable::TAG) {
    Ok(Some(data)) => data,
    Ok(None) => return Ok(default),
    Err(_unreadable_post_table) => return Ok(default),
  };
  let pitch = match data.read_at::<u32>(12) {
    Ok(0) => Pitch::Proportional,
    Ok(_) => Pitch::Fixed,
    Err(_missing_pitch_flag) => Pitch::Fixed,
  };
  let slant = match data.read_at::<i32>(4) {
    Ok(0) => Slant::Upright,
    Ok(_) => Slant::Slanted,
    Err(_missing_slant_angle) => Slant::Slanted,
  };

  Ok(Post { pitch, slant })
}

#[cfg(test)]
mod tests {
  use crate::Database;

  use super::{FaceName, Language, NameEncoding, NameId, name_to_unicode};

  #[test]
  fn macintosh_font_names_use_the_shared_encoding() {
    let name = FaceName {
      identifier: NameId::FAMILY_NAME,
      encoding: NameEncoding::MacintoshRoman,
      language: Language::English_UnitedStates,
      bytes: b"Caf\x8e \x11",
    };
    let Ok(unicode) = name_to_unicode(&name);

    assert_eq!(unicode.as_deref(), Some("Café ⌘"));
  }

  #[test]
  fn reads_format_one_name_records_before_the_language_tag_count() {
    let mut font = b"\0\x01\0\0\0\x01\0\0\0\0\0\0name\0\0\0\0\0\0\0\x1c\0\0\0\x24".to_vec();
    font.extend_from_slice(b"\0\x01\0\x02\0\x20");
    font.extend_from_slice(b"\0\x03\0\x01\x04\x09\0\x01\0\x02\0\0");
    font.extend_from_slice(b"\0\x03\0\x01\x04\x09\0\x06\0\x02\0\x02");
    font.extend_from_slice(b"\0\0\0A\0B");
    let Ok(mut database) = Database::new();
    let Ok(()) = database.load_font_data(font);
    let Ok(mut faces) = database.faces();

    assert_eq!(
      faces.next().map(|face| face.post_script_name.as_str()),
      Some("B")
    );
  }
}

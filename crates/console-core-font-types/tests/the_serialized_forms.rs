#![cfg(feature = "serde")]

use console_core_font_types::{
  F2Dot14, F4Dot12, F6Dot10, F26Dot6, Fixed, GlyphId16, LongDateTime, MajorMinor, NameId,
  Offset16, Tag, Uint24, Version16Dot16,
};

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Everything {
  fixed: Fixed,
  hinted: F26Dot6,
  normalized: F2Dot14,
  glyph: GlyphId16,
  date: LongDateTime,
  name: NameId,
  offset: Offset16,
  tag: Tag,
  wide: Uint24,
  major_minor: MajorMinor,
  version: Version16Dot16,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Tagged {
  tag: Tag,
}

#[test]
fn every_scalar_reads_back_what_it_wrote() -> Result<(), serde_json::Error> {
  let Ok(fixed) = Fixed::from_f64(521.5);
  let Ok(hinted) = F26Dot6::from_f64(-1001.1);
  let Ok(normalized) = F2Dot14::from_f32(1.2);
  let Ok(glyph) = GlyphId16::new(69);
  let Ok(date) = LongDateTime::new(1_234_569_101);
  let Ok(name) = NameId::new(8214);
  let Ok(offset) = Offset16::new(42);
  let Ok(tag) = Tag::new(b"cool");
  let Ok(wide) = Uint24::new(16_777_215);
  let major_minor = MajorMinor {
    major: 10,
    minor: 5,
  };
  let version = Version16Dot16::VERSION_2_5;
  let written = Everything {
    fixed,
    hinted,
    normalized,
    glyph,
    date,
    name,
    offset,
    tag,
    wide,
    major_minor,
    version,
  };

  let text = serde_json::to_string(&written)?;
  let read: Everything = serde_json::from_str(&text)?;

  assert_eq!(read, written);

  Ok(())
}

#[test]
fn a_tag_is_written_as_its_four_letters() -> Result<(), serde_json::Error> {
  let Ok(tag) = Tag::new(b"yolo");
  let Ok(unprintable) = Tag::new(&[3, 244, 0, 221]);

  let text = serde_json::to_string(&Tagged { tag })?;
  let read: Tagged = serde_json::from_str(&text)?;
  let streamed: Tagged = serde_json::from_reader(text.as_bytes())?;

  assert_eq!(text, r#"{"tag":"yolo"}"#);
  assert_eq!(read, Tagged { tag });
  assert_eq!(streamed, Tagged { tag });
  assert_eq!(
    serde_json::to_string(&Tagged { tag: unprintable }).map_err(|fault| fault.to_string()),
    Err(String::from(
      "invalid utf-8 sequence of 1 bytes from index 1"
    ))
  );
  assert_eq!(
    serde_json::from_str::<Tagged>(r#"{"tag":"a b"}"#).map_err(|fault| fault.to_string()),
    Err(String::from(
      "Invalid byte 0x62 at index 2 at line 1 column 12"
    ))
  );

  Ok(())
}

#[test]
fn one_is_written_as_one() -> Result<(), serde_json::Error> {
  let normalized_text = serde_json::to_string(&F2Dot14::ONE)?;
  let colour_text = serde_json::to_string(&F4Dot12::ONE)?;
  let coarse_text = serde_json::to_string(&F6Dot10::ONE)?;
  let fixed_text = serde_json::to_string(&Fixed::ONE)?;
  let hinted_text = serde_json::to_string(&F26Dot6::ONE)?;
  let normalized: F2Dot14 = serde_json::from_str("1.0")?;
  let colour: F4Dot12 = serde_json::from_str("1.0")?;
  let coarse: F6Dot10 = serde_json::from_str("1.0")?;
  let fixed: Fixed = serde_json::from_str("1.0")?;
  let hinted: F26Dot6 = serde_json::from_str("1.0")?;

  assert_eq!(
    [
      normalized_text,
      colour_text,
      coarse_text,
      fixed_text,
      hinted_text
    ],
    ["1.0"; 5]
  );
  assert_eq!(normalized, F2Dot14::ONE);
  assert_eq!(colour, F4Dot12::ONE);
  assert_eq!(coarse, F6Dot10::ONE);
  assert_eq!(fixed, Fixed::ONE);
  assert_eq!(hinted, F26Dot6::ONE);

  Ok(())
}

#[test]
fn every_sixteen_bit_fixed_point_number_is_written_as_its_float() -> Result<(), serde_json::Error>
{
  for raw in i16::MIN..=i16::MAX {
    let Ok(normalized) = F2Dot14::from_bits(raw);
    let Ok(colour) = F4Dot12::from_bits(raw);
    let Ok(coarse) = F6Dot10::from_bits(raw);

    let normalized_text = serde_json::to_value(normalized)?;
    let colour_text = serde_json::to_value(colour)?;
    let coarse_text = serde_json::to_value(coarse)?;

    let Ok(normalized_value) = normalized.to_f64();
    let Ok(colour_value) = colour.to_f64();
    let Ok(coarse_value) = coarse.to_f64();

    assert_eq!(normalized_text.as_f64(), Some(normalized_value));
    assert_eq!(colour_text.as_f64(), Some(colour_value));
    assert_eq!(coarse_text.as_f64(), Some(coarse_value));
  }

  Ok(())
}

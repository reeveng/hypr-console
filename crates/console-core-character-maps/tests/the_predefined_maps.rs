use console_core_character_maps::{
  BfString, CMap, CMapName, CharacterCollection, CidFamily, WritingMode, load_embedded,
};
use console_core_never::Never;

type Failure = Box<dyn std::error::Error>;

#[derive(Debug)]
enum Fault {
  Missing(&'static str),
  Unreadable(&'static str),
}

impl std::fmt::Display for Fault {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Fault::Missing(name) => write!(to, "{name} is not in the bundle"),
      Fault::Unreadable(name) => write!(to, "{name} does not parse"),
    }
  }
}

impl std::error::Error for Fault {}

fn predefined(name: &'static str) -> Result<CMap, Failure> {
  let Ok(wanted) = CMapName::from_bytes(name.as_bytes());
  let Ok(found) = load_embedded(wanted);
  let data = found.ok_or(Fault::Missing(name))?;

  CMap::parse(data, |base| match load_embedded(base) {
    Ok(embedded) => embedded,
  })
  .map_err(|_unreadable| Failure::from(Fault::Unreadable(name)))
}

fn collection(family: CidFamily, supplement: i32) -> Result<Option<CharacterCollection>, Never> {
  Ok(Some(CharacterCollection { family, supplement }))
}

fn cids(cmap: &CMap, codes: &[(u32, u8, u32)]) -> Result<(), Never> {
  for (code, width, expected) in codes {
    assert_eq!(
      cmap.lookup_cid_code(*code, *width),
      Ok(Some(*expected)),
      "code {code:#x} of {width} bytes"
    );
  }

  Ok(())
}

fn characters(cmap: &CMap, codes: &[(u32, char)]) -> Result<(), Never> {
  for (code, expected) in codes {
    assert_eq!(
      cmap.lookup_bf_string(*code),
      Ok(Some(BfString::Char(*expected))),
      "code {code:#x}"
    );
  }

  Ok(())
}

fn text(cmap: &CMap, code: u32, expected: &str) -> Result<(), Never> {
  assert_eq!(
    cmap.lookup_bf_string(code),
    Ok(Some(BfString::String(String::from(expected)))),
    "code {code:#x}"
  );

  Ok(())
}

#[test]
fn h_is_the_jis_row_and_cell_encoding() -> Result<(), Failure> {
  let cmap = predefined("H")?;

  assert_eq!(cmap.metadata.writing_mode, None);
  assert_eq!(
    Ok(cmap.metadata.character_collection.clone()),
    collection(CidFamily::AdobeJapan1, 1)
  );
  let Ok(()) = cids(&cmap, &[(0x2121, 2, 633), (0x2221, 2, 727)]);

  Ok(())
}

#[test]
fn a_shift_jis_map_has_one_byte_and_two_byte_codes() -> Result<(), Failure> {
  let cmap = predefined("90ms-RKSJ-H")?;

  assert_eq!(
    Ok(cmap.metadata.character_collection.clone()),
    collection(CidFamily::AdobeJapan1, 2)
  );
  let Ok(()) = cids(&cmap, &[(0x20, 1, 231), (0x8140, 2, 633)]);

  Ok(())
}

#[test]
fn v_is_h_turned_vertical_and_answers_for_what_it_leaves_to_it() -> Result<(), Failure> {
  let cmap = predefined("V")?;

  assert_eq!(cmap.metadata.writing_mode, Some(WritingMode::Vertical));
  assert_eq!(
    Ok(cmap.metadata.character_collection.clone()),
    collection(CidFamily::AdobeJapan1, 1)
  );
  let Ok(()) = cids(&cmap, &[(0x2121, 2, 633)]);

  Ok(())
}

#[test]
fn the_identity_maps_are_in_the_bundle_too() -> Result<(), Failure> {
  for (name, mode) in [
    ("Identity-H", None),
    ("Identity-V", Some(WritingMode::Vertical)),
  ] {
    let cmap = predefined(name)?;

    assert_eq!(cmap.metadata.writing_mode, mode);
    assert_eq!(
      Ok(cmap.metadata.character_collection.clone()),
      collection(CidFamily::AdobeIdentity, 0)
    );
    let Ok(()) = cids(
      &cmap,
      &[(0x0000, 2, 0), (0x0041, 2, 0x41), (0xFFFF, 2, 0xFFFF)],
    );
  }

  Ok(())
}

#[test]
fn a_gbk_map_counts_up_through_its_ranges() -> Result<(), Failure> {
  let cmap = predefined("GBK-EUC-H")?;

  assert_eq!(
    Ok(cmap.metadata.character_collection.clone()),
    collection(CidFamily::AdobeGB1, 2)
  );
  let Ok(()) = cids(
    &cmap,
    &[
      (0x20, 1, 7716),
      (0x21, 1, 814),
      (0x7E, 1, 907),
      (0xFE80, 2, 22094),
      (0xFEA0, 2, 22126),
    ],
  );

  Ok(())
}

#[test]
fn a_korean_map_and_two_big_five_maps_read_as_they_were_made() -> Result<(), Failure> {
  let korean = predefined("KSC-EUC-H")?;
  let big_five = predefined("B5pc-H")?;
  let eten = predefined("ETen-B5-H")?;

  assert_eq!(
    Ok(korean.metadata.character_collection.clone()),
    collection(CidFamily::AdobeKorea1, 0)
  );
  let Ok(()) = cids(
    &korean,
    &[
      (0x20, 1, 8094),
      (0x41, 1, 8127),
      (0xA1A1, 2, 101),
      (0xA1FE, 2, 194),
      (0xFDA1, 2, 7962),
    ],
  );
  assert_eq!(
    Ok(big_five.metadata.character_collection.clone()),
    collection(CidFamily::AdobeCNS1, 0)
  );
  let Ok(()) = cids(
    &big_five,
    &[
      (0x20, 1, 1),
      (0x41, 1, 34),
      (0x80, 1, 61),
      (0xD140, 2, 7251),
      (0xD17E, 2, 7313),
      (0xF9D5, 2, 13645),
    ],
  );
  assert_eq!(
    Ok(eten.metadata.character_collection.clone()),
    collection(CidFamily::AdobeCNS1, 0)
  );
  let Ok(()) = cids(
    &eten,
    &[
      (0x20, 1, 13648),
      (0x7E, 1, 13742),
      (0xA140, 2, 99),
      (0xA158, 2, 123),
      (0xD040, 2, 7094),
      (0xF9FE, 2, 14096),
    ],
  );

  Ok(())
}

#[test]
fn a_utf16_map_has_four_byte_codes_for_the_surrogates() -> Result<(), Failure> {
  let cmap = predefined("UniJIS-UTF16-H")?;

  assert_eq!(
    Ok(cmap.metadata.character_collection.clone()),
    collection(CidFamily::AdobeJapan1, 7)
  );
  let Ok(()) = cids(
    &cmap,
    &[
      (0x20, 2, 1),
      (0x41, 2, 34),
      (0x5C, 2, 97),
      (0x5BCC, 2, 3531),
      (0xD884_DF50, 4, 19130),
    ],
  );

  Ok(())
}

#[test]
fn the_gb1_ucs2_map_goes_from_a_cid_to_text() -> Result<(), Failure> {
  let gb1 = predefined("Adobe-GB1-UCS2")?;

  assert_eq!(
    Ok(gb1.metadata.character_collection.clone()),
    collection(
      CidFamily::Custom {
        registry: b"Adobe".to_vec(),
        ordering: b"Adobe_GB1_UCS2".to_vec()
      },
      5
    )
  );
  let Ok(()) = characters(
    &gb1,
    &[
      (0x0063, '\u{00B7}'),
      (0x0001, ' '),
      (0x0022, 'A'),
      (0x005F, '~'),
    ],
  );
  let Ok(()) = text(&gb1, 0x560F, "\u{90CE}\u{FE00}");

  Ok(())
}

#[test]
fn the_japan1_ucs2_map_goes_from_a_cid_to_text() -> Result<(), Failure> {
  let japan1 = predefined("Adobe-Japan1-UCS2")?;

  assert_eq!(
    Ok(japan1.metadata.character_collection.clone()),
    collection(
      CidFamily::Custom {
        registry: b"Adobe".to_vec(),
        ordering: b"Adobe_Japan1_UCS2".to_vec()
      },
      7
    )
  );
  let Ok(()) = characters(
    &japan1,
    &[(0x003D, '\u{00A5}'), (0x0001, ' '), (0x003C, '[')],
  );
  let Ok(()) = text(&japan1, 0x0476, "\u{82A6}\u{E0100}");
  let Ok(()) = text(&japan1, 0x265B, "10/11");
  let Ok(()) = text(&japan1, 0x2E6B, "オングストローム");

  Ok(())
}

#[test]
fn the_korea1_ucs2_map_goes_from_a_cid_to_text() -> Result<(), Failure> {
  let korea1 = predefined("Adobe-Korea1-UCS2")?;

  assert_eq!(
    Ok(korea1.metadata.character_collection.clone()),
    collection(
      CidFamily::Custom {
        registry: b"Adobe".to_vec(),
        ordering: b"Adobe_Korea1_UCS2".to_vec()
      },
      2
    )
  );
  let Ok(()) = characters(&korea1, &[(0x0060, '\u{20A9}')]);
  let Ok(()) = text(&korea1, 0x2072, "[10]");
  let Ok(()) = text(&korea1, 0x2073, "[11]");

  Ok(())
}

#[test]
fn the_cns1_ucs2_map_goes_from_a_cid_to_text() -> Result<(), Failure> {
  let cns1 = predefined("Adobe-CNS1-UCS2")?;

  assert_eq!(
    Ok(cns1.metadata.character_collection.clone()),
    collection(
      CidFamily::Custom {
        registry: b"Adobe".to_vec(),
        ordering: b"Adobe_CNS1_UCS2".to_vec()
      },
      7
    )
  );
  let Ok(()) = characters(
    &cns1,
    &[
      (0x0060, '\u{00A9}'),
      (0x0001, ' '),
      (0x005F, '~'),
      (0x36B0, '\u{200CC}'),
    ],
  );

  Ok(())
}

#[test]
fn a_family_points_at_the_ucs2_map_the_bundle_holds() -> Result<(), Failure> {
  for family in [
    CidFamily::AdobeJapan1,
    CidFamily::AdobeGB1,
    CidFamily::AdobeCNS1,
    CidFamily::AdobeKorea1,
  ] {
    let name = match family.ucs2_cmap() {
      Ok(Some(name)) => name,
      Ok(None) => return Err(Failure::from(Fault::Missing("a ucs2 map"))),
    };

    let Ok(found) = load_embedded(name);

    assert!(found.is_some(), "{family:?}");
  }

  assert_eq!(CidFamily::AdobeIdentity.ucs2_cmap(), Ok(None));

  Ok(())
}

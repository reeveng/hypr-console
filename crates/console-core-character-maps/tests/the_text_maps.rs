use console_core_character_maps::{
  BfString, CMap, CMapName, CharacterCollection, CidFamily, ParseError, WritingMode,
};
use console_core_never::Never;

type Failure = Box<dyn std::error::Error>;

const PREAMBLE: &[u8] = br#"/CIDSystemInfo 3 dict dup begin
  /Registry (Adobe) def
  /Ordering (Japan1) def
  /Supplement 0 def
end def
/CMapName /Test def
/WMode 0 def
2 begincodespacerange
<00> <FF>
<0000> <FFFF>
endcodespacerange
"#;

const BASE: &[u8] = br#"
/CIDSystemInfo 3 dict dup begin
  /Registry (Adobe) def
  /Ordering (Japan1) def
  /Supplement 0 def
end def
/CMapName /Base def
/WMode 0 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
1 begincidrange
<0000> <00FF> 0
endcidrange
"#;

fn without_bases(data: &[u8]) -> Result<CMap, ParseError> {
  CMap::parse(data, |_name| None)
}

fn with_base(data: &[u8]) -> Result<CMap, ParseError> {
  CMap::parse(data, |name| match name.to_bytes() {
    Ok(b"Base") => Some(BASE),
    Ok(_other) => None,
  })
}

fn after_preamble(body: &[u8]) -> Result<CMap, ParseError> {
  let mut data = Vec::new();

  data.extend_from_slice(PREAMBLE);
  data.extend_from_slice(body);

  without_bases(&data)
}

fn cids(cmap: &CMap, codes: &[(u32, u8, Option<u32>)]) -> Result<(), Never> {
  for (code, width, expected) in codes {
    assert_eq!(
      cmap.lookup_cid_code(*code, *width),
      Ok(*expected),
      "code {code:#x} of {width} bytes"
    );
  }

  Ok(())
}

fn characters(cmap: &CMap, codes: &[(u32, Option<char>)]) -> Result<(), Never> {
  for (code, expected) in codes {
    assert_eq!(
      cmap.lookup_bf_string(*code),
      Ok(expected.map(BfString::Char)),
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

fn collection(family: CidFamily, supplement: i32) -> Result<Option<CharacterCollection>, Never> {
  Ok(Some(CharacterCollection { family, supplement }))
}

#[test]
fn the_metadata_of_a_cmap_is_read_off_its_header() -> Result<(), Failure> {
  let cmap = without_bases(
    br#"
/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo 3 dict dup begin
  /Registry (Adobe) def
  /Ordering (Japan1) def
  /Supplement 6 def
end def
/CMapName /Adobe-Japan1-H def
/CMapType 1 def
/WMode 0 def
endcmap"#,
  )?;

  assert_eq!(
    Ok(cmap.metadata.character_collection.clone()),
    collection(CidFamily::AdobeJapan1, 6)
  );
  assert_eq!(cmap.metadata.writing_mode, Some(WritingMode::Horizontal));

  Ok(())
}

#[test]
fn a_cmap_written_vertically_says_so() -> Result<(), Failure> {
  let cmap = without_bases(
    br#"
/CIDSystemInfo 3 dict dup begin
  /Registry (Adobe) def
  /Ordering (Japan1) def
  /Supplement 6 def
end def
/CMapName /Adobe-Japan1-V def
/WMode 1 def
"#,
  )?;

  assert_eq!(cmap.metadata.writing_mode, Some(WritingMode::Vertical));

  Ok(())
}

#[test]
fn a_code_in_a_range_maps_by_its_distance_from_the_start() -> Result<(), Failure> {
  let cmap = after_preamble(
    br#"
3 begincidrange
<0000> <00FF> 0
<0100> <01FF> 256
<8140> <817E> 633
endcidrange
"#,
  )?;

  let Ok(()) = cids(
    &cmap,
    &[
      (0x0000, 2, Some(0)),
      (0x0042, 2, Some(0x42)),
      (0x00FF, 2, Some(0xFF)),
      (0x0100, 2, Some(256)),
      (0x01FF, 2, Some(511)),
      (0x8140, 2, Some(633)),
      (0x817E, 2, Some(695)),
    ],
  );

  Ok(())
}

#[test]
fn a_single_code_maps_to_its_cid() -> Result<(), Failure> {
  let cmap = after_preamble(
    br#"
3 begincidchar
<03> 1
<04> 2
<20> 50
endcidchar
"#,
  )?;

  let Ok(()) = cids(
    &cmap,
    &[(0x03, 1, Some(1)), (0x04, 1, Some(2)), (0x20, 1, Some(50))],
  );

  Ok(())
}

#[test]
fn a_code_outside_every_range_maps_to_nothing() -> Result<(), Failure> {
  let cmap = after_preamble(
    br#"
1 begincidrange
<0100> <01FF> 0
endcidrange
"#,
  )?;

  let Ok(()) = cids(
    &cmap,
    &[(0x00FF, 2, None), (0x0200, 2, None), (0xFFFF, 2, None)],
  );

  Ok(())
}

#[test]
fn ranges_and_characters_in_several_sections_are_all_kept() -> Result<(), Failure> {
  let cmap = after_preamble(
    br#"
2 begincidrange
<0000> <00FF> 0
<0100> <01FF> 256
endcidrange
1 begincidchar
<0200> 600
endcidchar
1 begincidrange
<8140> <817E> 633
endcidrange
"#,
  )?;

  let Ok(()) = cids(
    &cmap,
    &[
      (0x0000, 2, Some(0)),
      (0x0100, 2, Some(256)),
      (0x0200, 2, Some(600)),
      (0x8140, 2, Some(633)),
    ],
  );

  Ok(())
}

#[test]
fn a_one_byte_code_is_looked_up_among_the_one_byte_ranges() -> Result<(), Failure> {
  let cmap = after_preamble(
    br#"
2 begincidrange
<00> <7F> 0
<80> <FF> 200
endcidrange
"#,
  )?;

  let Ok(()) = cids(
    &cmap,
    &[
      (0x00, 1, Some(0)),
      (0x41, 1, Some(0x41)),
      (0x80, 1, Some(200)),
      (0xFF, 1, Some(327)),
    ],
  );

  Ok(())
}

#[test]
fn a_cidsysteminfo_written_as_a_dictionary_is_read() -> Result<(), Failure> {
  let cmap = without_bases(
    br#"
/CIDInit /ProcSet findresource begin
10 dict begin
begincmap
/CIDSystemInfo
<< /Registry (Adobe)
/Ordering (UCS)
/Supplement 0
>> def
/CMapName /Adobe-Identity-UCS def
/CMapType 2 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
2 beginbfchar
<001F> <F049>
<002A> <F055>
endbfchar
endcmap
CMapName currentdict /CMap defineresource pop
end
end
"#,
  )?;

  assert_eq!(
    Ok(cmap.metadata.character_collection.clone()),
    collection(
      CidFamily::Custom {
        registry: b"Adobe".to_vec(),
        ordering: b"UCS".to_vec()
      },
      0
    )
  );
  let Ok(()) = characters(
    &cmap,
    &[(0x001F, Some('\u{F049}')), (0x002A, Some('\u{F055}'))],
  );

  Ok(())
}

#[test]
fn a_cmap_that_names_a_base_answers_for_what_it_leaves_out() -> Result<(), Failure> {
  let cmap = with_base(
    br#"
/Base usecmap
/CIDSystemInfo 3 dict dup begin
  /Registry (Adobe) def
  /Ordering (Japan1) def
  /Supplement 0 def
end def
/CMapName /Child def
/WMode 0 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
1 begincidrange
<0100> <01FF> 256
endcidrange
"#,
  )?;

  let Ok(()) = cids(
    &cmap,
    &[
      (0x0100, 2, Some(256)),
      (0x01FF, 2, Some(511)),
      (0x0000, 2, Some(0)),
      (0x00FF, 2, Some(0xFF)),
      (0x0200, 2, None),
    ],
  );

  Ok(())
}

#[test]
fn a_cmap_answers_before_the_base_it_names_does() -> Result<(), Failure> {
  let cmap = with_base(
    br#"
/Base usecmap
/CIDSystemInfo 3 dict dup begin
  /Registry (Adobe) def
  /Ordering (Japan1) def
  /Supplement 0 def
end def
/CMapName /Child def
/WMode 0 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
1 begincidrange
<0040> <007F> 500
endcidrange
"#,
  )?;

  let Ok(()) = cids(
    &cmap,
    &[
      (0x0000, 2, Some(0)),
      (0x003F, 2, Some(0x3F)),
      (0x0040, 2, Some(500)),
      (0x007F, 2, Some(563)),
      (0x0080, 2, Some(0x80)),
      (0x00FF, 2, Some(0xFF)),
    ],
  );

  Ok(())
}

#[test]
fn a_base_nobody_can_give_is_a_fault() {
  let found = without_bases(b"/Missing usecmap\n");

  assert_eq!(found.map(|_cmap| ()), Err(ParseError::Unresolved));
}

#[test]
fn cmaps_that_name_each_other_end_in_a_fault_rather_than_a_stack() {
  let found = CMap::parse(b"/Itself usecmap\n", |_name| {
    Some(b"/Itself usecmap\n".as_slice())
  });

  assert_eq!(found.map(|_cmap| ()), Err(ParseError::TooDeep));
}

#[test]
fn a_code_that_is_no_string_of_bytes_is_a_fault() {
  let found = after_preamble(b"1 begincidchar\n<> 3\nendcidchar\n");

  assert_eq!(found.map(|_cmap| ()), Err(ParseError::BadCode));
}

#[test]
fn a_notdef_character_answers_only_for_its_own_code() -> Result<(), Failure> {
  let cmap = after_preamble(
    br#"
2 beginnotdefchar
<03> 10
<20> 20
endnotdefchar
"#,
  )?;

  let Ok(()) = cids(
    &cmap,
    &[(0x03, 1, Some(10)), (0x20, 1, Some(20)), (0x04, 1, None)],
  );

  Ok(())
}

#[test]
fn a_notdef_range_gives_every_code_in_it_one_cid() -> Result<(), Failure> {
  let cmap = after_preamble(
    br#"
1 beginnotdefrange
<0000> <001F> 100
endnotdefrange
"#,
  )?;

  let Ok(()) = cids(
    &cmap,
    &[
      (0x0000, 2, Some(100)),
      (0x0001, 2, Some(100)),
      (0x001F, 2, Some(100)),
      (0x0020, 2, None),
    ],
  );

  Ok(())
}

#[test]
fn a_bf_character_maps_to_the_character_it_spells() -> Result<(), Failure> {
  let cmap = after_preamble(
    br#"
2 beginbfchar
<0041> <0048>
<0042> <0065>
endbfchar
"#,
  )?;

  let Ok(()) = characters(
    &cmap,
    &[(0x0041, Some('H')), (0x0042, Some('e')), (0x0043, None)],
  );

  Ok(())
}

#[test]
fn a_bf_character_of_several_units_maps_to_a_string() -> Result<(), Failure> {
  let cmap = after_preamble("1 beginbfchar\n<005F> <00660066>\nendbfchar\n".as_bytes())?;

  let Ok(()) = text(&cmap, 0x005F, "ff");

  Ok(())
}

#[test]
fn a_surrogate_pair_is_one_character() -> Result<(), Failure> {
  let cmap = after_preamble(b"1 beginbfchar\n<3A51> <D840DC3E>\nendbfchar\n")?;

  let Ok(()) = characters(&cmap, &[(0x3A51, Some('\u{2003E}'))]);

  Ok(())
}

#[test]
fn a_bf_range_counts_up_from_its_first_character() -> Result<(), Failure> {
  let cmap = after_preamble(b"1 beginbfrange\n<0000> <0004> <0041>\nendbfrange\n")?;

  let Ok(()) = characters(
    &cmap,
    &[
      (0x0000, Some('A')),
      (0x0001, Some('B')),
      (0x0004, Some('E')),
      (0x0005, None),
    ],
  );

  Ok(())
}

#[test]
fn a_bf_range_written_as_an_array_gives_each_code_its_own_string() -> Result<(), Failure> {
  let cmap = after_preamble(
    b"1 beginbfrange\n<005F> <0061> [<00660066> <00660069> <0066006C>]\nendbfrange\n",
  )?;

  let Ok(()) = text(&cmap, 0x005F, "ff");
  let Ok(()) = text(&cmap, 0x0060, "fi");
  let Ok(()) = text(&cmap, 0x0061, "fl");

  Ok(())
}

#[test]
fn a_code_with_no_bf_entry_maps_to_nothing() -> Result<(), Failure> {
  let cmap = after_preamble(b"1 beginbfchar\n<0041> <0048>\nendbfchar\n")?;

  let Ok(()) = characters(&cmap, &[(0x0000, None), (0x0042, None)]);

  Ok(())
}

#[test]
fn identity_h_maps_every_two_byte_code_to_itself() -> Result<(), Failure> {
  let Ok(cmap) = CMap::identity_h();

  assert_eq!(cmap.metadata.writing_mode, Some(WritingMode::Horizontal));
  assert_eq!(
    Ok(cmap.metadata.character_collection.clone()),
    collection(CidFamily::AdobeIdentity, 0)
  );
  let Ok(()) = cids(
    &cmap,
    &[
      (0x0041, 2, Some(0x0041)),
      (0x1234, 2, Some(0x1234)),
      (0xFFFF, 2, Some(0xFFFF)),
      (0x0041, 1, None),
      (0x0041, 3, None),
    ],
  );

  Ok(())
}

#[test]
fn identity_v_is_the_same_map_written_vertically() -> Result<(), Failure> {
  let Ok(cmap) = CMap::identity_v();

  assert_eq!(cmap.metadata.writing_mode, Some(WritingMode::Vertical));
  let Ok(()) = cids(
    &cmap,
    &[(0x0041, 2, Some(0x0041)), (0xFFFF, 2, Some(0xFFFF))],
  );

  Ok(())
}

#[test]
fn a_cmap_with_no_name_and_no_writing_mode_has_neither() -> Result<(), Failure> {
  let cmap = without_bases(
    br#"
/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CMapType 2 def
1 begincodespacerange
<00><ff>
endcodespacerange
1 beginbfrange
<01><01><0020>
endbfrange
endcmap
"#,
  )?;

  assert_eq!(cmap.metadata.character_collection, None);
  assert_eq!(cmap.metadata.writing_mode, None);
  let Ok(()) = characters(&cmap, &[(0x01, Some(' '))]);

  Ok(())
}

#[test]
fn a_registry_written_as_a_name_is_read_as_one() -> Result<(), Failure> {
  let cmap = without_bases(
    br#"
/CIDSystemInfo
<< /Registry /ABCDEF+SimSun
/Ordering (pdfbeaninc)
/Supplement 0
>> def
/CMapName /ABCDEF+SimSun def
/CMapType 2 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
1 beginbfrange
<0000> <0000> <6881>
endbfrange
"#,
  )?;

  assert_eq!(
    Ok(cmap.metadata.character_collection.clone()),
    collection(
      CidFamily::Custom {
        registry: b"ABCDEF+SimSun".to_vec(),
        ordering: b"pdfbeaninc".to_vec()
      },
      0
    )
  );
  let Ok(()) = characters(&cmap, &[(0x0000, Some('\u{6881}'))]);

  Ok(())
}

#[test]
fn a_destination_of_one_byte_is_padded_to_a_unit() -> Result<(), Failure> {
  let cmap = after_preamble(b"1 beginbfrange\n<00> <7f> <00>\nendbfrange\n")?;

  let Ok(()) = characters(
    &cmap,
    &[
      (0x00, Some('\u{0000}')),
      (0x41, Some('A')),
      (0x7f, Some('\u{007F}')),
    ],
  );

  Ok(())
}

#[test]
fn a_name_is_what_a_pdf_writes_it_as() {
  assert_eq!(CMapName::from_bytes(b"Identity-H"), Ok(CMapName::IdentityH));
  assert_eq!(CMapName::from_bytes(b"Identity-V"), Ok(CMapName::IdentityV));
  assert_eq!(
    CMapName::from_bytes(b"UniJIS-UTF16-H"),
    Ok(CMapName::Custom(b"UniJIS-UTF16-H"))
  );
  assert_eq!(CMapName::IdentityV.to_bytes(), Ok(b"Identity-V".as_slice()));
}

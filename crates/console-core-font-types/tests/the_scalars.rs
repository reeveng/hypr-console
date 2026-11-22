use console_core_font_types::{
  BigEndian, BoundingBox, CFF_SFNT_VERSION, Compatibility, Compatible, CornerOrder, F2Dot14,
  F26Dot6, F48Dot16, FWord, Fixed, FixedSize, GlyphId, GlyphId16, GlyphId24, Int24, InvalidTag,
  LongDateTime, MajorMinor, Matrix, NameId, NameIdKind, Nullable, Nullness, Offset16, Offset24,
  Offset32, Point, Scalar, TRUE_SFNT_VERSION, TT_SFNT_VERSION, TTC_HEADER_TAG, Tag, UfWord,
  Uint24, Version16Dot16,
};
use console_core_number_conversion::fitted;

#[test]
fn glyph_identifiers_widen_and_narrow() {
  let Ok(small) = GlyphId16::new(513);
  let Ok(wide) = GlyphId24::new(0x0100_0000);
  let Ok(five) = GlyphId24::new(5);

  assert_eq!(small.to_u16(), Ok(513));
  assert_eq!(small.to_u32(), Ok(513));
  assert_eq!(small.to_be_bytes(), Ok([2, 1]));
  assert_eq!(u32::from(small), 513);
  assert_eq!(Ok(GlyphId16::from(7u16)), GlyphId16::new(7));
  assert_eq!(GlyphId16::default(), GlyphId16::NOTDEF);
  assert_eq!(Ok(GlyphId::from(small)), GlyphId::new(513));
  assert_eq!(GlyphId::new(513).map(|large| large == small), Ok(true));
  assert_eq!(GlyphId::new(513).map(|large| small == large), Ok(true));
  assert_eq!(GlyphId::new(514).map(|large| large > small), Ok(true));
  assert_eq!(GlyphId::new(514).map(|large| small < large), Ok(true));
  assert_eq!(Ok(GlyphId::from(9u16)), GlyphId::new(9));
  assert_eq!(GlyphId::from(9u32).to_u32(), Ok(9));
  assert_eq!(GlyphId::default(), GlyphId::NOTDEF);
  assert_eq!(
    GlyphId::new(70_000).map(|large| large.to_string()),
    Ok(String::from("GID_70000"))
  );
  assert_eq!(small.to_string(), "GID_513");
  assert_eq!(format!("{small:?}"), "GlyphId16(513)");
  assert_eq!(
    GlyphId::new(65_535)
      .map(|large| GlyphId16::try_from(large).map_err(|fault| fault.to_string())),
    GlyphId16::new(65_535).map(Ok)
  );
  assert_eq!(
    GlyphId::new(65_536)
      .map(|large| GlyphId16::try_from(large).map_err(|fault| fault.to_string())),
    Ok(Err(String::from(
      "glyph identifier 65536 too large for conversion"
    )))
  );
  assert_eq!(wide.to_u32(), Ok(0x00ff_ffff));
  assert_eq!(GlyphId24::checked_new(0x0100_0000), Ok(None));
  assert_eq!(GlyphId24::checked_new(0x00ff_ffff), Ok(Some(wide)));
  assert_eq!(wide.to_be_bytes(), Ok([0xff, 0xff, 0xff]));
  assert_eq!(
    GlyphId24::from_be_bytes([1, 2, 3]).and_then(GlyphId24::to_u32),
    Ok(0x0001_0203)
  );
  assert_eq!(u32::from(five), 5);
  assert_eq!(Ok(GlyphId::from(five)), GlyphId::new(5));
  assert_eq!(
    GlyphId::new(0x00ff_ffff)
      .map(|large| GlyphId24::try_from(large).map_err(|fault| fault.to_string())),
    Ok(Ok(wide))
  );
  assert_eq!(
    GlyphId::new(0x0100_0000)
      .map(|large| GlyphId24::try_from(large).map_err(|fault| fault.to_string())),
    Ok(Err(String::from(
      "glyph identifier 16777216 too large for conversion"
    )))
  );
  assert_eq!(GlyphId24::default(), GlyphId24::NOTDEF);
  assert_eq!(
    GlyphId24::new(12).map(|id| id.to_string()),
    Ok(String::from("GID_12"))
  );
}

#[test]
fn a_tag_is_four_bytes_read_and_written_as_they_are() {
  let Ok(head) = Tag::new(b"head");
  let Ok(blank) = Tag::new(b"    ");
  let Ok(padded) = Tag::new(b"ab  ");
  let Ok(short) = Tag::new(b"a   ");
  let Ok(late) = Tag::new(b"b   ");

  assert_eq!(Tag::from_u32(0x6865_6164), Ok(head));
  assert_eq!(Tag::from_be_bytes(*b"head"), Ok(head));
  assert_eq!(head.to_be_bytes(), Ok(*b"head"));
  assert_eq!(head.into_bytes(), Ok(*b"head"));
  assert_eq!(head, *b"head");
  assert_eq!(head, "head");
  assert_eq!(head, *"head");
  assert_eq!(head, b"head".as_slice());
  assert_eq!(head.as_ref(), b"head");
  assert_eq!(format!("{head:?}"), "Tag(head)");
  assert_eq!(head.to_string(), "head");
  assert_eq!(
    Tag::new(&[0x19, b'z', b'@', 0x7f]).map(|odd| odd.to_string()),
    Ok(String::from("{0x19}z@{0x7F}"))
  );
  assert_eq!(Tag::default(), blank);
  assert_eq!(Tag::new(b"ttcf"), Ok(TTC_HEADER_TAG));

  let mut sorted = [late, short, padded];

  sorted.sort();
  assert_eq!(sorted, [short, padded, late]);
}

#[test]
fn tag_text_requires_printable_ascii_followed_only_by_padding() {
  let Ok(head) = Tag::new(b"head");
  let Ok(blank) = Tag::new(b"    ");
  let Ok(padded) = Tag::new(b"ab  ");
  let Ok(short) = Tag::new(b"a   ");
  let Ok(late_letter) = Tag::new(b"a b ");
  let Ok(control) = Tag::new(b"a\x01  ");
  let Ok(leading_space) = Tag::new(b" abc");

  assert_eq!("ab".parse::<Tag>(), Ok(padded));
  assert_eq!("a  ".parse::<Tag>(), Ok(short));
  assert_eq!("".parse::<Tag>(), Err(InvalidTag::InvalidLength(0)));
  assert_eq!("abcde".parse::<Tag>(), Err(InvalidTag::InvalidLength(5)));
  assert_eq!(
    " a".parse::<Tag>(),
    Err(InvalidTag::InvalidByte { pos: 0, byte: 0x20 })
  );
  assert_eq!(
    "a b".parse::<Tag>(),
    Err(InvalidTag::InvalidByte { pos: 2, byte: b'b' })
  );
  assert_eq!(
    "a\u{7f}".parse::<Tag>(),
    Err(InvalidTag::InvalidByte { pos: 1, byte: 0x7f })
  );
  assert_eq!(
    Tag::new_checked(&[0x21]).map(Tag::into_bytes),
    Ok(Ok(*b"!   "))
  );
  assert_eq!(
    Tag::new_checked(&[0x7e]).map(Tag::into_bytes),
    Ok(Ok(*b"~   "))
  );
  assert_eq!(
    late_letter.validate(),
    Err(InvalidTag::ByteAfterSpace { pos: 2 })
  );
  assert_eq!(
    control.validate(),
    Err(InvalidTag::InvalidByte { pos: 1, byte: 1 })
  );
  assert_eq!(blank.validate(), Err(InvalidTag::InvalidLength(0)));
  assert_eq!(
    leading_space.validate(),
    Err(InvalidTag::InvalidByte { pos: 0, byte: 0x20 })
  );
  assert_eq!(head.validate(), Ok(()));
  assert_eq!(short.validate(), Ok(()));
  assert_eq!(
    InvalidTag::InvalidLength(9).to_string(),
    "Invalid length (9)"
  );
  assert_eq!(
    InvalidTag::InvalidByte { pos: 1, byte: 0x7f }.to_string(),
    "Invalid byte 0x7F at index 1"
  );
  assert_eq!(
    InvalidTag::ByteAfterSpace { pos: 3 }.to_string(),
    "Non-space character after first space"
  );
}

#[test]
fn an_offset_is_null_at_zero() {
  let Ok(three) = Offset16::new(3);
  let Ok(four) = Offset16::new(4);
  let held = BigEndian::<Nullable<Offset16>>::from_slice(&[0, 12]).map(|raw| {
    raw.map(|bytes| {
      let Ok(byte) = bytes.get();

      byte
    })
  });

  assert_eq!(
    Offset16::new(0).and_then(Offset16::nullness),
    Ok(Nullness::Null)
  );
  assert_eq!(
    Offset16::new(1).and_then(Offset16::nullness),
    Ok(Nullness::NonNull)
  );
  assert_eq!(Offset16::new(65_535).and_then(Offset16::to_u32), Ok(65_535));
  assert_eq!(
    Uint24::new(0)
      .and_then(Offset24::new)
      .and_then(Offset24::nullness),
    Ok(Nullness::Null)
  );
  assert_eq!(
    Uint24::new(0x0001_0203)
      .and_then(Offset24::new)
      .and_then(Offset24::to_u32),
    Ok(0x0001_0203)
  );
  assert_eq!(
    Offset32::new(0).and_then(Offset32::nullness),
    Ok(Nullness::Null)
  );
  assert_eq!(
    Offset32::new(u32::MAX).and_then(Offset32::to_u32),
    Ok(u32::MAX)
  );
  assert_eq!(Offset32::new(9).map(|offset| offset == 9), Ok(true));
  assert_eq!(
    Nullable::<Offset16>::default().nullness(),
    Ok(Nullness::Null)
  );
  assert_eq!(
    Nullable::<Offset24>::default().nullness(),
    Ok(Nullness::Null)
  );
  assert_eq!(
    Nullable::<Offset32>::default().nullness(),
    Ok(Nullness::Null)
  );
  assert_eq!(
    held.map(|read| read.map(|offset| match offset.offset() {
      Ok(inner) => *inner,
    })),
    Offset16::new(12).map(Some)
  );
  assert_eq!(
    held.map(|read| read.map(|offset| offset == 12)),
    Ok(Some(true))
  );
  assert_eq!(
    held.map(|read| read.map(|offset| {
      let Ok(offset_nullness) = offset.nullness();

      offset_nullness
    })),
    Ok(Some(Nullness::NonNull))
  );
  assert!(three < four);
}

#[test]
fn a_name_identifier_knows_the_reserved_ones() {
  let Ok(predefined) = NameId::predefined();
  let numbers: Vec<u16> = predefined
    .clone()
    .map(|id| match id.to_u16() {
      Ok(number) => number,
    })
    .collect();
  let expected: Vec<u16> = (0..15).chain(16..=25).collect();
  let names: Vec<String> = predefined.map(|id| format!("{id:?}")).collect();

  assert_eq!(
    NameId::new(255).and_then(NameId::kind),
    Ok(NameIdKind::Standard)
  );
  assert_eq!(
    NameId::new(256).and_then(NameId::kind),
    Ok(NameIdKind::FontSpecific)
  );
  assert_eq!(
    NameId::new(32_766).and_then(|id| id.checked_add(1)),
    Ok(Some(NameId::LAST_ALLOWED_NAME_ID))
  );
  assert_eq!(
    NameId::new(32_767).and_then(|id| id.checked_add(1)),
    Ok(None)
  );
  assert_eq!(
    NameId::new(65_535).and_then(|id| id.checked_add(1)),
    Ok(None)
  );
  assert_eq!(NameId::new(7).and_then(NameId::to_u16), Ok(7));
  assert_eq!(NameId::new(258).and_then(NameId::to_be_bytes), Ok([1, 2]));
  assert_eq!(NameId::from(4u16), NameId::FULL_NAME);
  assert_eq!(NameId::default().to_u16(), Ok(0xffff));
  assert_eq!(numbers, expected);
  assert_eq!(format!("{:?}", NameId::FAMILY_NAME), "FAMILY_NAME");
  assert_eq!(
    NameId::VARIATIONS_POSTSCRIPT_NAME_PREFIX.to_string(),
    "VARIATIONS_POSTSCRIPT_NAME_PREFIX"
  );
  assert_eq!(
    NameId::new(15).map(|id| format!("{id:?}")),
    Ok(String::from("NameId 15"))
  );
  assert_eq!(
    NameId::new(300).map(|id| id.to_string()),
    Ok(String::from("NameId 300"))
  );
  assert_eq!(fitted::<_, u32>(names.len()), Ok(25));
  assert_eq!(names.first().map(String::as_str), Some("COPYRIGHT_NOTICE"));
  assert_eq!(
    names.get(15).map(String::as_str),
    Some("TYPOGRAPHIC_FAMILY_NAME")
  );
}

#[test]
fn twenty_four_bit_integers_hold_their_range() {
  let Ok(seven) = Uint24::new(7);
  let Ok(minus_seven) = Int24::new(-7);

  assert_eq!(Uint24::new(u32::MAX), Ok(Uint24::MAX));
  assert_eq!(Uint24::checked_new(u32::MAX), Ok(None));
  assert_eq!(Uint24::checked_new(0x00ff_ffff), Ok(Some(Uint24::MAX)));
  assert_eq!(
    Uint24::checked_new(5).map(|held| held.map(|five| match five.to_u32() {
      Ok(value) => value,
    })),
    Ok(Some(5))
  );
  assert_eq!(u32::from(seven), 7);
  assert_eq!(
    Uint24::new(70_000).map(|held| held.to_string()),
    Ok(String::from("70000"))
  );
  assert_eq!(
    Uint24::new(0x0001_0203).and_then(Uint24::to_be_bytes),
    Ok([1, 2, 3])
  );
  assert_eq!(
    Uint24::from_be_bytes([0xff, 0xaa, 0xcc]).and_then(Uint24::to_be_bytes),
    Ok([0xff, 0xaa, 0xcc])
  );
  assert_eq!(Int24::new(i32::MAX), Ok(Int24::MAX));
  assert_eq!(Int24::new(i32::MIN), Ok(Int24::MIN));
  assert_eq!(Int24::checked_new(0x0080_0000), Ok(None));
  assert_eq!(Int24::checked_new(-0x0080_0001), Ok(None));
  assert_eq!(Int24::checked_new(-0x0080_0000), Ok(Some(Int24::MIN)));
  assert_eq!(i32::from(minus_seven), -7);
  assert_eq!(
    Int24::new(-70_000).map(|held| held.to_string()),
    Ok(String::from("-70000"))
  );
  assert_eq!(
    Int24::new(123_456).and_then(Int24::to_be_bytes),
    Ok([0x01, 0xe2, 0x40])
  );
  assert_eq!(
    Int24::new(-123_456).and_then(Int24::to_be_bytes),
    Ok([0xfe, 0x1d, 0xc0])
  );
  assert_eq!(Int24::MAX.to_be_bytes(), Ok([0x7f, 0xff, 0xff]));
  assert_eq!(Int24::MIN.to_be_bytes(), Ok([0x80, 0, 0]));

  for raw in -0x0080_0000..=0x007f_ffff {
    let Ok(held) = Int24::new(raw);

    assert_eq!(held.to_be_bytes().and_then(Int24::from_be_bytes), Ok(held));
    assert_eq!(held.to_i32(), Ok(raw));
  }
}

#[test]
fn big_endian_bytes_read_as_the_value_they_hold() {
  let Ok(mut raw) = BigEndian::<u16>::new([0, 1]);
  let negative = BigEndian::from(-1i16);
  let positive = BigEndian::from(1i16);

  assert_eq!(
    BigEndian::<i16>::from_slice(&[0xff, 0xfe]).map(|held| held.map(|bytes| {
      let Ok(byte) = bytes.get();

      byte
    })),
    Ok(Some(-2))
  );
  assert_eq!(BigEndian::<i16>::from_slice(&[0xff]), Ok(None));
  assert_eq!(BigEndian::<i16>::from_slice(&[0xff, 0xfe, 0]), Ok(None));
  assert_eq!(i16::read(&[0x01, 0x02]), Ok(Some(0x0102)));
  assert_eq!(u32::read(&[0x01, 0x02]), Ok(None));
  assert_eq!(Uint24::read(&[1, 2, 3]), Uint24::new(0x0001_0203).map(Some));
  assert_eq!(Int24::read(&[0xff, 0xff, 0xfe]), Int24::new(-2).map(Some));
  assert_eq!(i64::read(&[0xff; 8]), Ok(Some(-1)));
  assert_eq!(u8::read(&[9]), Ok(Some(9)));
  assert_eq!(i8::read(&[0xff]), Ok(Some(-1)));
  assert_eq!(Tag::read(b"cmap"), Tag::new(b"cmap").map(Some));
  assert_eq!(
    MajorMinor::read(&[0, 1, 0, 2]),
    Ok(Some(MajorMinor::VERSION_1_2))
  );
  assert_eq!(
    Fixed::read(&[0, 1, 0x80, 0]),
    Fixed::from_f64(1.5).map(Some)
  );
  assert_eq!(
    LongDateTime::read(&[0, 0, 0, 0, 0, 0, 1, 0]).map(|date| date.map(|held| {
      let Ok(secs) = held.as_secs();

      secs
    })),
    Ok(Some(256))
  );
  assert_eq!(raw.get(), Ok(1));
  assert_eq!(raw.set(0x0203), Ok(()));
  assert_eq!(raw.be_bytes(), Ok([2u8, 3].as_slice()));
  assert_eq!(raw, 0x0203);
  assert_eq!(BigEndian::from(0x0405u16).get(), Ok(0x0405));
  assert_eq!(BigEndian::<u16>::default().get(), Ok(0));
  assert!(negative < positive);
  assert_eq!(negative.cmp(&positive), core::cmp::Ordering::Less);
  assert_eq!(format!("{negative:?}"), "-1");
  assert_eq!(positive.to_string(), "1");
  assert_eq!(format!("{:?}", BigEndian::from(Fixed::ONE)), "1");
}

#[test]
fn a_scalar_takes_as_many_bytes_as_the_file_gives_it() {
  assert_eq!(<u16 as FixedSize>::RAW_BYTE_LEN, 2);
  assert_eq!(<Uint24 as FixedSize>::RAW_BYTE_LEN, 3);
  assert_eq!(<Offset24 as FixedSize>::RAW_BYTE_LEN, 3);
  assert_eq!(<BigEndian<Offset24> as FixedSize>::RAW_BYTE_LEN, 3);
  assert_eq!(<LongDateTime as FixedSize>::RAW_BYTE_LEN, 8);
  assert_eq!(<Tag as FixedSize>::RAW_BYTE_LEN, 4);
  assert_eq!(<MajorMinor as FixedSize>::RAW_BYTE_LEN, 4);
  assert_eq!(<u8 as FixedSize>::RAW_BYTE_LEN, 1);
  assert_eq!(<GlyphId24 as FixedSize>::RAW_BYTE_LEN, 3);
  assert_eq!(<F2Dot14 as FixedSize>::RAW_BYTE_LEN, 2);
  assert_eq!(<NameId as FixedSize>::RAW_BYTE_LEN, 2);
  assert_eq!(<Version16Dot16 as FixedSize>::RAW_BYTE_LEN, 4);
  assert_eq!(<Nullable<Offset32> as FixedSize>::RAW_BYTE_LEN, 4);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CompatibilityId([u8; 16]);

impl Scalar for CompatibilityId {
  type Raw = [u8; 16];

  fn from_raw(raw: [u8; 16]) -> Result<Self, console_core_never::Never> {
    Ok(CompatibilityId(raw))
  }

  fn to_raw(self) -> Result<[u8; 16], console_core_never::Never> {
    Ok(self.0)
  }
}

#[test]
fn a_sixteen_byte_identifier_is_a_scalar() {
  let written: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
  let held = BigEndian::<CompatibilityId>::from_slice(&written).map(|raw| {
    raw.map(|bytes| {
      let Ok(byte) = bytes.get();

      byte
    })
  });

  assert_eq!(<CompatibilityId as FixedSize>::RAW_BYTE_LEN, 16);
  assert_eq!(held, Ok(Some(CompatibilityId(written))));
  assert_eq!(
    CompatibilityId::read(&written),
    Ok(Some(CompatibilityId(written)))
  );
  assert_eq!(CompatibilityId::read(&[0; 15]), Ok(None));
}

#[test]
fn every_scalar_writes_what_it_reads() {
  assert_eq!(
    Fixed::from_f64(-1.5).and_then(Fixed::to_raw),
    Ok([0xff, 0xfe, 0x80, 0])
  );
  assert_eq!(
    Fixed::from_raw([0xff, 0xfe, 0x80, 0]),
    Fixed::from_f64(-1.5)
  );
  assert_eq!(GlyphId16::new(258).and_then(GlyphId16::to_raw), Ok([1, 2]));
  assert_eq!(
    GlyphId24::new(0x0001_0203).and_then(GlyphId24::to_raw),
    Ok([1, 2, 3])
  );
  assert_eq!(GlyphId24::from_raw([1, 2, 3]), GlyphId24::new(0x0001_0203));
  assert_eq!(NameId::from_raw([1, 2]), NameId::new(258));
  assert_eq!(
    Version16Dot16::from_raw([0, 2, 0x50, 0]),
    Ok(Version16Dot16::VERSION_2_5)
  );
  assert_eq!(Version16Dot16::VERSION_2_5.to_raw(), Ok([0, 2, 0x50, 0]));
  assert_eq!(MajorMinor::VERSION_1_3.to_raw(), Ok([0, 1, 0, 3]));
  assert_eq!(
    Uint24::new(5)
      .and_then(Offset24::new)
      .and_then(Offset24::to_raw),
    Ok([0, 0, 5])
  );
  assert_eq!(Offset16::from_raw([0, 5]), Offset16::new(5));
  assert_eq!(
    LongDateTime::new(-1).and_then(LongDateTime::to_raw),
    Ok([0xff; 8])
  );
  assert_eq!(
    LongDateTime::new(-1).and_then(LongDateTime::to_be_bytes),
    Ok([0xff; 8])
  );
  assert_eq!(
    LongDateTime::from_raw([0xff; 8]).and_then(|date| date.as_secs()),
    Ok(-1)
  );
  assert_eq!(Tag::from_raw(*b"GSUB").and_then(Tag::to_raw), Ok(*b"GSUB"));
  assert_eq!(FWord::from_raw([0xff, 0xff]), FWord::new(-1));
  assert_eq!(F2Dot14::from_raw([0x40, 0]), Ok(F2Dot14::ONE));
}

#[test]
fn versions_are_compatible_within_one_major_number() {
  assert_eq!(
    MajorMinor::VERSION_1_2.compatibility(MajorMinor::VERSION_1_1),
    Ok(Compatibility::Compatible)
  );
  assert_eq!(
    MajorMinor::VERSION_1_1.compatibility(MajorMinor::VERSION_1_1),
    Ok(Compatibility::Compatible)
  );
  assert_eq!(
    MajorMinor::VERSION_1_1.compatibility(MajorMinor::VERSION_1_2),
    Ok(Compatibility::Incompatible)
  );
  assert_eq!(
    MajorMinor::VERSION_2_0.compatibility(MajorMinor::VERSION_1_0),
    Ok(Compatibility::Incompatible)
  );
  assert_eq!(
    MajorMinor::VERSION_1_3.compatibility((1u16, 0u16)),
    Ok(Compatibility::Compatible)
  );
  assert_eq!(
    MajorMinor::VERSION_1_3.compatibility((2u16, 0u16)),
    Ok(Compatibility::Incompatible)
  );
  assert_eq!(
    Version16Dot16::VERSION_2_5.compatibility(Version16Dot16::VERSION_2_0),
    Ok(Compatibility::Compatible)
  );
  assert_eq!(
    Version16Dot16::VERSION_2_0.compatibility(Version16Dot16::VERSION_2_5),
    Ok(Compatibility::Incompatible)
  );
  assert_eq!(
    Version16Dot16::VERSION_3_0.compatibility(Version16Dot16::VERSION_2_0),
    Ok(Compatibility::Incompatible)
  );
  assert_eq!(
    Version16Dot16::VERSION_1_1.compatibility((1u16, 1u16)),
    Ok(Compatibility::Compatible)
  );
  assert_eq!(
    Version16Dot16::VERSION_1_1.compatibility((1u16, 2u16)),
    Ok(Compatibility::Incompatible)
  );
  assert_eq!(
    Version16Dot16::VERSION_1_1.compatibility((1u16, 12u16)),
    Ok(Compatibility::Incompatible)
  );
  assert_eq!(3u16.compatibility(2u16), Ok(Compatibility::Compatible));
  assert_eq!(1u16.compatibility(2u16), Ok(Compatibility::Incompatible));
}

#[test]
fn a_version_is_written_with_its_minor_number_as_one_digit() {
  assert_eq!(Version16Dot16::VERSION_0_5.to_string(), "0.5");
  assert_eq!(
    format!("{:?}", Version16Dot16::VERSION_1_1),
    "Version16Dot16(00011000)"
  );
  assert_eq!(MajorMinor::VERSION_2_0.to_string(), "2.0");
  assert_eq!(
    format!("{:?}", MajorMinor::VERSION_1_0),
    "MajorMinor { major: 1, minor: 0 }"
  );
  assert_eq!(
    Version16Dot16::new(MajorMinor { major: 3, minor: 0 }),
    Ok(Version16Dot16::VERSION_3_0)
  );
  assert_eq!(
    Version16Dot16::new(MajorMinor { major: 0, minor: 5 }),
    Ok(Version16Dot16::VERSION_0_5)
  );
  assert_eq!(
    Version16Dot16::new(MajorMinor {
      major: 65_535,
      minor: 4
    })
    .and_then(Version16Dot16::to_major_minor),
    Ok((65_535, 4))
  );
  assert_eq!(
    Version16Dot16::VERSION_2_5.to_be_bytes(),
    Ok([0, 2, 0x50, 0])
  );
  assert_eq!(
    Version16Dot16::from_raw([0, 1, 0xff, 0xff]).and_then(Version16Dot16::to_major_minor),
    Ok((1, 15))
  );
  assert_eq!(MajorMinor::VERSION_1_2.to_be_bytes(), Ok([0, 1, 0, 2]));
  assert!(MajorMinor::VERSION_1_0 < MajorMinor::VERSION_1_1);
  assert!(Version16Dot16::VERSION_1_1 < Version16Dot16::VERSION_2_0);

  for minor in 0..=9 {
    assert_eq!(
      Version16Dot16::new(MajorMinor { major: 1, minor })
        .and_then(Version16Dot16::to_major_minor),
      Ok((1, minor))
    );
  }

  for minor in [10, u16::MAX] {
    assert_eq!(
      Version16Dot16::new(MajorMinor { major: 1, minor })
        .and_then(Version16Dot16::to_major_minor),
      Ok((1, 9))
    );
  }
}

#[test]
fn a_font_unit_is_sixteen_bits_and_a_fixed_point_number_away() {
  assert_eq!(
    FWord::new(-12).and_then(FWord::to_fixed),
    Fixed::from_i32(-12)
  );
  assert_eq!(
    UfWord::new(65535).and_then(UfWord::to_fixed),
    Fixed::from_i32(65535)
  );
  assert_eq!(FWord::new(-12).and_then(FWord::to_i16), Ok(-12));
  assert_eq!(UfWord::new(65535).and_then(UfWord::to_u16), Ok(65535));
  assert_eq!(
    FWord::new(-2).and_then(FWord::to_be_bytes),
    Ok([0xff, 0xfe])
  );
  assert_eq!(UfWord::new(258).and_then(UfWord::to_be_bytes), Ok([1, 2]));
  assert_eq!(i16::from(FWord::from(-5)), -5);
  assert_eq!(u16::from(UfWord::from(5)), 5);
  assert_eq!(
    FWord::new(-5).map(|unit| unit.to_string()),
    Ok(String::from("-5"))
  );
  assert_eq!(
    UfWord::new(5).map(|unit| unit.to_string()),
    Ok(String::from("5"))
  );
  assert_eq!(
    F48Dot16::from_bits(0x0001_8000)
      .and_then(|delta| FWord::new(10).and_then(|unit| unit.apply_delta(delta))),
    Ok(11.5)
  );
  assert_eq!(
    F48Dot16::from_bits(-0x0001_8000)
      .and_then(|delta| UfWord::new(10).and_then(|unit| unit.apply_delta(delta))),
    Ok(8.5)
  );
  assert_eq!(
    F48Dot16::from_f64(-0.25)
      .and_then(|delta| FWord::new(-100).and_then(|unit| unit.apply_delta(delta))),
    Ok(-100.25)
  );
  assert_eq!(
    UfWord::new(0).and_then(|unit| unit.apply_delta(F48Dot16::ZERO)),
    Ok(0.0)
  );
}

#[test]
fn a_date_is_seconds_from_1904() {
  let Ok(early) = LongDateTime::new(-5);
  let Ok(late) = LongDateTime::new(5);

  assert_eq!(
    LongDateTime::new(3_000_000_000).and_then(|date| date.as_secs()),
    Ok(3_000_000_000)
  );
  assert_eq!(LongDateTime::default().as_secs(), Ok(0));
  assert!(early < late);
}

#[test]
fn a_matrix_moves_a_point() {
  let Ok(matrix) = Matrix::from_elements([2.0f64, 0.5, -1.0, 3.0, 10.0, -20.0]);
  let Ok(nudge) = Matrix::from_elements([1.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
  let Ok(halved) = matrix.map(|element| element / 2.0);
  let Ok(two) = Fixed::from_i32(2);
  let Ok(fixed) = Matrix::from_elements([
    Fixed::ONE,
    Fixed::ZERO,
    Fixed::ZERO,
    two,
    Fixed::ONE,
    Fixed::ZERO,
  ]);
  let Ok(three) = Fixed::from_i32(3);
  let Ok(four) = Fixed::from_i32(4);
  let Ok(eight) = Fixed::from_i32(8);
  let mut chained = matrix;

  chained *= nudge;
  assert_eq!(matrix.transform(1.0, 2.0), Ok((10.0, -13.5)));
  assert_eq!(
    Point::new(1.0, 2.0).map(|point| matrix * point),
    Point::new(10.0, -13.5)
  );
  assert_eq!(matrix.elements(), Ok([2.0, 0.5, -1.0, 3.0, 10.0, -20.0]));
  assert_eq!(Matrix::<f64>::default(), Matrix::IDENTITY);
  assert_eq!(halved.elements(), Ok([1.0, 0.25, -0.5, 1.5, 5.0, -10.0]));
  assert_eq!(chained, matrix * nudge);
  assert_eq!(fixed.transform(three, four), Ok((four, eight)));
  assert_eq!(
    Matrix::<f32>::IDENTITY.elements(),
    Ok([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
  );
}

#[test]
fn a_point_is_two_coordinates() {
  let Ok(mut point) = Point::new(9, 6);

  assert_eq!(Point::broadcast(3), Point::new(3, 3));
  assert_eq!(
    Point::new(1i32, 2).and_then(|pair| pair.map(|coordinate| coordinate.wrapping_mul(10))),
    Point::new(10, 20)
  );
  assert_eq!(Ok(Point::<i32>::default()), Point::new(0, 0));
  point /= 3;
  assert_eq!(Ok(point), Point::new(3, 2));
  point *= 2;
  assert_eq!(Ok(point), Point::new(6, 4));

  let Ok(by) = Point::new(2, 4);

  point /= by;
  assert_eq!(Ok(point), Point::new(3, 1));
}

#[test]
fn a_point_is_added_scaled_and_negated_one_coordinate_at_a_time() {
  let Ok(one_two) = Point::new(1, 2);
  let Ok(three_four) = Point::new(3, 4);
  let Ok(ten_sixteen) = Point::new(10, 16);
  let Ok(two_three) = Point::new(2, 3);
  let Ok(x) = F26Dot6::from_f64(42.5);
  let Ok(y) = F26Dot6::from_f64(20.1);
  let mut moved = one_two;

  assert_eq!(Ok(one_two + three_four), Point::new(4, 6));
  assert_eq!(Ok(one_two - three_four), Point::new(-2, -2));
  assert_eq!(Ok(one_two * three_four), Point::new(3, 8));
  assert_eq!(Ok(one_two * 8), Point::new(8, 16));
  assert_eq!(Ok(ten_sixteen / two_three), Point::new(5, 5));
  assert_eq!(Ok(ten_sixteen / 2), Point::new(5, 8));
  assert_eq!(Ok(-one_two), Point::new(-1, -2));
  moved += three_four;
  assert_eq!(Ok(moved), Point::new(4, 6));
  moved -= three_four;
  assert_eq!(Ok(moved), Point::new(1, 2));
  moved *= three_four;
  assert_eq!(Ok(moved), Point::new(3, 8));
  assert_eq!(
    Point::new(42.5, 20.1).and_then(|pair| pair.map(|coordinate| {
      let Ok(f26dot6) = F26Dot6::from_f64(coordinate);

      f26dot6
    })),
    Point::new(x, y)
  );
}

#[test]
fn a_product_of_matrices_applies_the_right_one_first() {
  let Ok(translate) = Matrix::from_elements([1.0f32, 0.0, 0.0, 1.0, 10.0, 20.0]);
  let Ok(scale) = Matrix::from_elements([2.0, 0.0, 0.0, 3.0, 0.0, 0.0]);
  let Ok(skew) = Matrix::from_elements([0.5f32, 1.0, -2.0, 0.25, 7.0, -3.0]);
  let Ok(point) = Point::new(5.0, -22.0);

  assert_eq!(Matrix::IDENTITY * skew, skew);
  assert_eq!(skew * Matrix::IDENTITY, skew);
  assert_eq!(
    (translate * scale).elements(),
    Ok([2.0, 0.0, 0.0, 3.0, 10.0, 20.0])
  );
  assert_eq!(
    (scale * translate).elements(),
    Ok([2.0, 0.0, 0.0, 3.0, 20.0, 60.0])
  );
  assert_eq!(Ok(translate * point), Point::new(15.0, -2.0));
  assert_eq!(Ok(scale * point), Point::new(10.0, -66.0));
  assert_eq!(Ok(translate * scale * point), Point::new(20.0, -46.0));
  assert_eq!(Ok(scale * translate * point), Point::new(30.0, -6.0));
}

#[test]
fn a_bounding_box_scales_and_normalizes() {
  let Ok(minus_one) = Fixed::from_i32(-1);
  let Ok(one) = Fixed::from_i32(1);
  let Ok(two) = Fixed::from_i32(2);
  let Ok(three) = Fixed::from_i32(3);
  let Ok(four) = Fixed::from_i32(4);
  let Ok(six) = Fixed::from_i32(6);
  let Ok(minus_two) = Fixed::from_i32(-2);
  let bounds = BoundingBox {
    x_min: minus_one,
    y_min: two,
    x_max: three,
    y_max: one,
  };
  let Ok(normal) = bounds.normalize();
  let flat = BoundingBox {
    x_min: 1,
    y_min: 1,
    x_max: 1,
    y_max: 1,
  };

  assert_eq!(bounds.corner_order(), Ok(CornerOrder::OutOfOrder));
  assert_eq!(
    normal,
    BoundingBox {
      x_min: minus_one,
      y_min: one,
      x_max: three,
      y_max: two
    }
  );
  assert_eq!(normal.corner_order(), Ok(CornerOrder::InOrder));
  assert_eq!(
    normal.scale(two),
    Ok(BoundingBox {
      x_min: minus_two,
      y_min: two,
      x_max: six,
      y_max: four
    })
  );
  assert_eq!(flat.corner_order(), Ok(CornerOrder::InOrder));
  assert_eq!(
    BoundingBox::<i32>::default(),
    BoundingBox {
      x_min: 0,
      y_min: 0,
      x_max: 0,
      y_max: 0
    }
  );
}

#[test]
fn the_versions_of_an_sfnt_are_their_four_bytes() {
  assert_eq!(TT_SFNT_VERSION.to_be_bytes(), [0, 1, 0, 0]);
  assert_eq!(TRUE_SFNT_VERSION.to_be_bytes(), *b"true");
  assert_eq!(CFF_SFNT_VERSION.to_be_bytes(), *b"OTTO");
}

use console_core_iso_media_boxes::{
  BoxError, BoxHeader, Children, FourCharacterCode, FullBoxHeader,
};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const PAYLOAD: u64 = 3;

enum Error {
  FixtureTooShort,
  Rejected(BoxError),
}

impl std::fmt::Debug for Error {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Error::FixtureTooShort => write!(
        to,
        "a fixture is shorter than the bytes it was built to hold"
      ),
      Error::Rejected(refused) => write!(to, "a header that should parse was refused: {refused}"),
    }
  }
}

impl From<BoxError> for Error {
  fn from(refused: BoxError) -> Error {
    Error::Rejected(refused)
  }
}

#[derive(Clone, Copy)]
enum SizeField {
  Compact,
  Large,
}

struct Case {
  kind: [u8; 4],
  field: SizeField,
  user_type: Option<[u8; 16]>,
  width: u64,
}

fn encoded(case: &Case, size: u64) -> Result<Vec<u8>, Never> {
  let Ok(compact) = fitted::<u64, u32>(size);
  let (field, large) = match case.field {
    SizeField::Compact => (compact, None),
    SizeField::Large => (1, Some(size)),
  };
  let mut bytes = Vec::from(field.to_be_bytes());

  bytes.extend(case.kind);
  bytes.extend(large.map(u64::to_be_bytes).into_iter().flatten());
  bytes.extend(case.user_type.into_iter().flatten());

  Ok(bytes)
}

#[test]
fn short_extended_and_user_type_headers_share_one_size_rule() -> Result<(), Error> {
  for case in [
    Case {
      kind: *b"mdat",
      field: SizeField::Compact,
      user_type: None,
      width: 8,
    },
    Case {
      kind: *b"mdat",
      field: SizeField::Large,
      user_type: None,
      width: 16,
    },
    Case {
      kind: *b"uuid",
      field: SizeField::Compact,
      user_type: Some([7; 16]),
      width: 24,
    },
    Case {
      kind: *b"uuid",
      field: SizeField::Large,
      user_type: Some([9; 16]),
      width: 32,
    },
  ] {
    let size = case.width.saturating_add(PAYLOAD);
    let Ok(bytes) = encoded(&case, size);
    let prefix = bytes.first_chunk::<8>().ok_or(Error::FixtureTooShort)?;

    assert_eq!(BoxHeader::required_len(prefix), Ok(case.width));

    let header = BoxHeader::parse(&bytes)?;

    assert_eq!(header.kind, case.kind);
    assert_eq!(header.header_len, case.width);
    assert_eq!(header.size, Some(size));
    assert_eq!(header.payload_len(), Ok(Some(PAYLOAD)));
    assert_eq!(header.user_type, case.user_type);

    for length in 0..case.width {
      let Ok(end) = index(length);
      let shorter = bytes.get(..end).ok_or(Error::FixtureTooShort)?;

      assert!(matches!(
        BoxHeader::parse(shorter),
        Err(BoxError::Truncated { .. })
      ));
    }
  }

  Ok(())
}

#[test]
fn an_explicit_size_must_cover_the_complete_header() {
  for case in [
    Case {
      kind: *b"free",
      field: SizeField::Compact,
      user_type: None,
      width: 8,
    },
    Case {
      kind: *b"free",
      field: SizeField::Large,
      user_type: None,
      width: 16,
    },
    Case {
      kind: *b"uuid",
      field: SizeField::Compact,
      user_type: Some([0; 16]),
      width: 24,
    },
    Case {
      kind: *b"uuid",
      field: SizeField::Large,
      user_type: Some([0; 16]),
      width: 32,
    },
  ] {
    let Ok(bytes) = encoded(&case, case.width.saturating_sub(1));

    assert_eq!(fitted::<_, u64>(bytes.len()), Ok(case.width));
    assert!(matches!(
      BoxHeader::parse(&bytes),
      Err(BoxError::InvalidSize { .. })
    ));
  }
}

#[test]
fn a_zero_size_is_resolved_by_the_parent_instead_of_the_header() -> Result<(), Error> {
  let bytes = [0, 0, 0, 0, b'm', b'd', b'a', b't', 1, 2, 3];
  let (prefix, rest) = bytes
    .split_first_chunk::<8>()
    .ok_or(Error::FixtureTooShort)?;
  let header = BoxHeader::parse(prefix)?;

  assert_eq!(header.size, None);
  assert_eq!(header.payload_len(), Ok(None));

  let Ok(walk) = Children::new(&bytes);
  let children = walk.collect::<Result<Vec<_>, _>>()?;

  assert_eq!(children.len(), 1);

  let only = children.first().ok_or(Error::FixtureTooShort)?;

  assert_eq!(only.payload, &[1, 2, 3]);
  assert_eq!(only.payload.as_ptr(), rest.as_ptr());

  Ok(())
}

#[test]
fn child_iteration_stops_at_damage_and_accepts_quicktime_zero_padding() -> Result<(), Error> {
  let valid = [0, 0, 0, 9, b'f', b'r', b'e', b'e', 42, 0, 0, 0, 0];
  let Ok(walk) = Children::new(&valid);
  let children = walk.collect::<Result<Vec<_>, _>>()?;

  assert_eq!(children.len(), 1);

  let only = children.first().ok_or(Error::FixtureTooShort)?;

  assert_eq!(only.payload, &[42]);

  let damaged = [0, 0, 0, 99, b'f', b'r', b'e', b'e', 0];
  let Ok(mut damaged_walk) = Children::new(&damaged);

  assert!(matches!(
    damaged_walk.next(),
    Some(Err(BoxError::ExceedsContainer { .. }))
  ));
  assert!(damaged_walk.next().is_none());
  assert!(damaged_walk.next().is_none());

  let Ok(mut short_walk) = Children::new(&[0, 0, 0, 1]);

  assert!(matches!(
    short_walk.next(),
    Some(Err(BoxError::Truncated { .. }))
  ));

  Ok(())
}

#[test]
fn full_box_version_and_flags_keep_all_their_bits() {
  let Ok(header) = FullBoxHeader::from_bytes([0x81, 0xfe, 0xdc, 0xba]);

  assert_eq!(header.version, 0x81);
  assert_eq!(header.flags, 0x00fe_dcba);
}

#[test]
fn four_character_codes_preserve_the_wire_bytes_and_display_convention() {
  let Ok(code) = FourCharacterCode::new(b"mdat");

  assert_eq!(code.as_u32(), Ok(u32::from_be_bytes(*b"mdat")));
  assert_eq!(code, *b"mdat");

  let Ok(item_name) = FourCharacterCode([0xa9, b'n', b'a', b'm']).to_string_lossy();

  assert_eq!(item_name, "©nam");

  let Ok(unprintable) = FourCharacterCode([0, 0xff, b'a', 0x7f]).to_string_lossy();

  assert_eq!(unprintable, "??a?");
}

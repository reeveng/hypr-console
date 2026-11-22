//! The element encodings pinned with values taken from the reader as it stood before it was held to the rules.

use console_core_extensible_binary_meta_language::{
  Children, Condition, Extent, Size, VintBits, float, int, read_id, read_size, read_svint, read_vint,
  string, uint, vint_len, write_id, write_size, write_unknown_size,
};
use console_core_never::Never;

fn sized(size: u64, min_len: u32) -> Result<Vec<u8>, Never> {
  let mut out = Vec::new();
  let Ok(()) = write_size(&mut out, size, min_len);

  Ok(out)
}

fn identified(id: u32) -> Result<Vec<u8>, Never> {
  let mut out = Vec::new();
  let Ok(()) = write_id(&mut out, id);

  Ok(out)
}

fn unknown(len: u32) -> Result<Vec<u8>, Never> {
  let mut out = Vec::new();
  let Ok(()) = write_unknown_size(&mut out, len);

  Ok(out)
}

#[test]
fn a_width_is_the_leading_zeros_of_the_first_byte() {
  assert_eq!(vint_len(0x00), Ok(None));
  assert_eq!(vint_len(0x01), Ok(Some(8)));
  assert_eq!(vint_len(0x02), Ok(Some(7)));
  assert_eq!(vint_len(0x10), Ok(Some(4)));
  assert_eq!(vint_len(0x40), Ok(Some(2)));
  assert_eq!(vint_len(0x80), Ok(Some(1)));
  assert_eq!(vint_len(0xFF), Ok(Some(1)));
}

#[test]
fn a_vint_of_every_width_loses_its_marker() {
  assert_eq!(read_vint(&[0x81]), Ok(Some((1, 1, VintBits::ContainsZero))));
  assert_eq!(read_vint(&[0x40, 0x7F]), Ok(Some((127, 2, VintBits::ContainsZero))));
  assert_eq!(read_vint(&[0x20, 0x00, 0x00]), Ok(Some((0, 3, VintBits::ContainsZero))));
  assert_eq!(
    read_vint(&[0x10, 0x00, 0x00, 0x01]),
    Ok(Some((1, 4, VintBits::ContainsZero)))
  );
  assert_eq!(read_vint(&[0x08, 0, 0, 0, 2]), Ok(Some((2, 5, VintBits::ContainsZero))));
  assert_eq!(read_vint(&[0x04, 0, 0, 0, 0, 3]), Ok(Some((3, 6, VintBits::ContainsZero))));
  assert_eq!(
    read_vint(&[0x02, 0, 0, 0, 0, 0, 4]),
    Ok(Some((4, 7, VintBits::ContainsZero)))
  );
  assert_eq!(
    read_vint(&[0x01, 0, 0, 0, 0, 0, 0, 5]),
    Ok(Some((5, 8, VintBits::ContainsZero)))
  );
  assert_eq!(read_vint(&[0x81, 0xAA, 0xBB]), Ok(Some((1, 1, VintBits::ContainsZero))));
  assert_eq!(
    read_vint(&[0x01, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE]),
    Ok(Some((0x12_3456_789A_BCDE, 8, VintBits::ContainsZero)))
  );
}

#[test]
fn a_vint_of_all_ones_says_so() {
  assert_eq!(read_vint(&[0xFF]), Ok(Some((127, 1, VintBits::AllOnes))));
  assert_eq!(read_vint(&[0x7F, 0xFF]), Ok(Some((16383, 2, VintBits::AllOnes))));
  assert_eq!(
    read_vint(&[0x3F, 0xFF, 0xFF]),
    Ok(Some((2_097_151, 3, VintBits::AllOnes)))
  );
  assert_eq!(
    read_vint(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
    Ok(Some((0x00FF_FFFF_FFFF_FFFF, 8, VintBits::AllOnes)))
  );
}

#[test]
fn a_vint_that_cannot_be_read_is_nothing() {
  assert_eq!(read_vint(&[]), Ok(None));
  assert_eq!(read_vint(&[0x00, 0x01]), Ok(None));
  assert_eq!(read_vint(&[0x40]), Ok(None));
  assert_eq!(read_vint(&[0x01, 0, 0, 0, 0, 0, 0]), Ok(None));
}

#[test]
fn a_size_is_known_unless_every_bit_is_set() {
  assert_eq!(read_size(&[0x80]), Ok(Some((Size::Known(0), 1))));
  assert_eq!(read_size(&[0xFE]), Ok(Some((Size::Known(126), 1))));
  assert_eq!(read_size(&[0xFF]), Ok(Some((Size::Unknown, 1))));
  assert_eq!(read_size(&[0x40, 0x7F]), Ok(Some((Size::Known(127), 2))));
  assert_eq!(read_size(&[0x7F, 0xFF]), Ok(Some((Size::Unknown, 2))));
  assert_eq!(
    read_size(&[0x01, 0, 0, 0, 0, 0, 0, 5]),
    Ok(Some((Size::Known(5), 8)))
  );
  assert_eq!(read_size(&[0x00]), Ok(None));
  assert_eq!(read_size(&[0x40]), Ok(None));
  assert_eq!(Size::Known(9).known(), Ok(Some(9)));
  assert_eq!(Size::Unknown.known(), Ok(None));
}

#[test]
fn an_id_keeps_its_marker_and_has_at_most_four_bytes() {
  assert_eq!(
    read_id(&[0x1A, 0x45, 0xDF, 0xA3, 0x99]),
    Ok(Some((0x1A45_DFA3, 4)))
  );
  assert_eq!(read_id(&[0xA3]), Ok(Some((0xA3, 1))));
  assert_eq!(read_id(&[0x80]), Ok(Some((0x80, 1))));
  assert_eq!(read_id(&[0x81, 0x00]), Ok(Some((0x81, 1))));
  assert_eq!(read_id(&[0x42, 0x86, 0x81]), Ok(Some((0x4286, 2))));
  assert_eq!(read_id(&[0x2A, 0xD7, 0xB1]), Ok(Some((0x2A_D7B1, 3))));
  assert_eq!(
    read_id(&[0x1F, 0x43, 0xB6, 0x75]),
    Ok(Some((0x1F43_B675, 4)))
  );
  assert_eq!(read_id(&[]), Ok(None));
  assert_eq!(read_id(&[0x00]), Ok(None));
  assert_eq!(read_id(&[0xFF]), Ok(None));
  assert_eq!(read_id(&[0x7F, 0xFF]), Ok(None));
  assert_eq!(read_id(&[0x40, 0x00]), Ok(None));
  assert_eq!(read_id(&[0x20, 0x00, 0x00]), Ok(None));
  assert_eq!(read_id(&[0x1A, 0x45]), Ok(None));
  assert_eq!(read_id(&[0x08, 0, 0, 0, 1]), Ok(None));
}

#[test]
fn a_signed_vint_is_biased_by_its_width() {
  assert_eq!(read_svint(&[0xBF]), Ok(Some((0, 1))));
  assert_eq!(read_svint(&[0x80]), Ok(Some((-63, 1))));
  assert_eq!(read_svint(&[0xC0]), Ok(Some((1, 1))));
  assert_eq!(read_svint(&[0xFE]), Ok(Some((63, 1))));
  assert_eq!(read_svint(&[0x5F, 0xFF]), Ok(Some((0, 2))));
  assert_eq!(read_svint(&[0x60, 0x00]), Ok(Some((1, 2))));
  assert_eq!(read_svint(&[0x40, 0x00]), Ok(Some((-8191, 2))));
  assert_eq!(
    read_svint(&[0x1F, 0xFF, 0xFF, 0xFF]),
    Ok(Some((134_217_728, 4)))
  );
  assert_eq!(read_svint(&[0xFF]), Ok(Some((64, 1))));
  assert_eq!(
    read_svint(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
    Ok(Some((0x0080_0000_0000_0000, 8)))
  );
  assert_eq!(read_svint(&[]), Ok(None));
  assert_eq!(read_svint(&[0x00]), Ok(None));
  assert_eq!(read_svint(&[0x40]), Ok(None));
}

#[test]
fn an_id_is_written_in_the_fewest_bytes() {
  assert_eq!(identified(0), Ok(vec![0x00]));
  assert_eq!(identified(0xA3), Ok(vec![0xA3]));
  assert_eq!(identified(0xFF), Ok(vec![0xFF]));
  assert_eq!(identified(0x100), Ok(vec![0x01, 0x00]));
  assert_eq!(identified(0x4286), Ok(vec![0x42, 0x86]));
  assert_eq!(identified(0xFFFF), Ok(vec![0xFF, 0xFF]));
  assert_eq!(identified(0x1_0000), Ok(vec![0x01, 0x00, 0x00]));
  assert_eq!(identified(0x2A_D7B1), Ok(vec![0x2A, 0xD7, 0xB1]));
  assert_eq!(identified(0x100_0000), Ok(vec![0x01, 0x00, 0x00, 0x00]));
  assert_eq!(identified(0x1F43_B675), Ok(vec![0x1F, 0x43, 0xB6, 0x75]));
}

#[test]
fn a_size_is_written_in_the_fewest_bytes_that_is_not_all_ones() {
  assert_eq!(sized(0, 1), Ok(vec![0x80]));
  assert_eq!(sized(126, 1), Ok(vec![0xFE]));
  assert_eq!(sized(127, 1), Ok(vec![0x40, 0x7F]));
  assert_eq!(sized(16382, 1), Ok(vec![0x7F, 0xFE]));
  assert_eq!(sized(16383, 1), Ok(vec![0x20, 0x3F, 0xFF]));
  assert_eq!(sized(1_048_576, 1), Ok(vec![0x30, 0x00, 0x00]));
  assert_eq!(
    sized(0x00FF_FFFF_FFFF_FFFE, 1),
    Ok(vec![0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE])
  );
  assert_eq!(
    sized(0x00FF_FFFF_FFFF_FFFF, 1),
    Ok(vec![0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
  );
  assert_eq!(sized(5, 8), Ok(vec![1, 0, 0, 0, 0, 0, 0, 5]));
  assert_eq!(sized(5, 3), Ok(vec![0x20, 0x00, 0x05]));
  assert_eq!(sized(5, 0), Ok(vec![0x85]));
  assert_eq!(sized(5, 9), Ok(vec![1, 0, 0, 0, 0, 0, 0, 5]));
  assert_eq!(sized(200, 2), Ok(vec![0x40, 0xC8]));
}

#[test]
fn an_unknown_size_is_written_at_every_width() {
  assert_eq!(unknown(1), Ok(vec![0xFF]));
  assert_eq!(unknown(2), Ok(vec![0x7F, 0xFF]));
  assert_eq!(unknown(3), Ok(vec![0x3F, 0xFF, 0xFF]));
  assert_eq!(
    unknown(8),
    Ok(vec![0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
  );

  for width in 1..=8u32 {
    let Ok(bytes) = unknown(width);

    assert_eq!(read_size(&bytes), Ok(Some((Size::Unknown, width))));
  }
}

#[test]
fn written_sizes_and_ids_read_back() {
  for value in [
    0u64,
    1,
    126,
    127,
    128,
    16382,
    16383,
    16384,
    1_048_576,
    0x00FF_FFFF_FFFF_FFFE,
  ] {
    let Ok(bytes) = sized(value, 1);
    let Ok(width) = console_core_number_conversion::fitted::<_, u32>(bytes.len());

    assert_eq!(read_size(&bytes), Ok(Some((Size::Known(value), width))));
  }

  for id in [0xA3u32, 0x4286, 0x2A_D7B1, 0x1F43_B675] {
    let Ok(bytes) = identified(id);
    let Ok(width) = console_core_number_conversion::fitted::<_, u32>(bytes.len());

    assert_eq!(read_id(&bytes), Ok(Some((id, width))));
  }
}

#[test]
fn an_unsigned_value_takes_at_most_eight_bytes() {
  assert_eq!(uint(&[]), Ok(0));
  assert_eq!(uint(&[0x07]), Ok(7));
  assert_eq!(uint(&[1, 0]), Ok(256));
  assert_eq!(uint(&[0x12, 0x34, 0x56]), Ok(0x12_3456));
  assert_eq!(uint(&[0xFF; 8]), Ok(u64::MAX));
  assert_eq!(uint(&[0, 0, 0, 0, 0, 0, 0, 1, 0xFF]), Ok(1));
}

#[test]
fn a_signed_value_extends_its_sign() {
  assert_eq!(int(&[]), Ok(0));
  assert_eq!(int(&[0x7F]), Ok(127));
  assert_eq!(int(&[0x80]), Ok(-128));
  assert_eq!(int(&[0xFF]), Ok(-1));
  assert_eq!(int(&[0xFF, 0xFE]), Ok(-2));
  assert_eq!(int(&[0x80, 0x00]), Ok(-32768));
  assert_eq!(int(&[0x00, 0xFF]), Ok(255));
  assert_eq!(int(&[0x80, 0, 0, 0, 0, 0, 0, 0]), Ok(i64::MIN));
  assert_eq!(
    int(&[0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
    Ok(i64::MAX)
  );
  assert_eq!(
    int(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0x55]),
    Ok(-2)
  );
}

#[test]
fn a_float_is_four_or_eight_bytes_and_otherwise_zero() {
  assert_eq!(float(&[]), Ok(0.0));
  assert_eq!(float(&[1, 2, 3]), Ok(0.0));
  assert_eq!(float(&[0; 10]), Ok(0.0));
  assert_eq!(float(&1.5f32.to_be_bytes()), Ok(1.5));
  assert_eq!(float(&2.25f64.to_be_bytes()), Ok(2.25));
  assert_eq!(float(&(-48000.0f64).to_be_bytes()), Ok(-48000.0));
  assert_eq!(float(&0.1f32.to_be_bytes()), Ok(f64::from(0.1f32)));
  let Ok(nan) = float(&f32::NAN.to_be_bytes());
  assert!(nan.is_nan());
}

#[test]
fn a_string_stops_at_the_first_nul_and_repairs_what_is_not_utf8() {
  assert_eq!(string(b"").as_deref(), Ok(""));
  assert_eq!(string(b"abc").as_deref(), Ok("abc"));
  assert_eq!(string(b"abc\0\0").as_deref(), Ok("abc"));
  assert_eq!(string(b"\0abc").as_deref(), Ok(""));
  assert_eq!(string(b"ab\0cd").as_deref(), Ok("ab"));
  assert_eq!(string("h\u{e9}llo".as_bytes()).as_deref(), Ok("h\u{e9}llo"));
  assert_eq!(string(&[b'a', 0xFF, b'b']).as_deref(), Ok("a\u{FFFD}b"));
}

type Summary = (u32, u64, u32, Vec<u8>, Extent);

fn walked(data: &[u8]) -> Result<(Vec<Summary>, Condition), Never> {
  let mut walk = Children::from(data);
  let found: Vec<Summary> = walk
    .by_ref()
    .map(|element| {
      (
        element.id,
        element.offset,
        element.header_len,
        element.data.to_vec(),
        element.extent,
      )
    })
    .collect();

  let Ok(condition) = walk.condition();

  Ok((found, condition))
}

#[test]
fn children_are_walked_with_their_places() {
  let data = [0x42, 0x86, 0x81, 0x01, 0xEC, 0x82, 0, 0, 0xA3, 0x85, 1, 2];

  assert_eq!(
    walked(&data),
    Ok((
      vec![
        (0x4286, 0, 3, vec![1], Extent::Declared),
        (0xEC, 4, 2, vec![0, 0], Extent::Declared),
        (0xA3, 8, 2, vec![1, 2], Extent::Truncated),
      ],
      Condition::Intact
    ))
  );
}

#[test]
fn an_unknown_size_runs_to_the_end_of_the_parent() {
  assert_eq!(
    walked(&[0xA3, 0xFF, 1, 2, 3]),
    Ok((
      vec![(0xA3, 0, 2, vec![1, 2, 3], Extent::Unknown)],
      Condition::Intact
    ))
  );
}

#[test]
fn an_empty_element_at_the_end_has_no_data() {
  assert_eq!(
    walked(&[0xEC, 0x80]),
    Ok((
      vec![(0xEC, 0, 2, vec![], Extent::Declared)],
      Condition::Intact
    ))
  );
}

#[test]
fn a_header_that_cannot_be_read_ends_the_walk_damaged() {
  assert_eq!(walked(&[0x00, 0x01]), Ok((vec![], Condition::Damaged)));
  assert_eq!(walked(&[0xA3, 0x40]), Ok((vec![], Condition::Damaged)));
  assert_eq!(walked(&[0xA3]), Ok((vec![], Condition::Damaged)));
  assert_eq!(walked(&[]), Ok((vec![], Condition::Intact)));
  assert_eq!(
    walked(&[0xEC, 0x81, 9, 0x00]),
    Ok((
      vec![(0xEC, 0, 2, vec![9], Extent::Declared)],
      Condition::Damaged
    ))
  );
}

#[test]
fn a_walk_that_ended_stays_ended() {
  let bytes = [0x00u8, 0x01];
  let mut walk = Children::from(bytes.as_slice());

  assert!(walk.next().is_none());
  assert!(walk.next().is_none());
  assert_eq!(walk.condition(), Ok(Condition::Damaged));
}

#[test]
fn an_all_ones_vint_is_unknown_only_as_a_size() {
  for width in 1..=8u32 {
    let Ok(bytes) = unknown(width);
    let bits = width.saturating_mul(7);
    let value = 1u64.wrapping_shl(bits).wrapping_sub(1);

    assert_eq!(read_vint(&bytes), Ok(Some((value, width, VintBits::AllOnes))));
    assert_eq!(read_size(&bytes), Ok(Some((Size::Unknown, width))));
    let Ok(bias_plus_one) = console_core_number_conversion::fitted::<u64, i64>(
      1u64.wrapping_shl(bits.saturating_sub(1)),
    );

    assert_eq!(read_svint(&bytes), Ok(Some((bias_plus_one, width))));
  }
}

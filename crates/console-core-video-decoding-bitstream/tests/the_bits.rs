use console_core_never::Never;
use console_core_video_decoding_bitstream::{
  BitError, BitReader, BitWriter, Field, FlagBit, Remaining, annexb_nals, length_prefixed_nals,
  unescape_rbsp,
};

type Failure = Box<dyn std::error::Error>;

const UNSIGNED: [u32; 8] = [0, 1, 2, 3, 4, 255, 65_535, u32::MAX];

const SIGNED: [i32; 9] = [0, 1, -1, 2, -2, 1000, -1000, i32::MAX, -i32::MAX];

fn written(fields: &[Field]) -> Result<Vec<u8>, Never> {
  let Ok(mut writer) = BitWriter::new();

  for field in fields {
    let Ok(()) = writer.write_bits(*field);
  }

  writer.finish()
}

fn stirred(seed: u32) -> Result<u32, Never> {
  Ok(seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223))
}

#[test]
fn a_field_is_read_from_its_highest_bit_and_none_past_the_end() -> Result<(), Failure> {
  let bytes = [0b1010_1100, 0b0101_0011];
  let Ok(mut reader) = BitReader::new(&bytes);

  let first = reader.read_bits(1)?;
  let second = reader.read_bits(3)?;
  let third = reader.read_bits(8)?;

  assert_eq!((first, second, third), (1, 0b010, 0b1100_0101));
  assert_eq!(reader.bits_left(), Ok(4));
  assert_eq!(reader.read_bits(5), Err(BitError::PastTheEnd));
  assert_eq!(reader.read_bits(4), Ok(0b0011));

  Ok(())
}

#[test]
fn a_field_wider_than_thirty_two_bits_is_refused_rather_than_skipped() -> Result<(), Failure> {
  let bytes = [0xFF; 8];
  let Ok(mut reader) = BitReader::new(&bytes);

  assert_eq!(reader.read_bits(33), Err(BitError::TooWide));
  assert_eq!(reader.position(), Ok(0));
  assert_eq!(reader.read_bits_u64(36), Ok(0xF_FFFF_FFFF));
  assert_eq!(reader.position(), Ok(36));
  assert_eq!(reader.read_bits_u64(29), Err(BitError::PastTheEnd));
  assert_eq!(reader.position(), Ok(36));

  Ok(())
}

#[test]
fn a_look_ahead_past_the_end_is_handed_zeros_and_moves_nothing() -> Result<(), Failure> {
  let bytes = [0b1011_0000];
  let Ok(mut reader) = BitReader::new(&bytes);

  reader.skip(2)?;

  assert_eq!(reader.peek(8), Ok(0b1100_0000));
  assert_eq!(reader.peek(0), Ok(0));
  assert_eq!(reader.position(), Ok(2));
  assert_eq!(reader.skip(7), Err(BitError::PastTheEnd));

  Ok(())
}

#[test]
fn every_code_written_is_the_code_read() -> Result<(), Failure> {
  let Ok(mut writer) = BitWriter::new();

  for number in UNSIGNED {
    let Ok(()) = writer.write_ue(number);
  }

  for number in SIGNED {
    let Ok(()) = writer.write_se(number);
  }

  let Ok(bytes) = writer.finish();
  let Ok(mut reader) = BitReader::new(&bytes);

  for number in UNSIGNED {
    assert_eq!(reader.read_ue(), Ok(number));
  }

  for number in SIGNED {
    assert_eq!(reader.read_se(), Ok(number));
  }

  Ok(())
}

#[test]
fn the_first_codes_are_the_ones_the_standard_spells() -> Result<(), Failure> {
  let Ok(bytes) = written(&[
    Field {
      value: 0b1010_0110,
      width: 8,
    },
    Field {
      value: 0b0_1000_0101,
      width: 9,
    },
  ]);
  let Ok(mut reader) = BitReader::new(&bytes);

  for number in 0..5 {
    assert_eq!(reader.read_ue(), Ok(number));
  }

  Ok(())
}

#[test]
fn a_code_longer_than_a_number_is_said_rather_than_read() -> Result<(), Failure> {
  let Ok(zeros) = written(&[
    Field {
      value: 0,
      width: 32,
    },
    Field { value: 0, width: 1 },
    Field { value: 1, width: 1 },
  ]);
  let Ok(mut too_many_zeros) = BitReader::new(&zeros);

  assert_eq!(too_many_zeros.read_ue(), Err(BitError::TooLong));

  let Ok(largest) = written(&[
    Field {
      value: 0,
      width: 32,
    },
    Field { value: 1, width: 1 },
    Field {
      value: 1,
      width: 32,
    },
  ]);
  let Ok(mut past_the_largest) = BitReader::new(&largest);

  assert_eq!(past_the_largest.read_ue(), Err(BitError::TooLong));

  let Ok(mut writer) = BitWriter::new();
  let Ok(()) = writer.write_ue(u32::MAX);
  let Ok(beyond) = writer.finish();
  let Ok(mut past_a_signed_number) = BitReader::new(&beyond);

  assert_eq!(past_a_signed_number.read_se(), Err(BitError::TooLong));

  Ok(())
}

#[test]
fn fields_of_every_width_come_back_as_they_went_in() -> Result<(), Failure> {
  let mut fields = Vec::new();
  let mut seed = 7u32;

  for _round in 0..12 {
    for width in 1..=32 {
      let Ok(next) = stirred(seed);

      seed = next;

      fields.push(Field { value: seed, width });
    }
  }

  let Ok(bytes) = written(&fields);
  let Ok(mut reader) = BitReader::new(&bytes);

  for field in &fields {
    let wanted = match 1u64.checked_shl(field.width) {
      Some(past) => u64::from(field.value) & past.wrapping_sub(1),
      None => u64::from(field.value),
    };

    let read = reader.read_bits(field.width)?;

    assert_eq!(u64::from(read), wanted);
  }

  Ok(())
}

#[test]
fn the_byte_an_encoder_put_in_is_taken_back_out() -> Result<(), Failure> {
  let escaped = [0, 0, 3, 1, 0, 0, 3, 0, 5, 0, 0, 3, 3];

  assert_eq!(
    unescape_rbsp(&escaped),
    Ok(vec![0, 0, 1, 0, 0, 0, 5, 0, 0, 3])
  );
  assert_eq!(unescape_rbsp(&[0, 3, 0, 3]), Ok(vec![0, 3, 0, 3]));

  Ok(())
}

#[test]
fn a_raw_stream_is_cut_at_its_start_codes_and_not_at_the_zeros_before_them() -> Result<(), Failure>
{
  let stream = [
    0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x68, 3, 0, 0, 0, 1, 0x65, 9, 9,
  ];
  let one: &[u8] = &[0x67, 1, 2];
  let two: &[u8] = &[0x68, 3];
  let three: &[u8] = &[0x65, 9, 9];
  let last: &[u8] = &[7];

  assert_eq!(annexb_nals(&stream), Ok(vec![one, two, three]));
  assert_eq!(annexb_nals(&[9, 9, 0, 0]), Ok(Vec::new()));
  assert_eq!(annexb_nals(&[0, 0, 1, 0, 0, 0, 1, 7]), Ok(vec![last]));

  Ok(())
}

#[test]
fn a_sample_is_cut_where_each_length_says_and_a_short_one_is_said() -> Result<(), Failure> {
  let sample = [0, 0, 0, 2, 0x67, 1, 0, 0, 0, 1, 0x68];
  let one: &[u8] = &[0x67, 1];
  let two: &[u8] = &[0x68];
  let first: &[u8] = &[0x41, 0x42];
  let second: &[u8] = &[0x43];

  assert_eq!(length_prefixed_nals(&sample, 4), Ok(vec![one, two]));
  assert_eq!(
    length_prefixed_nals(&[2, 0x41, 0x42, 1, 0x43], 1),
    Ok(vec![first, second])
  );
  assert_eq!(
    length_prefixed_nals(&[0, 0, 0, 3, 1, 2], 4),
    Err(BitError::PastTheEnd)
  );
  assert_eq!(length_prefixed_nals(&[0, 0], 4), Err(BitError::PastTheEnd));
  assert_eq!(length_prefixed_nals(&sample, 0), Err(BitError::Unmeasured));
  assert_eq!(length_prefixed_nals(&[], 4), Ok(Vec::new()));

  Ok(())
}

#[test]
fn what_is_left_before_the_stop_bit_is_payload_and_the_rest_is_not() -> Result<(), Failure> {
  let bytes = [0b1100_0000];
  let Ok(mut reader) = BitReader::new(&bytes);

  assert_eq!(reader.more_rbsp_data(), Ok(Remaining::Payload));
  assert_eq!(reader.read_flag(), Ok(FlagBit::One));
  assert_eq!(reader.more_rbsp_data(), Ok(Remaining::TrailingBits));
  assert_eq!(reader.read_flag(), Ok(FlagBit::One));
  assert_eq!(reader.read_flag(), Ok(FlagBit::Zero));

  let Ok(empty) = BitReader::new(&[0, 0]);

  assert_eq!(empty.more_rbsp_data(), Ok(Remaining::TrailingBits));

  Ok(())
}

#[test]
fn a_position_moves_to_the_next_byte_and_can_be_set_again() -> Result<(), Failure> {
  let bytes = [0xAB, 0xCD];
  let Ok(mut reader) = BitReader::new(&bytes);

  reader.skip(3)?;

  let Ok(()) = reader.byte_align();

  assert_eq!(reader.byte_pos(), Ok(1));
  assert_eq!(reader.read_bits(8), Ok(0xCD));

  let Ok(()) = reader.seek_bits(4);

  assert_eq!(reader.read_bits(4), Ok(0xB));
  assert_eq!(reader.data(), Ok(bytes.as_slice()));

  Ok(())
}

//! Blocks built by hand are split into the frames they hold, and damaged ones are refused whole.

use std::error::Error;

use console_core_matroska_blocks::{BlockHeader, Frame, Lacing, LacingError, parse_lacing};
use console_core_never::Never;
use console_core_number_conversion::index;

fn lengths(frames: &[Frame]) -> Result<Vec<u64>, Never> {
  Ok(frames.iter().map(|frame| frame.len).collect())
}

fn frame_bytes<'a>(data: &'a [u8], frame: &Frame) -> Result<Option<&'a [u8]>, Never> {
  let Ok(start) = index(frame.offset);
  let Ok(len) = index(frame.len);

  Ok(start.checked_add(len).and_then(|end| data.get(start..end)))
}

fn held<'a>(data: &'a [u8], frames: &[Frame]) -> Result<Vec<&'a [u8]>, Never> {
  Ok(
    frames
      .iter()
      .filter_map(|frame| {
        let Ok(found) = frame_bytes(data, frame);

        found
      })
      .collect(),
  )
}

#[test]
fn xiph_lacing_reads_runs_of_255_and_the_terminator() -> Result<(), Box<dyn Error>> {
  let mut data = vec![2u8, 255, 45, 2];

  data.extend(std::iter::repeat_n(7u8, 300));
  data.extend_from_slice(&[1, 2]);
  data.extend_from_slice(&[3, 4, 5]);

  let frames = parse_lacing(Lacing::Xiph, &data)?;
  let Ok(pieces) = held(&data, &frames);

  assert_eq!(lengths(&frames), Ok(vec![300, 2, 3]));
  assert_eq!(pieces.last().copied(), Some([3u8, 4, 5].as_slice()));

  let mut data_2 = vec![1u8, 255, 0];

  data_2.extend(std::iter::repeat_n(1u8, 255));
  data_2.push(9);

  let full_run_frames = parse_lacing(Lacing::Xiph, &data_2)?;

  assert_eq!(lengths(&full_run_frames), Ok(vec![255, 1]));
  assert_eq!(
    parse_lacing(Lacing::Xiph, &[1, 200, 1]),
    Err(LacingError::SizesExceedBlock)
  );
  assert_eq!(
    parse_lacing(Lacing::Xiph, &[1, 255]),
    Err(LacingError::TruncatedXiph)
  );

  Ok(())
}

#[test]
fn ebml_lacing_reads_a_size_and_signed_deltas() -> Result<(), Box<dyn Error>> {
  let mut data = vec![3u8, 0x85, 0xBD, 0xC0];

  data.extend_from_slice(&[1; 5]);
  data.extend_from_slice(&[2; 3]);
  data.extend_from_slice(&[3; 4]);
  data.extend_from_slice(&[4; 2]);

  let frames = parse_lacing(Lacing::Ebml, &data)?;
  let Ok(pieces) = held(&data, &frames);

  assert_eq!(lengths(&frames), Ok(vec![5, 3, 4, 2]));
  assert_eq!(
    pieces,
    [
      [1u8; 5].as_slice(),
      [2u8; 3].as_slice(),
      [3u8; 4].as_slice(),
      [4u8; 2].as_slice()
    ]
  );
  assert_eq!(
    parse_lacing(Lacing::Ebml, &[1, 0x90]),
    Err(LacingError::SizesExceedBlock)
  );
  assert_eq!(
    parse_lacing(Lacing::Ebml, &[2, 0x80, 0x80]),
    Err(LacingError::NegativeEbmlSize)
  );

  Ok(())
}

#[test]
fn fixed_lacing_divides_the_block_evenly() -> Result<(), Box<dyn Error>> {
  let data = [2u8, 1, 1, 2, 2, 3, 3];
  let frames = parse_lacing(Lacing::FixedSize, &data)?;
  let Ok(pieces) = held(&data, &frames);
  let whole = parse_lacing(Lacing::None, &[9, 9])?;

  assert_eq!(
    pieces,
    [
      [1u8, 1].as_slice(),
      [2u8, 2].as_slice(),
      [3u8, 3].as_slice()
    ]
  );
  assert_eq!(
    parse_lacing(Lacing::FixedSize, &[2, 1, 1]),
    Err(LacingError::FixedSizeNotDividing)
  );
  assert_eq!(whole, [Frame { offset: 0, len: 2 }]);
  assert_eq!(Lacing::from_flags(0x80 | 0x06), Ok(Lacing::Ebml));
  assert_eq!(Lacing::from_flags(0x02), Ok(Lacing::Xiph));
  assert_eq!(Lacing::from_flags(0x04), Ok(Lacing::FixedSize));

  Ok(())
}

#[test]
fn a_header_leaves_payload_and_timestamp_policy_to_the_caller() -> Result<(), Box<dyn Error>> {
  let bytes = [
    0x40, 0x80, 0xff, 0xfb, 0x86, 2, 0x82, 0xbe, 1, 2, 3, 4, 5, 6,
  ];
  let Ok(parsed) = BlockHeader::parse(&bytes);
  let header = parsed.ok_or("a header")?;
  let Ok(start) = index(header.header_len);
  let Ok(lacing) = Lacing::from_flags(header.flags);
  let payload = bytes.get(start..).ok_or("a payload")?;
  let frames = parse_lacing(lacing, payload)?;

  assert_eq!(header.track, 128);
  assert_eq!(header.relative_timestamp, -5);
  assert_eq!(header.flags, 0x86);
  assert_eq!(header.header_len, 5);
  assert_eq!(
    frames,
    [
      Frame { offset: 3, len: 2 },
      Frame { offset: 5, len: 1 },
      Frame { offset: 6, len: 3 }
    ]
  );

  for end in 0..start {
    let prefix = bytes.get(..end).ok_or("a prefix")?;

    assert_eq!(BlockHeader::parse(prefix), Ok(None));
  }

  assert_eq!(BlockHeader::parse(&[0, 0, 0, 0]), Ok(None));

  Ok(())
}

#[test]
fn an_empty_lace_is_refused() {
  for mode in [Lacing::Xiph, Lacing::Ebml, Lacing::FixedSize] {
    assert_eq!(parse_lacing(mode, &[]), Err(LacingError::MissingFrameCount));
  }
}

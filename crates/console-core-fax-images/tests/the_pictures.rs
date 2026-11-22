use console_core_fax_images::{
  Color, DecodeError, DecodeSettings, Decoder, DecoderContext, EncodingMode, EndOfBlock,
  RowAlignment, decode,
};
use console_core_never::Never;

type Failure = Box<dyn std::error::Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Made {
  Pixel(Color),
  Eight(Color, u32),
  Line,
}

#[derive(Debug, Default)]
struct Picture {
  made: Vec<Made>,
}

impl Decoder for Picture {
  fn push_pixel(&mut self, color: Color) -> Result<(), Never> {
    self.made.push(Made::Pixel(color));

    Ok(())
  }

  fn push_pixel_chunk(&mut self, color: Color, chunk_count: u32) -> Result<(), Never> {
    self.made.push(Made::Eight(color, chunk_count));

    Ok(())
  }

  fn next_line(&mut self) -> Result<(), Never> {
    self.made.push(Made::Line);

    Ok(())
  }
}

fn plain(encoding: EncodingMode) -> Result<DecodeSettings, Never> {
  Ok(DecodeSettings {
    columns: 0,
    rows: 0,
    end_of_block: EndOfBlock::Expected,
    row_alignment: RowAlignment::Bit,
    encoding,
  })
}

fn decoded(
  data: &[u8],
  settings: DecodeSettings,
) -> Result<(Result<u64, DecodeError>, Vec<Made>), Never> {
  let mut picture = Picture::default();
  let Ok(mut context) = DecoderContext::new(settings);
  let read = decode(data, &mut picture, &mut context);

  Ok((read, picture.made))
}

const W: Color = Color::White;

const B: Color = Color::Black;

#[test]
fn a_white_page_in_group_4_is_one_bit_a_line_and_a_byte_at_a_time() -> Result<(), Failure> {
  let Ok(page) = plain(EncodingMode::Group4);
  let sixteen = DecodeSettings {
    columns: 16,
    rows: 3,
    ..page
  };
  let Ok((read, made)) = decoded(&[0b1110_0000], sixteen);

  assert_eq!(read, Ok(1));
  assert_eq!(
    made,
    vec![
      Made::Eight(W, 2),
      Made::Line,
      Made::Eight(W, 2),
      Made::Line,
      Made::Eight(W, 2),
      Made::Line
    ]
  );

  Ok(())
}

#[test]
fn a_line_of_runs_is_white_first_and_turns_at_every_run() -> Result<(), Failure> {
  let Ok(page) = plain(EncodingMode::Group3OneDimensional);
  let eight = DecodeSettings {
    columns: 8,
    rows: 1,
    ..page
  };
  let Ok((read, made)) = decoded(&[0b1000_1110, 0b0000_0000], eight);

  assert_eq!(read, Ok(2));
  assert_eq!(
    made,
    vec![
      Made::Pixel(W),
      Made::Pixel(W),
      Made::Pixel(W),
      Made::Pixel(B),
      Made::Pixel(B),
      Made::Pixel(W),
      Made::Pixel(W),
      Made::Pixel(W),
      Made::Line,
    ]
  );

  Ok(())
}

#[test]
fn a_row_that_starts_on_a_byte_skips_what_is_left_of_the_one_before() -> Result<(), Failure> {
  let Ok(page) = plain(EncodingMode::Group3OneDimensional);
  let eight = DecodeSettings {
    columns: 8,
    rows: 2,
    ..page
  };
  let aligned = DecodeSettings {
    row_alignment: RowAlignment::Byte,
    ..eight
  };
  let Ok((read, made)) = decoded(&[0b1001_1000, 0b1001_1000], aligned);

  assert_eq!(read, Ok(2));
  assert_eq!(
    made,
    vec![Made::Eight(W, 1), Made::Line, Made::Eight(W, 1), Made::Line]
  );

  Ok(())
}

#[test]
fn a_run_longer_than_its_line_is_cut_at_the_end_of_it() -> Result<(), Failure> {
  let Ok(page) = plain(EncodingMode::Group3OneDimensional);
  let four = DecodeSettings {
    columns: 4,
    rows: 1,
    ..page
  };
  let Ok((read, made)) = decoded(&[0b1001_1000], four);

  assert_eq!(read, Ok(1));
  assert_eq!(
    made,
    vec![
      Made::Pixel(W),
      Made::Pixel(W),
      Made::Pixel(W),
      Made::Pixel(W),
      Made::Line
    ]
  );

  Ok(())
}

#[test]
fn the_end_of_block_marker_ends_a_picture_before_its_rows() -> Result<(), Failure> {
  let Ok(page) = plain(EncodingMode::Group4);
  let eight = DecodeSettings {
    columns: 8,
    rows: 100,
    ..page
  };
  let Ok((read, made)) = decoded(&[0b1000_0000, 0b0000_1000, 0b0000_0000, 0b1000_0000], eight);

  assert_eq!(read, Ok(4));
  assert_eq!(made, vec![Made::Eight(W, 1), Made::Line]);

  Ok(())
}

#[test]
fn a_picture_no_columns_wide_ends_rather_than_going_round_for_ever() -> Result<(), Failure> {
  let Ok(page) = plain(EncodingMode::Group3OneDimensional);
  let nothing = DecodeSettings {
    columns: 0,
    rows: 0,
    ..page
  };
  let Ok((read, made)) = decoded(&[0xAA; 4], nothing);

  assert_eq!(read, Ok(0));
  assert_eq!(made, vec![Made::Line; 33]);

  Ok(())
}

#[test]
fn bits_that_are_no_code_are_said_and_what_came_before_them_is_kept() -> Result<(), Failure> {
  let Ok(page) = plain(EncodingMode::Group4);
  let eight = DecodeSettings {
    columns: 8,
    rows: 2,
    ..page
  };
  let Ok((read, made)) = decoded(&[0b1000_0000, 0b0000_0000], eight);

  assert_eq!(read, Err(DecodeError::InvalidCode));
  assert_eq!(made, vec![Made::Eight(W, 1), Made::Line]);

  Ok(())
}

#[test]
fn a_context_decodes_a_second_picture_from_nothing() -> Result<(), Failure> {
  let Ok(page) = plain(EncodingMode::Group4);
  let sixteen = DecodeSettings {
    columns: 16,
    rows: 2,
    ..page
  };
  let Ok(mut context) = DecoderContext::new(sixteen);
  let mut first = Picture::default();
  let mut second = Picture::default();
  let once = decode(&[0b1100_0000], &mut first, &mut context);
  let twice = decode(&[0b1100_0000], &mut second, &mut context);

  assert_eq!(once, twice);
  assert_eq!(first.made, second.made);

  Ok(())
}

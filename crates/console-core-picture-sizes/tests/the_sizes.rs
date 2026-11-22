use console_core_never::Never;
use console_core_picture_sizes::{PictureFormat, SizeError, format, size};
use console_core_size::Size;

fn a_png(drawn: Size<u32>) -> Result<Vec<u8>, Never> {
  let mut head = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();

  head.extend(drawn.width.to_be_bytes());
  head.extend(drawn.height.to_be_bytes());

  Ok(head)
}

fn a_jpeg(marker: u8) -> Result<Vec<u8>, Never> {
  let mut head = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00];

  head.extend([0xFF, marker, 0x00, 0x11, 0x08]);
  head.extend(900u16.to_be_bytes());
  head.extend(600u16.to_be_bytes());
  head.extend([0x03, 0x01, 0x11]);

  Ok(head)
}

fn a_webp(chunk: &[u8], body: &[u8]) -> Result<Vec<u8>, Never> {
  let mut head = b"RIFF\0\0\0\0WEBP".to_vec();

  head.extend_from_slice(chunk);
  head.extend_from_slice(body);

  Ok(head)
}

const TALL: Size<u32> = Size {
  width: 600,
  height: 900,
};

#[test]
fn a_png_says_how_big_it_is_in_its_header() {
  let Ok(square) = a_png(Size {
    width: 256,
    height: 256,
  });
  let Ok(tall) = a_png(TALL);

  assert_eq!(format(&square), Ok(PictureFormat::Png));
  assert_eq!(
    size(&square),
    Ok(Size {
      width: 256,
      height: 256
    })
  );
  assert_eq!(size(&tall), Ok(TALL));
}

#[test]
fn a_jpeg_says_so_after_whatever_else_it_carries() {
  let Ok(tall) = a_jpeg(0xC0);

  assert_eq!(format(&tall), Ok(PictureFormat::Jpeg));
  assert_eq!(size(&tall), Ok(TALL));
}

#[test]
fn every_marker_that_starts_a_frame_is_read_and_a_table_is_not() {
  let frames = (0xC0..=0xCF).filter(|marker| !matches!(marker, 0xC4 | 0xC8 | 0xCC));

  for marker in frames {
    let Ok(tall) = a_jpeg(marker);

    assert_eq!(size(&tall), Ok(TALL), "{marker:02X}");
  }

  for table in [0xC4, 0xC8, 0xCC] {
    let Ok(tall) = a_jpeg(table);

    assert_eq!(size(&tall), Err(SizeError::Truncated), "{table:02X}");
  }
}

#[test]
fn a_thumbnail_inside_a_segment_is_stepped_over_with_it() {
  let mut head = vec![
    0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x0B, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x10, 0x00, 0x10,
  ];
  let Ok(picture) = a_jpeg(0xC2);

  head.extend(picture.get(2..).into_iter().flatten());

  assert_eq!(size(&head), Ok(TALL));
}

#[test]
fn stray_bytes_and_fill_before_a_marker_are_stepped_over() {
  let Ok(tall) = a_jpeg(0xC0);
  let mut strayed = tall
    .get(..8)
    .into_iter()
    .flatten()
    .copied()
    .collect::<Vec<u8>>();

  strayed.extend([0x00, 0x00, 0xFF, 0xFF, 0xFF]);
  strayed.extend(tall.get(9..).into_iter().flatten());

  assert_eq!(size(&strayed), Ok(TALL));
}

#[test]
fn a_scan_or_an_end_before_any_frame_is_no_size() {
  assert_eq!(size(&[0xFF, 0xD8, 0xFF, 0xD9]), Err(SizeError::Corrupted));
  assert_eq!(
    size(&[0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x08]),
    Err(SizeError::Corrupted)
  );
  assert_eq!(
    size(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x01]),
    Err(SizeError::Corrupted)
  );
}

#[test]
fn a_gif_says_its_screen() {
  assert_eq!(size(b"GIF89a\x58\x02\x84\x03"), Ok(TALL));
  assert_eq!(format(b"GIF87a"), Ok(PictureFormat::Gif));
}

#[test]
fn a_webp_says_its_size_in_whichever_of_its_three_ways_it_is_written() {
  let Ok(extended) = a_webp(b"VP8X", b"\0\0\0\0\0\0\0\0\x57\x02\x00\x83\x03\x00");
  let packed = 599u32 | 899u32.wrapping_shl(14);
  let Ok(lossless) = a_webp(
    b"VP8L",
    &[
      [0, 0, 0, 0, 0x2F].as_slice(),
      packed.to_le_bytes().as_slice(),
    ]
    .concat(),
  );
  let Ok(lossy) = a_webp(b"VP8 ", b"\0\0\0\0\0\0\0\x9d\x01\x2a\x58\x42\x84\x83");

  assert_eq!(format(&extended), Ok(PictureFormat::Webp));
  assert_eq!(size(&extended), Ok(TALL));
  assert_eq!(size(&lossless), Ok(TALL));
  assert_eq!(size(&lossy), Ok(TALL));
}

#[test]
fn anything_else_says_why_rather_than_guessing() {
  assert_eq!(size(b"<svg></svg>"), Err(SizeError::Unrecognised));
  assert_eq!(size(b""), Err(SizeError::Unrecognised));
  assert_eq!(
    size(b"\x89PNG\r\n\x00\n\0\0\0\x0dIHDR"),
    Err(SizeError::Unrecognised)
  );

  let Ok(mut dot) = a_png(Size {
    width: 1,
    height: 1,
  });

  dot.truncate(12);

  assert_eq!(size(&dot), Err(SizeError::Truncated), "cut short");
  assert_eq!(
    size(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x00]),
    Err(SizeError::Truncated)
  );
}

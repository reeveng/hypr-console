use std::error::Error;

use console_core_geometry::Size;
use console_core_gif_files::{GifError, decoded, measured};

const PICTURE: Size<u32> = Size {
  width: 23,
  height: 17,
};

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the paired GIF fixture and its expected RGBA share a stem so the test cannot accidentally compare two separately named pictures"
  )
)]
macro_rules! fixture {
  ($name:literal) => {
    (
      $name,
      include_bytes!(concat!("pictures/", $name, ".gif")).as_slice(),
      include_bytes!(concat!("pictures/", $name, ".rgba")).as_slice(),
    )
  };
}

const EVERY_KIND: [(&str, &[u8], &[u8]); 9] = [
  fixture!("global"),
  fixture!("two-colours"),
  fixture!("interlaced"),
  fixture!("local"),
  fixture!("local-only"),
  fixture!("see-through"),
  fixture!("placed"),
  fixture!("87a"),
  fixture!("two-frames"),
];

#[test]
fn every_kind_of_frame_comes_back_as_giflib_draws_it() -> Result<(), Box<dyn Error>> {
  for (name, gif, rgba) in EVERY_KIND {
    let picture = decoded(gif)?;

    assert_eq!(picture.size, PICTURE, "{name}");
    assert!(picture.bytes == rgba, "{name} is not what giflib draws");
  }

  Ok(())
}

#[test]
fn a_table_that_fills_comes_back_whether_it_is_cleared_or_kept() -> Result<(), Box<dyn Error>> {
  for (name, gif, rgba) in [fixture!("full-table-cleared"), fixture!("full-table-kept")] {
    let picture = decoded(gif)?;

    assert_eq!(
      picture.size,
      Size {
        width: 97,
        height: 61
      },
      "{name}"
    );
    assert!(picture.bytes == rgba, "{name} is not what giflib draws");
  }

  Ok(())
}

#[test]
fn the_size_is_said_from_the_screen_alone() -> Result<(), Box<dyn Error>> {
  let (_, gif, _) = fixture!("placed");
  let screen = gif
    .get(..13)
    .ok_or("the fixture has no complete screen header")?;

  assert_eq!(measured(gif), Ok(PICTURE));
  assert_eq!(measured(screen), Ok(PICTURE));

  Ok(())
}

#[test]
fn a_file_that_is_not_a_gif_says_so() {
  assert_eq!(
    decoded(b"\x89PNG\r\n\x1a\n, which is something else entirely").map(|picture| picture.size),
    Err(GifError::NotAGif)
  );
}

#[test]
fn a_file_cut_short_is_refused_rather_than_drawn_in_part() -> Result<(), Box<dyn Error>> {
  let (_, gif, _) = fixture!("global");

  for cut in [4, 12, 40, 200, gif.len().saturating_sub(20)] {
    let short = gif
      .get(..cut)
      .ok_or("the fixture ends before the truncation point")?;

    assert_eq!(
      decoded(short).map(|picture| picture.size),
      Err(GifError::Truncated),
      "cut at {cut}"
    );
  }

  Ok(())
}

#[test]
fn an_index_past_the_end_of_the_palette_is_damage() {
  let gif = include_bytes!("pictures/past-the-palette.gif");

  assert_eq!(
    decoded(gif).map(|picture| picture.size),
    Err(GifError::Corrupt)
  );
}

#[test]
fn a_positioned_frame_can_be_read_without_its_canvas() -> Result<(), Box<dyn Error>> {
  let (_, gif, canvas) = fixture!("placed");
  let frame = console_core_gif_files::decoded_frame(gif)?;
  assert_eq!(
    frame.size,
    Size {
      width: 10,
      height: 6
    }
  );
  let mut reference = Vec::new();

  for row in canvas.as_chunks::<4>().0.chunks_exact(23).skip(4).take(6) {
    let placed = row.get(5..15).ok_or("incomplete reference row")?;

    for pixel in placed {
      reference.extend_from_slice(pixel);
    }
  }

  assert_eq!(frame.bytes, reference);
  Ok(())
}

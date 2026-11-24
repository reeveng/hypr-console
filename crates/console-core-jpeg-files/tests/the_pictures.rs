//! The `.ycbcr` fixtures contain interleaved 8-bit scanlines from decoding
//! the corresponding JPEG with ITU-T T.873 reference B (libjpeg-turbo 3.0.0),
//! using `out_color_space = JCS_YCbCr`, `JDCT_ISLOW` and fancy upsampling.

use std::error::Error;

use console_core_geometry::Size;
use console_core_jpeg_files::{JpegError, Picture, Task, decoded, decoded_spread, measured};

#[cfg(feature = "std")]
use console_core_jpeg_files::{Quality, encoded};

const WHOLE: Size<u32> = Size {
  width: 75,
  height: 53,
};
const SUBSAMPLED_RGB: &[u8] = include_bytes!("pictures/subsampled.rgb");

const TURNED: Size<u32> = Size {
  width: 53,
  height: 75,
};

const TALL: Size<u32> = Size {
  width: 13,
  height: 1037,
};

const LYING: Size<u32> = Size {
  width: 1037,
  height: 13,
};

struct Difference {
  mean: MeanError,
  most: u8,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this wraps the average absolute difference in 8-bit RGB channel steps, so a mean smaller than one step is representable"
  )
)]
struct MeanError(f64);

fn difference(picture: &Picture, expected: &[u8]) -> Result<Difference, Box<dyn Error>> {
  let rgb: Vec<u8> = picture
    .bytes
    .as_chunks::<4>()
    .0
    .iter()
    .flat_map(|[red, green, blue, _]| [*red, *green, *blue])
    .collect();

  assert_eq!(
    rgb.len(),
    expected.len(),
    "{rgb_len} bytes against {expected_len}",
    rgb_len = rgb.len(),
    expected_len = expected.len()
  );

  let differences: Vec<u8> = rgb
    .iter()
    .zip(expected)
    .map(|(got, wanted)| got.abs_diff(*wanted))
    .collect();
  let total: u64 = differences.iter().map(|each| u64::from(*each)).sum();
  let count = u32::try_from(differences.len())?;
  let most = differences
    .iter()
    .copied()
    .max()
    .ok_or("no pixels at all")?;

  let total_2 = u32::try_from(total)?;

  Ok(Difference {
    mean: MeanError(f64::from(total_2) / f64::from(count)),
    most,
  })
}

fn close(picture: &Picture, expected: &[u8], within: MeanError) -> Result<(), Box<dyn Error>> {
  let found = difference(picture, expected)?;

  assert!(
    found.mean.0 <= within.0,
    "on average {:.2} away from libjpeg, at most {}",
    found.mean.0,
    found.most
  );

  Ok(())
}

#[test]
fn subsampled_components_agree_with_the_itu_reference_across_scan_layouts()
-> Result<(), Box<dyn Error>> {
  use console_core_jpeg_files::{ColorSpace, decoded_components};

  let expected: &[u8; 75 * 53 * 3] = include_bytes!("pictures/subsampled.ycbcr");
  let layouts: [(&str, &[u8]); 3] = [
    ("baseline", include_bytes!("pictures/subsampled.jpg")),
    ("progressive", include_bytes!("pictures/progressive.jpg")),
    ("restart markers", include_bytes!("pictures/restarts.jpg")),
  ];

  for (layout, bytes) in layouts {
    let picture = decoded_components(bytes)?;
    assert_eq!(picture.size, WHOLE, "{layout}");
    assert_eq!(picture.color, ColorSpace::YCbCr, "{layout}");
    assert_eq!(picture.bytes.len(), expected.len(), "{layout}");

    let pixels = picture.bytes.as_chunks::<3>().0;
    let reference = expected.as_chunks::<3>().0;

    for (index, (pixel, expected_pixel)) in pixels.iter().zip(reference).enumerate() {
      assert_eq!(pixel, expected_pixel, "{layout} YCbCr at pixel {index}");
    }
  }

  Ok(())
}

#[test]
fn every_layout_a_camera_writes_comes_back_as_libjpeg_draws_it() -> Result<(), Box<dyn Error>> {
  let pictures: [(&[u8], &[u8]); 6] = [
    (
      include_bytes!("pictures/full.jpg"),
      include_bytes!("pictures/full.rgb"),
    ),
    (
      include_bytes!("pictures/halved.jpg"),
      include_bytes!("pictures/halved.rgb"),
    ),
    (include_bytes!("pictures/subsampled.jpg"), SUBSAMPLED_RGB),
    (
      include_bytes!("pictures/grey.jpg"),
      include_bytes!("pictures/grey.rgb"),
    ),
    (include_bytes!("pictures/progressive.jpg"), SUBSAMPLED_RGB),
    (include_bytes!("pictures/restarts.jpg"), SUBSAMPLED_RGB),
  ];

  for (bytes, expected) in pictures {
    let picture = decoded(bytes, WHOLE)?;

    assert_eq!(picture.size, WHOLE);

    close(&picture, expected, MeanError(0.0))?;
  }

  Ok(())
}

#[test]
fn chroma_at_the_image_edges_matches_the_itu_reference() -> Result<(), Box<dyn Error>> {
  use console_core_jpeg_files::decoded_components;

  let pictures: [(&str, &[u8], &[u8]); 3] = [
    (
      "vertical subsampling",
      include_bytes!("pictures/vertical-8x4.jpg"),
      include_bytes!("pictures/vertical-8x4.ycbcr"),
    ),
    (
      "two chroma samples across",
      include_bytes!("pictures/narrow-4x8.jpg"),
      include_bytes!("pictures/narrow-4x8.ycbcr"),
    ),
    (
      "partial chroma block",
      include_bytes!("pictures/subsampled-8x4.jpg"),
      include_bytes!("pictures/subsampled-8x4.ycbcr"),
    ),
  ];

  for (layout, bytes, expected) in pictures {
    let picture = decoded_components(bytes)?;
    assert_eq!(picture.bytes, expected, "{layout}");
  }

  Ok(())
}

#[test]
fn a_picture_is_read_at_the_smallest_size_that_still_covers_what_was_asked()
-> Result<(), Box<dyn Error>> {
  let pictures: [(&[u8], [&[u8]; 3]); 2] = [
    (
      include_bytes!("pictures/subsampled.jpg"),
      [
        include_bytes!("pictures/subsampled-38x27.rgb"),
        include_bytes!("pictures/subsampled-19x14.rgb"),
        include_bytes!("pictures/subsampled-10x7.rgb"),
      ],
    ),
    (
      include_bytes!("pictures/progressive.jpg"),
      [
        include_bytes!("pictures/progressive-38x27.rgb"),
        include_bytes!("pictures/progressive-19x14.rgb"),
        include_bytes!("pictures/progressive-10x7.rgb"),
      ],
    ),
  ];

  let sizes = [
    Size {
      width: 38,
      height: 27,
    },
    Size {
      width: 19,
      height: 14,
    },
    Size {
      width: 10,
      height: 7,
    },
  ];
  let asked = [
    Size {
      width: 30,
      height: 20,
    },
    Size {
      width: 19,
      height: 1,
    },
    Size {
      width: 1,
      height: 1,
    },
  ];

  for (bytes, expected) in pictures {
    for ((size, covering), expected_2) in sizes.iter().zip(asked).zip(expected) {
      let picture = decoded(bytes, covering)?;

      assert_eq!(picture.size, *size, "asked for {covering:?}");

      close(&picture, expected_2, MeanError(2.0))?;
    }
  }

  Ok(())
}

#[test]
fn a_picture_asked_for_larger_than_it_is_comes_back_at_its_own_size() -> Result<(), Box<dyn Error>>
{
  let picture = decoded(
    include_bytes!("pictures/subsampled.jpg"),
    Size {
      width: 4000,
      height: 3000,
    },
  )?;

  assert_eq!(picture.size, WHOLE);

  Ok(())
}

#[test]
fn a_photograph_comes_back_the_way_the_camera_was_held() -> Result<(), Box<dyn Error>> {
  let bytes = include_bytes!("pictures/turned.jpg");

  assert_eq!(measured(bytes), Ok(TURNED));

  let picture = decoded(bytes, TURNED)?;

  assert_eq!(picture.size, TURNED);

  close(
    &picture,
    include_bytes!("pictures/turned.rgb"),
    MeanError(0.1),
  )
}

#[test]
fn a_turned_photograph_is_asked_for_in_its_upright_size() -> Result<(), Box<dyn Error>> {
  let picture = decoded(
    include_bytes!("pictures/turned.jpg"),
    Size {
      width: 14,
      height: 19,
    },
  )?;

  assert_eq!(
    picture.size,
    Size {
      width: 14,
      height: 19
    }
  );

  Ok(())
}

#[test]
fn the_size_is_said_without_reading_the_picture() {
  assert_eq!(
    measured(include_bytes!("pictures/subsampled.jpg")),
    Ok(WHOLE)
  );
  assert_eq!(
    measured(include_bytes!("pictures/progressive.jpg")),
    Ok(WHOLE)
  );
}

#[test]
fn ink_is_drawn_as_ffmpeg_drew_it() -> Result<(), Box<dyn Error>> {
  let pictures: [(&[u8], &[u8], f64); 2] = [
    (
      include_bytes!("pictures/cmyk.jpg"),
      include_bytes!("pictures/cmyk.rgb"),
      0.1,
    ),
    (
      include_bytes!("pictures/ycck.jpg"),
      include_bytes!("pictures/ycck.rgb"),
      1.2,
    ),
  ];

  for (bytes, expected, within) in pictures {
    let picture = decoded(bytes, WHOLE)?;

    assert_eq!(picture.size, WHOLE);
    close(&picture, expected, MeanError(within))?;
  }

  Ok(())
}

#[test]
fn a_file_that_is_not_a_jpeg_says_so() {
  assert_eq!(measured(b"\x89PNG\r\n\x1a\n"), Err(JpegError::NotAJpeg));
  assert_eq!(decoded(b"", WHOLE), Err(JpegError::NotAJpeg));
}

#[test]
fn a_file_that_stops_before_its_frame_is_cut_short() -> Result<(), Box<dyn Error>> {
  let bytes = include_bytes!("pictures/subsampled.jpg");
  let (opening, _rest) = bytes.split_first_chunk::<40>().ok_or("an opening")?;

  assert_eq!(decoded(opening, WHOLE), Err(JpegError::Truncated));

  Ok(())
}

#[test]
fn a_file_that_stops_before_its_picture_ends_is_drawn_as_far_as_it_goes()
-> Result<(), Box<dyn Error>> {
  let bytes = include_bytes!("pictures/subsampled.jpg");
  let (first_half, _rest) = bytes
    .split_at_checked(bytes.len().div_euclid(2))
    .ok_or("half a file")?;
  let picture = decoded(first_half, WHOLE)?;

  assert_eq!(picture.size, WHOLE);

  let top = difference(&picture, SUBSAMPLED_RGB)?;

  assert!(top.most > 0, "the bottom half cannot be the picture");

  Ok(())
}

#[test]
fn a_picture_taller_than_a_round_of_rows_is_colored_a_round_at_a_time()
-> Result<(), Box<dyn Error>> {
  let pictures: [(&[u8], &[u8], Size<u32>); 3] = [
    (
      include_bytes!("pictures/tall.jpg"),
      include_bytes!("pictures/tall.rgb"),
      TALL,
    ),
    (
      include_bytes!("pictures/tall-progressive.jpg"),
      include_bytes!("pictures/tall.rgb"),
      TALL,
    ),
    (
      include_bytes!("pictures/tall-turned.jpg"),
      include_bytes!("pictures/tall-turned.rgb"),
      LYING,
    ),
  ];

  for (bytes, expected, size) in pictures {
    let picture = decoded(bytes, size)?;

    assert_eq!(picture.size, size);

    let found = difference(&picture, expected)?;

    assert!(
      found.mean.0 <= 0.1 && found.most <= 2,
      "on average {:.2} away from libjpeg, at most {}",
      found.mean.0,
      found.most
    );
  }

  Ok(())
}

fn backwards(tasks: &[Task<'_>]) -> Result<(), JpegError> {
  tasks.iter().rev().try_for_each(Task::done)
}

#[test]
fn the_jobs_of_a_picture_can_be_done_in_any_order() -> Result<(), Box<dyn Error>> {
  let pictures: [&[u8]; 5] = [
    include_bytes!("pictures/subsampled.jpg"),
    include_bytes!("pictures/progressive.jpg"),
    include_bytes!("pictures/restarts.jpg"),
    include_bytes!("pictures/turned.jpg"),
    include_bytes!("pictures/tall-turned.jpg"),
  ];

  for bytes in pictures {
    for covering in [
      Size {
        width: 10,
        height: 10,
      },
      WHOLE,
    ] {
      let in_order = decoded(bytes, covering)?;
      let out_of_order = decoded_spread(bytes, covering, &backwards)?;

      assert_eq!(in_order, out_of_order);
    }
  }

  Ok(())
}

#[cfg(feature = "std")]
fn rgb(picture: &Picture) -> Result<Vec<u8>, Box<dyn Error>> {
  Ok(
    picture
      .bytes
      .as_chunks::<4>()
      .0
      .iter()
      .flat_map(|[red, green, blue, _]| [*red, *green, *blue])
      .collect(),
  )
}

#[cfg(feature = "std")]
#[test]
fn a_picture_written_and_read_again_is_as_close_as_its_quality_keeps_it()
-> Result<(), Box<dyn Error>> {
  let picture = decoded(include_bytes!("pictures/subsampled.jpg"), WHOLE)?;
  let expected = rgb(&picture)?;
  let mut smaller = 0u32;

  for (percent, within) in [(50, 5.5), (85, 3.5), (95, 2.0)] {
    let Ok(quality) = Quality::percent(percent);
    let written = encoded(&picture, quality)?;
    let again = decoded(&written, WHOLE)?;
    let long = u32::try_from(written.len())?;

    assert_eq!(again.size, WHOLE);
    assert!(
      long > smaller,
      "{percent} is no larger than the quality under it"
    );

    close(&again, &expected, MeanError(within))?;

    smaller = long;
  }

  Ok(())
}

#[cfg(feature = "std")]
#[test]
fn what_is_written_is_jfif_from_its_first_marker_to_its_last() -> Result<(), Box<dyn Error>> {
  let picture = decoded(include_bytes!("pictures/full.jpg"), WHOLE)?;
  let Ok(quality) = Quality::percent(85);
  let written = encoded(&picture, quality)?;

  assert!(written.starts_with(&[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00
  ]));
  assert!(written.ends_with(&[0xFF, 0xD9]));

  Ok(())
}

#[cfg(feature = "std")]
#[test]
fn a_picture_that_is_not_whole_squares_is_filled_out_and_read_back_at_its_own_size()
-> Result<(), Box<dyn Error>> {
  let Ok(quality) = Quality::percent(90);

  let sizes: [Size<u32>; 4] = [
    Size {
      width: 1,
      height: 1,
    },
    Size {
      width: 8,
      height: 8,
    },
    Size {
      width: 17,
      height: 9,
    },
    Size {
      width: 33,
      height: 31,
    },
  ];

  for size in sizes {
    let rgba: Vec<u8> = (0..size.height)
      .flat_map(|y| (0..size.width).map(move |x| (x, y)))
      .flat_map(|(x, y)| {
        let [across, ..] = x.wrapping_mul(7).to_le_bytes();
        let [down, ..] = y.wrapping_mul(5).to_le_bytes();

        [across, down, 128, 255]
      })
      .collect();
    let picture = Picture { size, bytes: rgba };
    let written = encoded(&picture, quality)?;
    let again = decoded(&written, size)?;
    let expected = rgb(&picture)?;

    assert_eq!(again.size, size);

    close(&again, &expected, MeanError(3.0))?;
  }

  Ok(())
}

#[cfg(feature = "std")]
#[test]
fn pixels_that_are_not_the_size_they_came_with_or_too_wide_to_say_are_refused() {
  let Ok(quality) = Quality::percent(85);
  let short = Picture {
    size: Size {
      width: 2,
      height: 2,
    },
    bytes: vec![0; 15],
  };
  let nothing = Picture {
    size: Size {
      width: 0,
      height: 0,
    },
    bytes: Vec::new(),
  };
  let wide = Picture {
    size: Size {
      width: 65_536,
      height: 1,
    },
    bytes: vec![0; 262_144],
  };

  assert_eq!(encoded(&short, quality), Err(JpegError::Mismatched));
  assert_eq!(encoded(&nothing, quality), Err(JpegError::Mismatched));
  assert_eq!(encoded(&wide, quality), Err(JpegError::TooLarge));
}

#[test]
fn the_stored_raster_ignores_exif_for_an_embedded_jpeg() -> Result<(), Box<dyn Error>> {
  use console_core_jpeg_files::decoded_as_stored;
  let tagged = include_bytes!("pictures/tall-turned.jpg");
  let ordinary = include_bytes!("pictures/tall.jpg");
  let stored = decoded_as_stored(tagged)?;
  let size = measured(ordinary)?;
  let expected = decoded(ordinary, size)?;
  let upright = measured(tagged)?;
  assert_eq!(stored, expected);
  assert_ne!(stored.size, upright);
  Ok(())
}

#[test]
fn two_independent_components_keep_their_values_in_scan_order() -> Result<(), Box<dyn Error>> {
  use console_core_jpeg_files::{ColorSpace, decoded_components};
  let mut samples = decoded_components(include_bytes!("pictures/two-component.jpg"))?;
  assert_eq!(
    samples.size,
    Size {
      width: 1,
      height: 1
    }
  );
  assert_eq!(samples.color, ColorSpace::TwoComponent);
  let Ok(()) = samples.convert_ycbcr_to_rgb();
  assert_eq!(samples.bytes, [136, 120]);
  Ok(())
}

#[test]
fn an_avi1_frame_can_omit_its_standard_huffman_tables() -> Result<(), Box<dyn Error>> {
  let bytes = include_bytes!("pictures/avi1.jpg");
  let samples = console_core_jpeg_files::decoded_components(bytes)?;
  assert_eq!(samples.bytes, [128]);
  Ok(())
}

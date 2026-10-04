use std::error::Error;

use console_core_geometry::Size;
use console_core_jpeg_files::{JpegError, Picture, Quality, Task, Unsupported, decoded, decoded_spread, encoded, measured};

const WHOLE: Size<u32> = Size { width: 75, height: 53 };

const TURNED: Size<u32> = Size { width: 53, height: 75 };

const TALL: Size<u32> = Size { width: 13, height: 1037 };

const LYING: Size<u32> = Size { width: 1037, height: 13 };

struct Difference {
    mean: f64,
    most: u8,
}

fn difference(picture: &Picture, expected: &[u8]) -> Result<Difference, Box<dyn Error>> {
    let rgb: Vec<u8> = picture.rgba.as_chunks::<4>().0.iter().flat_map(|[red, green, blue, _]| [*red, *green, *blue]).collect();

    assert_eq!(rgb.len(), expected.len(), "{rgb_len} bytes against {expected_len}", rgb_len = rgb.len(), expected_len = expected.len());

    let differences: Vec<u8> = rgb.iter().zip(expected).map(|(got, wanted)| got.abs_diff(*wanted)).collect();
    let total: u64 = differences.iter().map(|each| u64::from(*each)).sum();
    let count = u32::try_from(differences.len())?;
    let most = differences.iter().copied().max().ok_or("no pixels at all")?;

    let total = u32::try_from(total)?;

    Ok(Difference { mean: f64::from(total) / f64::from(count), most })
}

fn close(picture: &Picture, expected: &[u8], within: f64) -> Result<(), Box<dyn Error>> {
    let found = difference(picture, expected)?;

    assert!(found.mean <= within, "on average {:.2} away from libjpeg, at most {}", found.mean, found.most);

    Ok(())
}

#[test]
fn every_layout_a_camera_writes_comes_back_as_libjpeg_draws_it() -> Result<(), Box<dyn Error>> {
    let pictures: [(&[u8], &[u8], f64); 6] = [
        (include_bytes!("pictures/full.jpg"), include_bytes!("pictures/full.rgb"), 0.0),
        (include_bytes!("pictures/halved.jpg"), include_bytes!("pictures/halved.rgb"), 0.2),
        (include_bytes!("pictures/subsampled.jpg"), include_bytes!("pictures/subsampled.rgb"), 0.1),
        (include_bytes!("pictures/grey.jpg"), include_bytes!("pictures/grey.rgb"), 0.0),
        (include_bytes!("pictures/progressive.jpg"), include_bytes!("pictures/progressive.rgb"), 0.1),
        (include_bytes!("pictures/restarts.jpg"), include_bytes!("pictures/restarts.rgb"), 0.1),
    ];

    for (bytes, expected, within) in pictures {
        let picture = decoded(bytes, WHOLE)?;

        assert_eq!(picture.size, WHOLE);

        close(&picture, expected, within)?;
    }

    Ok(())
}

#[test]
fn a_picture_is_read_at_the_smallest_size_that_still_covers_what_was_asked() -> Result<(), Box<dyn Error>> {
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

    let sizes = [Size { width: 38, height: 27 }, Size { width: 19, height: 14 }, Size { width: 10, height: 7 }];
    let asked = [Size { width: 30, height: 20 }, Size { width: 19, height: 1 }, Size { width: 1, height: 1 }];

    for (bytes, expected) in pictures {
        for ((size, asked), expected) in sizes.iter().zip(asked).zip(expected) {
            let picture = decoded(bytes, asked)?;

            assert_eq!(picture.size, *size, "asked for {asked:?}");

            close(&picture, expected, 2.0)?;
        }
    }

    Ok(())
}

#[test]
fn a_picture_asked_for_larger_than_it_is_comes_back_at_its_own_size() -> Result<(), Box<dyn Error>> {
    let picture = decoded(include_bytes!("pictures/subsampled.jpg"), Size { width: 4000, height: 3000 })?;

    assert_eq!(picture.size, WHOLE);

    Ok(())
}

#[test]
fn a_photograph_comes_back_the_way_the_camera_was_held() -> Result<(), Box<dyn Error>> {
    let bytes = include_bytes!("pictures/turned.jpg");

    assert_eq!(measured(bytes), Ok(TURNED));

    let picture = decoded(bytes, TURNED)?;

    assert_eq!(picture.size, TURNED);

    close(&picture, include_bytes!("pictures/turned.rgb"), 0.1)
}

#[test]
fn a_turned_photograph_is_asked_for_in_its_upright_size() -> Result<(), Box<dyn Error>> {
    let picture = decoded(include_bytes!("pictures/turned.jpg"), Size { width: 14, height: 19 })?;

    assert_eq!(picture.size, Size { width: 14, height: 19 });

    Ok(())
}

#[test]
fn the_size_is_said_without_reading_the_picture() {
    assert_eq!(measured(include_bytes!("pictures/subsampled.jpg")), Ok(WHOLE));
    assert_eq!(measured(include_bytes!("pictures/progressive.jpg")), Ok(WHOLE));
}

#[test]
fn what_a_camera_does_not_write_is_refused_by_name() {
    let refused = decoded(include_bytes!("pictures/cmyk.jpg"), WHOLE);

    assert_eq!(refused, Err(JpegError::Unsupported(Unsupported::Cmyk)));
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
fn a_file_that_stops_before_its_picture_ends_is_drawn_as_far_as_it_goes() -> Result<(), Box<dyn Error>> {
    let bytes = include_bytes!("pictures/subsampled.jpg");
    let (first_half, _rest) = bytes.split_at_checked(bytes.len().div_euclid(2)).ok_or("half a file")?;
    let picture = decoded(first_half, WHOLE)?;

    assert_eq!(picture.size, WHOLE);

    let top = difference(&picture, include_bytes!("pictures/subsampled.rgb"))?;

    assert!(top.most > 0, "the bottom half cannot be the picture");

    Ok(())
}

#[test]
fn a_picture_taller_than_a_round_of_rows_is_colored_a_round_at_a_time() -> Result<(), Box<dyn Error>> {
    let pictures: [(&[u8], &[u8], Size<u32>); 3] = [
        (include_bytes!("pictures/tall.jpg"), include_bytes!("pictures/tall.rgb"), TALL),
        (include_bytes!("pictures/tall-progressive.jpg"), include_bytes!("pictures/tall.rgb"), TALL),
        (include_bytes!("pictures/tall-turned.jpg"), include_bytes!("pictures/tall-turned.rgb"), LYING),
    ];

    for (bytes, expected, size) in pictures {
        let picture = decoded(bytes, size)?;

        assert_eq!(picture.size, size);

        let found = difference(&picture, expected)?;

        assert!(found.mean <= 0.1 && found.most <= 2, "on average {:.2} away from libjpeg, at most {}", found.mean, found.most);
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
        for covering in [Size { width: 10, height: 10 }, WHOLE] {
            let in_order = decoded(bytes, covering)?;
            let out_of_order = decoded_spread(bytes, covering, &backwards)?;

            assert_eq!(in_order, out_of_order);
        }
    }

    Ok(())
}

fn rgb(picture: &Picture) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(picture.rgba.as_chunks::<4>().0.iter().flat_map(|[red, green, blue, _]| [*red, *green, *blue]).collect())
}

#[test]
fn a_picture_written_and_read_again_is_as_close_as_its_quality_keeps_it() -> Result<(), Box<dyn Error>> {
    let picture = decoded(include_bytes!("pictures/subsampled.jpg"), WHOLE)?;
    let expected = rgb(&picture)?;
    let mut smaller = 0u32;

    for (percent, within) in [(50, 5.5), (85, 3.5), (95, 2.0)] {
        let Ok(quality) = Quality::percent(percent);
        let written = encoded(&picture, quality)?;
        let again = decoded(&written, WHOLE)?;
        let long = u32::try_from(written.len())?;

        assert_eq!(again.size, WHOLE);
        assert!(long > smaller, "{percent} is no larger than the quality under it");

        close(&again, &expected, within)?;

        smaller = long;
    }

    Ok(())
}

#[test]
fn what_is_written_is_jfif_from_its_first_marker_to_its_last() -> Result<(), Box<dyn Error>> {
    let picture = decoded(include_bytes!("pictures/full.jpg"), WHOLE)?;
    let Ok(quality) = Quality::percent(85);
    let written = encoded(&picture, quality)?;

    assert!(written.starts_with(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00]));
    assert!(written.ends_with(&[0xFF, 0xD9]));

    Ok(())
}

#[test]
fn a_picture_that_is_not_whole_squares_is_filled_out_and_read_back_at_its_own_size() -> Result<(), Box<dyn Error>> {
    let Ok(quality) = Quality::percent(90);

    let sizes: [Size<u32>; 4] =
        [Size { width: 1, height: 1 }, Size { width: 8, height: 8 }, Size { width: 17, height: 9 }, Size { width: 33, height: 31 }];

    for size in sizes {
        let rgba: Vec<u8> = (0..size.height)
            .flat_map(|y| (0..size.width).map(move |x| (x, y)))
            .flat_map(|(x, y)| {
                let [across, ..] = x.wrapping_mul(7).to_le_bytes();
                let [down, ..] = y.wrapping_mul(5).to_le_bytes();

                [across, down, 128, 255]
            })
            .collect();
        let picture = Picture { size, rgba };
        let written = encoded(&picture, quality)?;
        let again = decoded(&written, size)?;
        let expected = rgb(&picture)?;

        assert_eq!(again.size, size);

        close(&again, &expected, 3.0)?;
    }

    Ok(())
}

#[test]
fn pixels_that_are_not_the_size_they_came_with_or_too_wide_to_say_are_refused() {
    let Ok(quality) = Quality::percent(85);
    let short = Picture { size: Size { width: 2, height: 2 }, rgba: vec![0; 15] };
    let nothing = Picture { size: Size { width: 0, height: 0 }, rgba: Vec::new() };
    let wide = Picture { size: Size { width: 65_536, height: 1 }, rgba: vec![0; 262_144] };

    assert_eq!(encoded(&short, quality), Err(JpegError::Mismatched));
    assert_eq!(encoded(&nothing, quality), Err(JpegError::Mismatched));
    assert_eq!(encoded(&wide, quality), Err(JpegError::TooLarge));
}

use std::error::Error;

use console_core_geometry::Size;
use console_core_png_files::{PngError, Picture, decoded, encoded, measured};

const PICTURE: Size<u32> = Size { width: 23, height: 17 };

macro_rules! fixture {
    ($name:literal) => {
        ($name, include_bytes!(concat!("pictures/", $name, ".png")).as_slice(), include_bytes!(concat!("pictures/", $name, ".rgba")).as_slice())
    };
}

const EVERY_KIND: [(&str, &[u8], &[u8]); 21] = [
    fixture!("grey-1"),
    fixture!("grey-2"),
    fixture!("grey-4"),
    fixture!("grey-8"),
    fixture!("grey-16"),
    fixture!("grey-8-keyed"),
    fixture!("grey-16-keyed"),
    fixture!("rgb-8"),
    fixture!("rgb-16"),
    fixture!("rgb-8-keyed"),
    fixture!("indexed-1"),
    fixture!("indexed-2"),
    fixture!("indexed-4"),
    fixture!("indexed-8"),
    fixture!("grey-alpha-8"),
    fixture!("grey-alpha-16"),
    fixture!("rgba-8"),
    fixture!("rgba-16"),
    fixture!("rgba-8-interlaced"),
    fixture!("grey-1-interlaced"),
    fixture!("indexed-4-interlaced"),
];

#[test]
fn every_kind_of_pixel_comes_back_as_libpng_draws_it() -> Result<(), Box<dyn Error>> {
    for (name, png, rgba) in EVERY_KIND {
        let picture = decoded(png)?;

        assert_eq!(picture.size, PICTURE, "{name}");
        assert!(picture.rgba == rgba, "{name} is not what libpng draws");
    }

    Ok(())
}

#[test]
fn an_interlaced_picture_too_small_for_some_passes_skips_them() -> Result<(), Box<dyn Error>> {
    let small = [
        (fixture!("rgb-16-interlaced"), PICTURE),
        (fixture!("one-pixel-interlaced"), Size { width: 1, height: 1 }),
        (fixture!("three-by-two-interlaced"), Size { width: 3, height: 2 }),
    ];

    for ((name, png, rgba), size) in small {
        let picture = decoded(png)?;

        assert_eq!(picture.size, size, "{name}");
        assert!(picture.rgba == rgba, "{name} is not what libpng draws");
    }

    Ok(())
}

#[test]
fn the_size_is_said_from_the_header_alone() {
    let (_, png, _) = fixture!("rgba-8");

    assert_eq!(measured(png), Ok(PICTURE));
    let (header, _) = png.split_at(33);

    assert_eq!(measured(header), Ok(PICTURE));
}

#[test]
fn a_file_that_is_not_a_png_says_so() {
    assert_eq!(decoded(b"GIF89a, which is something else entirely").map(|picture| picture.size), Err(PngError::NotAPng));
}

#[test]
fn a_file_cut_short_is_refused_rather_than_drawn_in_part() {
    let (_, png, _) = fixture!("rgba-8");

    for cut in [4, 20, 40, png.len().saturating_sub(20)] {
        let (short, _) = png.split_at(cut);

        assert_eq!(decoded(short).map(|picture| picture.size), Err(PngError::Truncated), "cut at {cut}");
    }
}

#[test]
fn a_chunk_whose_bytes_changed_is_damage() -> Result<(), Box<dyn Error>> {
    let (_, png, _) = fixture!("rgba-8");
    let mut changed = png.to_vec();

    match changed.get_mut(60) {
        Some(byte) => *byte = byte.wrapping_add(1),
        None => return Err(Box::<dyn Error>::from("the picture is shorter than the byte changed")),
    }

    assert_eq!(decoded(&changed).map(|picture| picture.size), Err(PngError::Corrupt));

    Ok(())
}

#[test]
fn every_kind_of_picture_written_and_read_again_is_the_picture_it_was() -> Result<(), Box<dyn Error>> {
    for (name, png, _) in EVERY_KIND {
        let picture = decoded(png)?;
        let written = encoded(&picture)?;
        let again = decoded(&written)?;

        assert!(again == picture, "{name} came back different");
    }

    Ok(())
}

#[test]
fn a_picture_is_written_with_only_the_channels_it_uses() -> Result<(), Box<dyn Error>> {
    let size = Size { width: 2, height: 1 };
    let pictures = [
        ([[9, 9, 9, 255], [200, 200, 200, 255]], 0),
        ([[9, 9, 9, 255], [200, 200, 200, 17]], 4),
        ([[9, 10, 9, 255], [200, 200, 200, 255]], 2),
        ([[9, 10, 9, 255], [200, 200, 200, 17]], 6),
    ];

    for (pixels, color_type) in pictures {
        let picture = Picture { size, rgba: pixels.as_flattened().to_vec() };
        let written = encoded(&picture)?;
        let again = decoded(&written)?;
        let header = written.get(16..29).and_then(<[u8]>::first_chunk::<13>);

        assert_eq!(header.map(|[.., depth, color, _, _, _]| (*depth, *color)), Some((8, color_type)), "{pixels:?}");
        assert!(again == picture, "{pixels:?} came back different");
    }

    Ok(())
}

#[test]
fn pixels_that_are_not_the_size_they_came_with_are_refused() {
    let short = Picture { size: Size { width: 2, height: 2 }, rgba: vec![0; 15] };
    let nothing = Picture { size: Size { width: 0, height: 0 }, rgba: Vec::new() };

    assert_eq!(encoded(&short), Err(PngError::Mismatched));
    assert_eq!(encoded(&nothing), Err(PngError::Mismatched));
}

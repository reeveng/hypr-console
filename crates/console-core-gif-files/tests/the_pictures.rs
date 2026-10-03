use std::error::Error;

use console_core_geometry::Size;
use console_core_gif_files::{GifError, decoded, measured};

const PICTURE: Size<u32> = Size { width: 23, height: 17 };

macro_rules! fixture {
    ($name:literal) => {
        ($name, include_bytes!(concat!("pictures/", $name, ".gif")).as_slice(), include_bytes!(concat!("pictures/", $name, ".rgba")).as_slice())
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
        assert!(picture.rgba == rgba, "{name} is not what giflib draws");
    }

    Ok(())
}

#[test]
fn a_table_that_fills_comes_back_whether_it_is_cleared_or_kept() -> Result<(), Box<dyn Error>> {
    for (name, gif, rgba) in [fixture!("full-table-cleared"), fixture!("full-table-kept")] {
        let picture = decoded(gif)?;

        assert_eq!(picture.size, Size { width: 97, height: 61 }, "{name}");
        assert!(picture.rgba == rgba, "{name} is not what giflib draws");
    }

    Ok(())
}

#[test]
fn the_size_is_said_from_the_screen_alone() {
    let (_, gif, _) = fixture!("placed");
    let (screen, _) = gif.split_at(13);

    assert_eq!(measured(gif), Ok(PICTURE));
    assert_eq!(measured(screen), Ok(PICTURE));
}

#[test]
fn a_file_that_is_not_a_gif_says_so() {
    assert_eq!(decoded(b"\x89PNG\r\n\x1a\n, which is something else entirely").map(|picture| picture.size), Err(GifError::NotAGif));
}

#[test]
fn a_file_cut_short_is_refused_rather_than_drawn_in_part() {
    let (_, gif, _) = fixture!("global");

    for cut in [4, 12, 40, 200, gif.len().saturating_sub(20)] {
        let (short, _) = gif.split_at(cut);

        assert_eq!(decoded(short).map(|picture| picture.size), Err(GifError::Truncated), "cut at {cut}");
    }
}

#[test]
fn an_index_past_the_end_of_the_palette_is_damage() {
    let gif = include_bytes!("pictures/past-the-palette.gif");

    assert_eq!(decoded(gif).map(|picture| picture.size), Err(GifError::Corrupt));
}

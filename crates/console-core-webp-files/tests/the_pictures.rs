use std::error::Error;

use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_webp_files::{Receiver, Task, WebpError, decoded, decoded_in_bands, decoded_spread, measured};

macro_rules! fixture {
    ($name:literal) => {
        ($name, include_bytes!(concat!("pictures/", $name, ".webp")).as_slice(), include_bytes!(concat!("pictures/", $name, ".rgba")).as_slice())
    };
}

const EVERY_KIND: [(&str, &[u8], &[u8]); 26] = [
    fixture!("colour-m0-q0"),
    fixture!("colour-m0-q50"),
    fixture!("colour-m0-q100"),
    fixture!("colour-m3-q0"),
    fixture!("colour-m3-q50"),
    fixture!("colour-m3-q100"),
    fixture!("colour-m6-q0"),
    fixture!("colour-m6-q50"),
    fixture!("colour-m6-q100"),
    fixture!("alpha"),
    fixture!("alpha-cleared"),
    fixture!("palette-2"),
    fixture!("palette-3"),
    fixture!("palette-4"),
    fixture!("palette-11"),
    fixture!("palette-16"),
    fixture!("palette-200"),
    fixture!("grey"),
    fixture!("one-pixel"),
    fixture!("row"),
    fixture!("column"),
    fixture!("photograph"),
    fixture!("photograph-fast"),
    fixture!("screenshot"),
    fixture!("extended"),
    fixture!("animated"),
];

#[test]
fn every_kind_of_lossless_picture_comes_back_as_libwebp_draws_it() -> Result<(), Box<dyn Error>> {
    for (name, webp, rgba) in EVERY_KIND {
        let picture = decoded(webp)?;
        let area = u64::from(picture.size.width).saturating_mul(u64::from(picture.size.height)).saturating_mul(4);

        assert_eq!(u64::try_from(rgba.len()), Ok(area), "{name}");
        assert!(picture.rgba == rgba, "{name} is not what libwebp draws");
    }

    Ok(())
}

#[test]
fn the_size_is_said_from_the_first_thirty_bytes() {
    for (name, webp, size) in [
        ("alpha", include_bytes!("pictures/alpha.webp").as_slice(), Size { width: 53, height: 29 }),
        ("extended", include_bytes!("pictures/extended.webp").as_slice(), Size { width: 31, height: 19 }),
        ("animated", include_bytes!("pictures/animated.webp").as_slice(), Size { width: 40, height: 30 }),
        ("lossy", include_bytes!("pictures/lossy.webp").as_slice(), Size { width: 45, height: 27 }),
    ] {
        let (first, _) = webp.split_at(30);

        assert_eq!(measured(webp), Ok(size), "{name}");
        assert_eq!(measured(first), Ok(size), "{name}");
    }
}

const EVERY_LOSSY_KIND: [(&str, &[u8], &[u8]); 9] = [
    fixture!("lossy"),
    fixture!("lossy-alpha"),
    fixture!("lossy-alpha-gradient"),
    fixture!("lossy-odd"),
    fixture!("lossy-partitions"),
    fixture!("lossy-simple-filter"),
    fixture!("lossy-tall"),
    fixture!("lossy-very-tall"),
    fixture!("lossy-unfiltered"),
];

#[test]
fn every_kind_of_lossy_picture_comes_back_as_libwebp_draws_it() -> Result<(), Box<dyn Error>> {
    for (name, webp, rgba) in EVERY_LOSSY_KIND {
        let picture = decoded(webp)?;
        let area = u64::from(picture.size.width).saturating_mul(u64::from(picture.size.height)).saturating_mul(4);

        assert_eq!(u64::try_from(rgba.len()), Ok(area), "{name}");
        assert!(picture.rgba == rgba, "{name} is not what libwebp draws");
    }

    Ok(())
}

fn backwards(tasks: &[Task<'_>]) -> Result<(), WebpError> {
    tasks.iter().rev().try_for_each(Task::done)
}

#[test]
fn the_tasks_of_a_lossy_picture_can_be_done_in_any_order() -> Result<(), Box<dyn Error>> {
    for (name, webp, rgba) in EVERY_LOSSY_KIND {
        let picture = decoded_spread(webp, &backwards)?;

        assert!(picture.rgba == rgba, "{name} done backwards is not what libwebp draws");
    }

    Ok(())
}

fn parts(size: Size<u32>) -> Result<[Rectangle<u32>; 7], Never> {
    let Size { width, height } = size;
    let side = width.min(height);
    let third = Size { width: width.div_ceil(3), height: height.div_ceil(3) };

    Ok([
        Rectangle { origin: Point { x: 0, y: 0 }, size },
        Rectangle { origin: Point { x: width.saturating_sub(side).saturating_div(2), y: height.saturating_sub(side).saturating_div(2) }, size: Size { width: side, height: side } },
        Rectangle { origin: Point { x: 0, y: 0 }, size: Size { width: 16, height: 64 } },
        Rectangle { origin: Point { x: 8, y: 3 }, size: Size { width: 24, height: 125 } },
        Rectangle { origin: Point { x: third.width, y: third.height }, size: third },
        Rectangle { origin: Point { x: width.saturating_sub(third.width), y: height.saturating_sub(third.height) }, size: third },
        Rectangle { origin: Point { x: width.saturating_div(2), y: height.saturating_div(2) }, size },
    ])
}

fn cut(rgba: &[u8], from: (Size<u32>, Rectangle<u32>)) -> Result<Vec<u8>, Box<dyn Error>> {
    let (size, part) = from;
    let right = part.origin.x.saturating_add(part.size.width).min(size.width);
    let Ok(line) = index(size.width.saturating_mul(4));
    let Ok(left) = index(part.origin.x.min(size.width).saturating_mul(4));
    let Ok(right) = index(right.saturating_mul(4));
    let Ok(top) = index(part.origin.y);
    let Ok(tall) = index(part.size.height);
    let mut kept = Vec::new();

    for row in rgba.chunks_exact(line.max(1)).skip(top).take(tall) {
        let wanted = row.get(left..right).ok_or("a row narrower than the picture")?;

        kept.extend_from_slice(wanted);
    }

    Ok(kept)
}

struct Collected {
    rgba: Vec<u8>,
}

impl Receiver for Collected {
    fn received(&mut self, rgba: &[u8]) -> Result<(), Never> {
        self.rgba.extend_from_slice(rgba);

        Ok(())
    }
}

#[test]
fn a_picture_handed_in_bands_is_the_part_of_it_that_was_asked_for() -> Result<(), Box<dyn Error>> {
    for (name, webp, rgba) in EVERY_LOSSY_KIND.into_iter().chain([fixture!("photograph"), fixture!("animated")]) {
        let size = measured(webp)?;

        let Ok(parts) = parts(size);

        for part in parts {
            let mut handed = Collected { rgba: Vec::new() };
            let drawn = decoded_in_bands(webp, &backwards, (part, &mut handed))?;
            let expected = cut(rgba, (size, part))?;

            assert_eq!(drawn, size, "{name}");
            assert!(handed.rgba == expected, "{name}: {part:?} is not that part of what libwebp draws");
        }
    }

    Ok(())
}

#[test]
fn a_lossy_bitstream_that_ends_before_its_pixels_do_is_refused() -> Result<(), Box<dyn Error>> {
    let (_, webp, _) = fixture!("lossy-tall");

    for cut in (40..webp.len()).step_by(97) {
        let (short, _) = webp.split_at(cut);
        let mut short = short.to_vec();
        let riff = u32::try_from(cut.saturating_sub(8))?;
        let chunk = u32::try_from(cut.saturating_sub(20))?;

        for (at, byte) in (4..8).zip(riff.to_le_bytes()).chain((16..20).zip(chunk.to_le_bytes())) {
            match short.get_mut(at) {
                Some(slot) => *slot = byte,
                None => return Err(Box::<dyn Error>::from("the cut is shorter than the header")),
            }
        }

        assert_eq!(decoded(&short).map(|picture| picture.size), Err(WebpError::Truncated), "cut at {cut}");
    }

    Ok(())
}

#[test]
fn a_file_that_is_not_a_webp_says_so() {
    assert_eq!(decoded(b"RIFF\x24\x00\x00\x00WAVEfmt , which is something else entirely").map(|picture| picture.size), Err(WebpError::NotAWebp));
}

#[test]
fn a_file_cut_short_is_refused_rather_than_drawn_in_part() {
    let (_, webp, _) = fixture!("photograph");

    for cut in [4, 12, 20, 24, 200, webp.len().saturating_div(2), webp.len().saturating_sub(1)] {
        let (short, _) = webp.split_at(cut);

        assert_eq!(decoded(short).map(|picture| picture.size), Err(WebpError::Truncated), "cut at {cut}");
    }
}

#[test]
fn a_bitstream_that_ends_before_its_pixels_do_is_refused() -> Result<(), Box<dyn Error>> {
    let (_, webp, _) = fixture!("screenshot");

    for cut in (30..webp.len()).step_by(997) {
        let (short, _) = webp.split_at(cut);
        let mut short = short.to_vec();
        let riff = u32::try_from(cut.saturating_sub(8))?;
        let chunk = u32::try_from(cut.saturating_sub(20))?;

        for (at, byte) in (4..8).zip(riff.to_le_bytes()).chain((16..20).zip(chunk.to_le_bytes())) {
            match short.get_mut(at) {
                Some(slot) => *slot = byte,
                None => return Err(Box::<dyn Error>::from("the cut is shorter than the header")),
            }
        }

        assert_eq!(decoded(&short).map(|picture| picture.size), Err(WebpError::Truncated), "cut at {cut}");
    }

    Ok(())
}

//! The picture drawn the way a person holds the machine, laid onto a panel
//! that may be mounted a quarter turn from it.
//!
//! The Legion Go's panel is taller than it is wide and sits in the machine on
//! its side. A compositor turns every frame for it; with no compositor, this
//! does, once per frame and by the same rule `console_screen::Screen` walks a
//! point by for transform 1: what is across the picture is down the panel, and
//! what is down the picture runs back across it. A finger on the panel is
//! reported the panel's way up, so [`on_the_picture`] walks it back the other
//! way.
//!
//! A frame is laid a band at a time. The last frame shown is kept, the new one
//! is held against it, and only the rows between the first and the last that
//! differ are turned onto the panel -- a finger dragged between two dots
//! changes the rows the line runs through and nothing else, and turning the
//! whole picture for it was most of what a frame cost. The turning walks the
//! panel's rows in order, so what reaches the mapped buffer is written the
//! way that memory wants it, a row at a time.

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::index;
use console_screen::{Mounted, QUARTER};

const PIXEL: u32 = 4;

pub fn drawn_at(panel: Size<u32>) -> Result<Size<u32>, Never> {
    let Ok(mounted) = Mounted::of(panel);

    Ok(match mounted {
        Mounted::Sideways => Size { width: panel.height, height: panel.width },
        Mounted::Upright => panel,
    })
}

pub fn on_the_picture(panel: Size<u32>, share: Point<f64>) -> Result<Point<f64>, Never> {
    let Ok(mounted) = Mounted::of(panel);
    let Ok(drawn) = drawn_at(panel);
    let (wide, tall) = (f64::from(drawn.width), f64::from(drawn.height));

    Ok(match mounted {
        Mounted::Sideways => Point { x: (1.0 - share.y) * wide, y: share.x * tall },
        Mounted::Upright => Point { x: share.x * wide, y: share.y * tall },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rows {
    pub from: u32,
    pub to: u32,
}

pub fn changed(before: &[u8], now: &[u8], drawn: Size<u32>) -> Result<Option<Rows>, Never> {
    let every = Rows { from: 0, to: drawn.height };

    match before.len() == now.len() {
        true => {}
        false => return Ok(Some(every)),
    }

    let Ok(across) = index(drawn.width.saturating_mul(PIXEL));
    let pairs = || before.chunks_exact(across.max(1)).zip(now.chunks_exact(across.max(1)));
    let differing = |(row, (was, is)): (u32, (&[u8], &[u8]))| match was == is {
        true => None,
        false => Some(row),
    };
    let first = (0_u32..).zip(pairs()).find_map(differing);
    let last = (0_u32..).zip(pairs()).filter_map(differing).last();

    Ok(match (first, last) {
        (Some(from), Some(last)) => Some(Rows { from, to: last.saturating_add(1) }),
        (None, _) | (_, None) => None,
    })
}

pub fn laid_into(picture: &[u8], panel: Size<u32>, rows: Rows, target: &mut [u8], pitch: u32) -> Result<(), Never> {
    let Ok(mounted) = Mounted::of(panel);
    let Ok(transform) = mounted.transform();
    let Ok(drawn) = drawn_at(panel);
    let Ok(pixel_long) = index(PIXEL);
    let Ok(pitch) = index(pitch);
    let Ok(across) = index(drawn.width.saturating_mul(PIXEL));
    let Ok(from) = index(rows.from);
    let Ok(many) = index(rows.to.saturating_sub(rows.from));

    match transform == QUARTER {
        true => {}
        false => {
            for (row, laid) in picture.chunks_exact(across.max(1)).zip(target.chunks_mut(pitch.max(1))).skip(from).take(many) {
                match laid.get_mut(..across) {
                    Some(laid) => laid.copy_from_slice(row),
                    None => {}
                }
            }

            return Ok(());
        }
    }

    for (along, laid) in (0_u32..drawn.width).rev().zip(target.chunks_mut(pitch.max(1))) {
        for down in rows.from..rows.to {
            let Ok(taken) = index(down.saturating_mul(drawn.width).saturating_add(along).saturating_mul(PIXEL));
            let Ok(put) = index(down.saturating_mul(PIXEL));
            let pixel = picture.get(taken..taken.saturating_add(pixel_long));
            let spot = laid.get_mut(put..put.saturating_add(pixel_long));

            match (pixel, spot) {
                (Some(pixel), Some(spot)) => spot.copy_from_slice(pixel),
                (None, _) | (_, None) => {}
            }
        }
    }

    Ok(())
}

pub fn rotate(picture: &[u8], panel: Size<u32>) -> Result<Vec<u8>, Never> {
    let Ok(drawn) = drawn_at(panel);
    let mut laid = vec![0_u8; picture.len()];
    let Ok(()) = laid_into(picture, panel, Rows { from: 0, to: drawn.height }, &mut laid, panel.width.saturating_mul(PIXEL));

    Ok(laid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_panel_is_drawn_on_as_it_is() {
        let panel = Size { width: 2, height: 1 };
        let picture = vec![1, 1, 1, 1, 2, 2, 2, 2];
        let Ok(laid) = rotate(&picture, panel);

        assert_eq!(laid, picture);
    }

    #[test]
    fn a_tall_panel_gets_the_picture_a_quarter_turn_round() {
        let panel = Size { width: 1, height: 2 };
        let Ok(drawn) = drawn_at(panel);
        let left_then_right = vec![1, 1, 1, 1, 2, 2, 2, 2];
        let Ok(laid) = rotate(&left_then_right, panel);

        assert_eq!(drawn, Size { width: 2, height: 1 });
        assert_eq!(laid, vec![2, 2, 2, 2, 1, 1, 1, 1]);
    }

    #[test]
    fn a_finger_on_a_turned_panel_lands_on_the_pixel_drawn_under_it() -> Result<(), &'static str> {
        let panel = Size { width: 3, height: 5 };
        let Ok(drawn) = drawn_at(panel);
        let Ok(long) = console_core_number_conversion::index(5_u32.saturating_mul(3).saturating_mul(4));
        let Ok(from) = console_core_number_conversion::index(2_u32.saturating_mul(5).saturating_add(1).saturating_mul(4));
        let mut picture = vec![0_u8; long];

        for byte in picture.iter_mut().skip(from).take(4) {
            *byte = 9;
        }

        let Ok(laid) = rotate(&picture, panel);
        let (lit, _) = (0_u32..)
            .zip(laid.as_chunks::<4>().0)
            .find(|(_, pixel)| pixel.first() == Some(&9))
            .ok_or("the pixel went nowhere")?;
        let (column, row) = (lit.rem_euclid(3), lit.div_euclid(3));
        let share = Point { x: (f64::from(column) + 0.5) / 3.0, y: (f64::from(row) + 0.5) / 5.0 };
        let Ok(found) = on_the_picture(panel, share);

        assert_eq!(drawn, Size { width: 5, height: 3 });
        assert_eq!((found.x.floor(), found.y.floor()), (1.0, 2.0));

        Ok(())
    }

    #[test]
    fn only_the_rows_that_differ_are_a_change() {
        let drawn = Size { width: 2, height: 4 };
        let before = vec![0_u8; 32];
        let mut now = before.clone();

        for byte in now.iter_mut().skip(8).take(12) {
            *byte = 1;
        }

        let Ok(band) = changed(&before, &now, drawn);
        let Ok(still) = changed(&before, &before, drawn);

        assert_eq!(band, Some(Rows { from: 1, to: 3 }));
        assert_eq!(still, None);
    }

    #[test]
    fn a_band_laid_on_a_turned_panel_is_the_whole_turn_there_and_nothing_elsewhere() {
        let panel = Size { width: 2, height: 3 };
        let picture: Vec<u8> = (1_u8..=24).collect();
        let Ok(whole) = rotate(&picture, panel);
        let mut laid = vec![0_u8; 24];
        let Ok(()) = laid_into(&picture, panel, Rows { from: 1, to: 2 }, &mut laid, 8);

        for ((whole, laid), column) in whole.as_chunks::<4>().0.iter().zip(laid.as_chunks::<4>().0.iter()).zip([0_u8, 1].into_iter().cycle()) {
            match column {
                1 => assert_eq!(whole, laid),
                _ => assert_eq!(*laid, [0, 0, 0, 0]),
            }
        }
    }
}

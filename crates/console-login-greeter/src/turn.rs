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

pub fn turned(picture: &[u8], panel: Size<u32>) -> Result<Vec<u8>, Never> {
    let Ok(mounted) = Mounted::of(panel);
    let Ok(transform) = mounted.transform();

    match transform == QUARTER {
        true => {}
        false => return Ok(picture.to_vec()),
    }

    let Ok(drawn) = drawn_at(panel);
    let Ok(across) = index(drawn.width);
    let Ok(pixel_long) = index(PIXEL);
    let mut laid = vec![0_u8; picture.len()];

    for (down, row) in (0_u32..).zip(picture.chunks_exact(across.saturating_mul(pixel_long).max(1))) {
        for (along, pixel) in (0_u32..).zip(row.chunks_exact(pixel_long)) {
            let Ok(at) = index(
                drawn.width.saturating_sub(1).saturating_sub(along).saturating_mul(panel.width).saturating_add(down).saturating_mul(PIXEL),
            );

            match laid.get_mut(at..at.saturating_add(pixel_long)) {
                Some(target) => target.copy_from_slice(pixel),
                None => {}
            }
        }
    }

    Ok(laid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_panel_is_drawn_on_as_it_is() {
        let panel = Size { width: 2, height: 1 };
        let picture = vec![1, 1, 1, 1, 2, 2, 2, 2];
        let Ok(laid) = turned(&picture, panel);

        assert_eq!(laid, picture);
    }

    #[test]
    fn a_tall_panel_gets_the_picture_a_quarter_turn_round() {
        let panel = Size { width: 1, height: 2 };
        let Ok(drawn) = drawn_at(panel);
        let left_then_right = vec![1, 1, 1, 1, 2, 2, 2, 2];
        let Ok(laid) = turned(&left_then_right, panel);

        assert_eq!(drawn, Size { width: 2, height: 1 });
        assert_eq!(laid, vec![2, 2, 2, 2, 1, 1, 1, 1]);
    }

    #[test]
    fn a_finger_on_a_turned_panel_lands_on_the_pixel_drawn_under_it() {
        let panel = Size { width: 3, height: 5 };
        let Ok(drawn) = drawn_at(panel);
        let mut picture = vec![0_u8; 5 * 3 * 4];

        for byte in picture.iter_mut().skip((2 * 5 + 1) * 4).take(4) {
            *byte = 9;
        }

        let Ok(laid) = turned(&picture, panel);
        let lit = match (0_u32..).zip(laid.chunks_exact(4)).find(|(_, pixel)| pixel.first() == Some(&9)) {
            Some((lit, _)) => lit,
            None => panic!("the pixel went nowhere"),
        };
        let (column, row) = (lit % 3, lit / 3);
        let share = Point { x: (f64::from(column) + 0.5) / 3.0, y: (f64::from(row) + 0.5) / 5.0 };
        let Ok(found) = on_the_picture(panel, share);

        assert_eq!(drawn, Size { width: 5, height: 3 });
        assert_eq!((found.x.floor(), found.y.floor()), (1.0, 2.0));
    }
}

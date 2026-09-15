//! How much bigger a pixel is than a point, said in a whole number.
//!
//! This device's panel is driven at 2.5, and `wl_surface.set_buffer_scale`
//! takes an integer. That is the whole of the problem: a surface that says 2
//! is drawn at four ninths of the pixels the screen has and comes out soft, and
//! one that says 3 is drawn at too many and is scaled back down. GTK is what
//! absorbs the difference today, and it is the only thing it was still doing
//! for this desktop that nothing here could do without it.
//!
//! `wp_fractional_scale_v1` answers in a hundred and twentieths, because 120 is
//! divisible by every denominator a display has ever been sold with -- halves,
//! thirds, quarters, fifths, sixths, eighths. 2.5 is 300 of them exactly, with
//! no float anywhere between the compositor saying it and the buffer being cut
//! to it. So that is what is kept, rather than the `f64` `console-screen` reads
//! out of the compositor's file: a ratio that arrived as a whole number and is
//! spent as one cannot drift between the two.
//!
//! Rounding is up, and it has to be. A buffer a pixel short of the destination
//! is a row of whatever was behind it down one edge of the surface, which is
//! the kind of fault that shows up on one machine at one scale and on nobody
//! else's.

use console_core_geometry::Size;
use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale(u32);

impl Scale {
    pub const WHOLE: u32 = 120;

    pub const ONE: Scale = Scale(Scale::WHOLE);

    pub fn of(hundred_twentieths: u32) -> Result<Scale, Never> {
        Ok(Scale(hundred_twentieths.max(1)))
    }

    pub fn hundred_twentieths(self) -> Result<u32, Never> {
        Ok(self.0)
    }

    pub fn device(self, logical: Size<u32>) -> Result<Size<u32>, Never> {
        let Ok(wide) = self.along(logical.wide);
        let Ok(tall) = self.along(logical.tall);

        Ok(Size { wide, tall })
    }

    fn along(self, logical: u32) -> Result<u32, Never> {
        Ok(logical.saturating_mul(self.0).div_ceil(Scale::WHOLE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_this_device_is_driven_at_is_a_whole_number_of_hundred_twentieths() {
        let Ok(two_and_a_half) = Scale::of(300);
        let Ok(device) = two_and_a_half.device(Size { wide: 768, tall: 480 });

        assert_eq!(device, Size { wide: 1920, tall: 1200 });
    }

    #[test]
    fn a_scale_of_one_leaves_the_size_alone() {
        let Ok(device) = Scale::ONE.device(Size { wide: 320, tall: 44 });

        assert_eq!(device, Size { wide: 320, tall: 44 });
    }

    #[test]
    fn a_size_that_does_not_divide_rounds_up_rather_than_leaving_an_edge_short() {
        let Ok(third) = Scale::of(160);
        let Ok(device) = third.device(Size { wide: 101, tall: 1 });

        assert_eq!(device, Size { wide: 135, tall: 2 });
    }

    #[test]
    fn a_compositor_that_answers_nothing_is_not_a_surface_of_no_pixels() {
        let Ok(none) = Scale::of(0);
        let Ok(device) = none.device(Size { wide: 320, tall: 44 });

        assert_eq!(device, Size { wide: 3, tall: 1 });
    }
}

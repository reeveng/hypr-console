//! A loop, made into a picture that rests and then stirs.
//!
//! An artist's wallpaper is a video: nine to twenty-two seconds, thirty frames
//! a second, every frame a whole picture. Played as it is, the compositor
//! redraws the screen thirty times a second for as long as the machine is on,
//! and this machine runs off a battery. `docs/theme.md` has the argument in
//! full; the short of it is that a WebP frame declares how long it lasts and
//! the wallpaper daemon sleeps in `poll()` for exactly that long, so a picture
//! that rests costs nothing at all while it is resting.
//!
//! So the loop is cut down to two things. A still, which is what is on the
//! screen almost all of the time, and a stir, which is a few seconds of the
//! artist's animation played out of it and back into it.
//!
//! Two problems come with cutting a loop short, and both are solved here.
//!
//! The first is where to cut. The stir has to end somewhere it can jump back to
//! the still without the jump being seen, so the slice taken is the one whose
//! last frame is nearest its first: `stir` measures every candidate and takes
//! the closest. A loop of frogs bobbing on water has such a stretch in it about
//! once a cycle, and finding it is cheaper than asking somebody to.
//!
//! The second is size. Every frame at the size of this screen is four megabytes
//! before it is compressed, and a picture made of fifty of them is not a
//! wallpaper, it is a video by another name. But almost nothing in these
//! pictures moves: a campfire flickers, some water shifts, and the rest of the
//! frame is the same paint it was a second ago. `changed` finds the rectangle
//! that actually differs from the frame before it, and only that rectangle is
//! encoded. The frames are muxed to neither blend nor dispose, so a rectangle
//! painted over the still leaves the rest of the still exactly where it was.


use console_never::Never;
use console_number_conversion::{Float, fitted};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patch {
    pub x: u32,
    pub y: u32,
    pub wide: u32,
    pub tall: u32,
}

impl Patch {
    pub fn whole(wide: u32, tall: u32) -> Result<Self, Never> {
        Ok(Patch { x: 0, y: 0, wide, tall })
    }

    pub fn area(&self) -> Result<u64, Never> {
        Ok(u64::from(self.wide).saturating_mul(u64::from(self.tall)))
    }
}

pub fn apart(one: &[u8], other: &[u8]) -> Result<f64, Never> {
    let mut total = 0u64;
    let mut counted = 0u64;

    for (a, b) in one.iter().step_by(7).zip(other.iter().step_by(7)) {
        total = total.saturating_add(u64::from(a.abs_diff(*b)));
        counted = counted.saturating_add(1);
    }

    let Ok(seen) = total.float();
    let Ok(many) = counted.float();

    Ok(match counted {
        0 => 0.0,
        _ => seen / many,
    })
}

pub fn stir(frames: &[Vec<u8>], want: usize) -> Result<(usize, usize), Never> {
    match frames.len() <= want || want == 0 {
        true => return Ok((0, frames.len().saturating_sub(1))),
        false => {},
    }

    let gap = |one: &usize| match (frames.get(*one), frames.get(one.saturating_add(want))) {
        (Some(here), Some(there)) => {
            let Ok(apart) = apart(here, there);

            apart
        }
        (Some(_), None) | (None, _) => f64::INFINITY,
    };

    let best = (0..frames.len().saturating_sub(want))
        .min_by(|one, other| gap(one).total_cmp(&gap(other)))
        .unwrap_or(0);

    Ok((best, best.saturating_add(want)))
}

pub fn changed(before: &[u8], after: &[u8], wide: u32, tolerance: u8) -> Result<Option<Patch>, Never> {
    let Ok(rows) = fitted::<usize, u32>(before.len().saturating_div(3));

    let tall = rows.saturating_div(wide.max(1));
    let (mut left, mut right) = (wide, 0u32);
    let (mut top, mut bottom) = (tall, 0u32);

    for row in 0..tall {
        for column in 0..wide {
            let Ok(at) =
                fitted::<u32, usize>(row.saturating_mul(wide).saturating_add(column).saturating_mul(3));
            let moved =
                match (before.get(at..at.saturating_add(3)), after.get(at..at.saturating_add(3))) {
                (Some(before), Some(after)) => before
                    .iter()
                    .zip(after)
                    .any(|(before, after)| before.abs_diff(*after) > tolerance),
                (Some(_), None) | (None, _) => false,
            };

            match moved {
                true => {
                    left = left.min(column);
                    right = right.max(column);
                    top = top.min(row);
                    bottom = bottom.max(row);
                }
                false => {},
            }
        }
    }

    match left > right {
        true => return Ok(None),
        false => {},
    }

    let x = left & !1;
    let y = top & !1;

    Ok(Some(Patch {
        x,
        y,
        wide: right.saturating_sub(x).saturating_add(1),
        tall: bottom.saturating_sub(y).saturating_add(1),
    }))
}

pub fn cut(frame: &[u8], wide: u32, patch: &Patch) -> Result<Vec<u8>, Never> {
    let area = patch.area()?;

    let Ok(room) = fitted(area.saturating_mul(3));

    let mut out = Vec::with_capacity(room);

    for row in patch.y..patch.y.saturating_add(patch.tall) {
        let Ok(from) =
            fitted::<u32, usize>(row.saturating_mul(wide).saturating_add(patch.x).saturating_mul(3));
        let Ok(across) = fitted::<u32, usize>(patch.wide.saturating_mul(3));

        let wanted = from.saturating_add(across);

        match frame.get(from..wanted) {
            Some(row) => out.extend_from_slice(row),
            None => {},
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(wide: u32, tall: u32, shade: u8) -> Vec<u8> {
        vec![shade; (wide * tall * 3) as usize]
    }

    fn dotted(wide: u32, tall: u32, shade: u8, at: (u32, u32), dot: u8) -> Vec<u8> {
        let mut picture = flat(wide, tall, shade);
        let index = ((at.1 * wide + at.0) * 3) as usize;
        picture[index..index + 3].fill(dot);
        picture
    }

    #[test]
    fn two_pictures_the_same_are_no_distance_apart() {
        assert_eq!(apart(&flat(8, 8, 30), &flat(8, 8, 30)), Ok(0.0));
    }

    #[test]
    fn a_picture_further_from_another_measures_further() {
        let Ok(near) = apart(&flat(8, 8, 30), &flat(8, 8, 40));

        let Ok(far) = apart(&flat(8, 8, 30), &flat(8, 8, 90));

        assert!(near < far, "{near} was not under {far}");
    }

    #[test]
    fn the_stir_is_the_slice_that_ends_nearest_where_it_began() {
        let shades = [10, 90, 200, 90, 10, 90, 200];
        let frames: Vec<Vec<u8>> = shades.iter().map(|shade| flat(4, 4, *shade)).collect();
        assert_eq!(stir(&frames, 4), Ok((0, 4)));
    }

    #[test]
    fn a_loop_shorter_than_the_stir_asked_for_is_taken_whole() {
        let frames: Vec<Vec<u8>> = (0..3).map(|shade| flat(4, 4, shade * 20)).collect();
        assert_eq!(stir(&frames, 9), Ok((0, 2)));
    }

    #[test]
    fn nothing_moving_is_no_rectangle_at_all() {
        assert_eq!(changed(&flat(8, 8, 30), &flat(8, 8, 30), 8, 0), Ok(None));
    }

    #[test]
    fn a_change_under_the_tolerance_is_not_a_change() {
        assert_eq!(changed(&flat(8, 8, 30), &flat(8, 8, 32), 8, 3), Ok(None));

        let Ok(moved) = changed(&flat(8, 8, 30), &flat(8, 8, 40), 8, 3);

        assert!(moved.is_some());
    }

    #[test]
    fn the_rectangle_is_the_bounds_of_what_moved_on_an_even_corner() {
        let moved = changed(&flat(8, 8, 30), &dotted(8, 8, 30, (5, 3), 200), 8, 0);

        assert_eq!(moved, Ok(Some(Patch { x: 4, y: 2, wide: 2, tall: 2 })));
    }

    #[test]
    fn a_rectangle_cut_out_holds_that_rectangle_and_no_more() {
        let picture = dotted(8, 8, 30, (5, 3), 200);
        let patch = Patch { x: 4, y: 2, wide: 2, tall: 2 };
        let Ok(taken) = cut(&picture, 8, &patch);

        assert_eq!(taken.len(), 2 * 2 * 3);
        assert_eq!(&taken[9..12], &[200, 200, 200]);
    }
}

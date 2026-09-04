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


use console_number::{Float, fitted};

/// A part of the picture.
///
/// The offsets are even because that is how a WebP animation stores them: the
/// format keeps them halved, so an odd offset is not a thing the container can
/// say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patch {
    pub x: u32,
    pub y: u32,
    pub wide: u32,
    pub tall: u32,
}

impl Patch {
    /// The whole picture.
    pub fn whole(wide: u32, tall: u32) -> Self {
        Patch { x: 0, y: 0, wide, tall }
    }

    /// How many pixels it covers.
    pub fn area(&self) -> u64 {
        u64::from(self.wide) * u64::from(self.tall)
    }
}

/// How far apart two pictures are, as a mean over every channel.
///
/// Sampled rather than read in full: a picture at the size of this screen is
/// twelve million numbers, every candidate slice of a loop is a pair of them,
/// and the answer this is used for is which of several is smallest. Every
/// seventh channel is enough to rank them and is seven times less to read.
pub fn apart(one: &[u8], other: &[u8]) -> f64 {
    let mut total = 0u64;
    let mut counted = 0u64;

    for (a, b) in one.iter().step_by(7).zip(other.iter().step_by(7)) {
        total += u64::from(a.abs_diff(*b));
        counted += 1;
    }

    match counted {
        0 => 0.0,
        _ => total.float() / counted.float(),
    }
}

/// The stretch of a loop that comes back closest to where it started.
///
/// `want` is how many frames the stir should be. Every slice of that length is
/// measured by how far its last frame is from its first, and the closest wins,
/// because that gap is the jump the eye sees when the picture snaps back to
/// resting. A loop shorter than what was asked for is returned whole: it
/// already ends where it began, which is the best any slice could do.
pub fn stir(frames: &[Vec<u8>], want: usize) -> (usize, usize) {
    if frames.len() <= want || want == 0 {
        return (0, frames.len().saturating_sub(1));
    }

    let best = (0..frames.len() - want)
        .min_by(|one, other| {
            apart(&frames[*one], &frames[one + want])
                .total_cmp(&apart(&frames[*other], &frames[other + want]))
        })
        .unwrap_or(0);
    (best, best + want)
}

/// The rectangle that differs between two frames, if anything does.
///
/// `tolerance` is how much a channel may move without counting as movement.
/// Video arrives from a lossy codec, so a still region is never bit for bit
/// still, and a tolerance of zero would find the whole frame changed every
/// time and undo the entire saving.
pub fn changed(before: &[u8], after: &[u8], wide: u32, tolerance: u8) -> Option<Patch> {
    let tall = fitted::<usize, u32>(before.len() / 3) / wide.max(1);
    let (mut left, mut right) = (wide, 0u32);
    let (mut top, mut bottom) = (tall, 0u32);

    for row in 0..tall {
        for column in 0..wide {
            let at: usize = fitted((row * wide + column) * 3);
            let moved = (0..3).any(|channel| {
                before[at + channel].abs_diff(after[at + channel]) > tolerance
            });

            if moved {
                left = left.min(column);
                right = right.max(column);
                top = top.min(row);
                bottom = bottom.max(row);
            }
        }
    }

    if left > right {
        return None;
    }

    // Grown outwards to an even corner, which is the only corner the container
    // can name, and never past the edge of the picture.
    let x = left & !1;
    let y = top & !1;
    Some(Patch { x, y, wide: right - x + 1, tall: bottom - y + 1 })
}

/// One rectangle of a frame, copied out on its own.
pub fn cut(frame: &[u8], wide: u32, patch: &Patch) -> Vec<u8> {
    let mut out = Vec::with_capacity(fitted(patch.area() * 3));

    for row in patch.y..patch.y + patch.tall {
        let from: usize = fitted((row * wide + patch.x) * 3);
        out.extend_from_slice(&frame[from..from + fitted::<u32, usize>(patch.wide * 3)]);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture of one colour, at a size the tests can hold in their heads.
    fn flat(wide: u32, tall: u32, shade: u8) -> Vec<u8> {
        vec![shade; (wide * tall * 3) as usize]
    }

    /// The same, with one pixel painted a different colour.
    fn dotted(wide: u32, tall: u32, shade: u8, at: (u32, u32), dot: u8) -> Vec<u8> {
        let mut picture = flat(wide, tall, shade);
        let index = ((at.1 * wide + at.0) * 3) as usize;
        picture[index..index + 3].fill(dot);
        picture
    }

    #[test]
    fn two_pictures_the_same_are_no_distance_apart() {
        assert_eq!(apart(&flat(8, 8, 30), &flat(8, 8, 30)), 0.0);
    }

    #[test]
    fn a_picture_further_from_another_measures_further() {
        let near = apart(&flat(8, 8, 30), &flat(8, 8, 40));
        let far = apart(&flat(8, 8, 30), &flat(8, 8, 90));
        assert!(near < far, "{near} was not under {far}");
    }

    /// The whole reason the slice is chosen rather than taken from the front:
    /// a loop that wanders away and comes back should be cut where it has come
    /// back, so the snap to resting is not seen.
    #[test]
    fn the_stir_is_the_slice_that_ends_nearest_where_it_began() {
        let shades = [10, 90, 200, 90, 10, 90, 200];
        let frames: Vec<Vec<u8>> = shades.iter().map(|shade| flat(4, 4, *shade)).collect();
        assert_eq!(stir(&frames, 4), (0, 4));
    }

    #[test]
    fn a_loop_shorter_than_the_stir_asked_for_is_taken_whole() {
        let frames: Vec<Vec<u8>> = (0..3).map(|shade| flat(4, 4, shade * 20)).collect();
        assert_eq!(stir(&frames, 9), (0, 2));
    }

    #[test]
    fn nothing_moving_is_no_rectangle_at_all() {
        assert_eq!(changed(&flat(8, 8, 30), &flat(8, 8, 30), 8, 0), None);
    }

    /// A codec's own noise is not movement, and a tolerance is what says so.
    #[test]
    fn a_change_under_the_tolerance_is_not_a_change() {
        assert_eq!(changed(&flat(8, 8, 30), &flat(8, 8, 32), 8, 3), None);
        assert!(changed(&flat(8, 8, 30), &flat(8, 8, 40), 8, 3).is_some());
    }

    /// One pixel moving gives a rectangle around that pixel, with its corner
    /// pulled back to the even offset a WebP animation can name.
    #[test]
    fn the_rectangle_is_the_bounds_of_what_moved_on_an_even_corner() {
        let moved = changed(&flat(8, 8, 30), &dotted(8, 8, 30, (5, 3), 200), 8, 0);
        assert_eq!(moved, Some(Patch { x: 4, y: 2, wide: 2, tall: 2 }));
    }

    #[test]
    fn a_rectangle_cut_out_holds_that_rectangle_and_no_more() {
        let picture = dotted(8, 8, 30, (5, 3), 200);
        let patch = Patch { x: 4, y: 2, wide: 2, tall: 2 };
        let taken = cut(&picture, 8, &patch);
        assert_eq!(taken.len(), 2 * 2 * 3);
        // The dot is the second pixel of the second row of the cut.
        assert_eq!(&taken[9..12], &[200, 200, 200]);
    }
}

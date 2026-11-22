//! The pixels a context is made of, rolled along a row rather than gathered again.
//!
//! A context is a few pixels around the one being decoded, one bit each, and
//! most of them are the neighbours the pixel before had, a column further
//! left. `Neighbourhood` works out once which bits that is true of, for the
//! taps it was given, and a step along a row shifts those and reads only the
//! rest. The taps are written lowest bit first, so a tap is rolled exactly
//! when the one below it is the tap one column to its right, whatever the
//! adaptive pixels were moved to.

use console_core_geometry::Point;
use console_core_never::Never;

use crate::bitmap::Bitmap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
  Decoded,
  Reference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tap {
  pub(crate) source: Source,
  pub(crate) at: Point<i32>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Reference<'a> {
  pub(crate) bitmap: &'a Bitmap,
  pub(crate) shift: Point<i32>,
}

#[derive(Debug, Clone)]
pub(crate) struct Neighbourhood {
  taps: Vec<Tap>,
  keep: u32,
  fresh: Vec<(u32, Tap)>,
}

impl Neighbourhood {
  pub(crate) fn new(taps: Vec<Tap>) -> Result<Neighbourhood, Never> {
    let mut keep = 0u32;
    let mut fresh = Vec::new();
    let lower = std::iter::once(None).chain(taps.iter().map(Some));

    for ((bit, tap), below) in (0u32..).zip(&taps).zip(lower) {
      let rolled = match below {
        Some(below_2) => {
          below_2.source == tap.source
            && below_2.at.x == tap.at.x.saturating_add(1)
            && below_2.at.y == tap.at.y
        }
        None => false,
      };

      match ::core::primitive::bool::then_some(rolled, ()) {
        ::core::option::Option::Some(()) => keep |= 1u32.wrapping_shl(bit),
        ::core::option::Option::None => fresh.push((bit, *tap)),
      }
    }

    Ok(Neighbourhood { taps, keep, fresh })
  }

  pub(crate) fn first(
    &self,
    at: Point<i32>,
    decoded: &Bitmap,
    reference: Option<&Reference<'_>>,
  ) -> Result<u32, Never> {
    let mut context = 0u32;

    for (bit, tap) in (0u32..).zip(&self.taps) {
      let Ok(pixel) = read(tap, at, decoded, reference);

      context |= pixel.wrapping_shl(bit);
    }

    Ok(context)
  }

  pub(crate) fn next(
    &self,
    previous: u32,
    at: Point<i32>,
    decoded: &Bitmap,
    reference: Option<&Reference<'_>>,
  ) -> Result<u32, Never> {
    let mut context = previous.wrapping_shl(1) & self.keep;

    for (bit, tap) in &self.fresh {
      let Ok(pixel) = read(tap, at, decoded, reference);

      context |= pixel.wrapping_shl(*bit);
    }

    Ok(context)
  }
}

fn read(
  tap: &Tap,
  at: Point<i32>,
  decoded: &Bitmap,
  reference: Option<&Reference<'_>>,
) -> Result<u32, Never> {
  let place = Point {
    x: at.x.saturating_add(tap.at.x),
    y: at.y.saturating_add(tap.at.y),
  };

  Ok(match (tap.source, reference) {
    (Source::Decoded, _) => {
      let Ok(pixel) = decoded.get(place);

      u32::from(pixel)
    }
    (Source::Reference, Some(reference_2)) => {
      let moved = Point {
        x: place.x.saturating_sub(reference_2.shift.x),
        y: place.y.saturating_sub(reference_2.shift.y),
      };
      let Ok(pixel) = reference_2.bitmap.get(moved);

      u32::from(pixel)
    }
    (Source::Reference, None) => 0,
  })
}

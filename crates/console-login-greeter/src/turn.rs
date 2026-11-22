//! The picture drawn the way a person holds the machine, laid onto a panel
//! that may be mounted a quarter turn from it.
//!
//! The Legion Go's panel is taller than it is wide and sits in the machine on
//! its side. A compositor turns every frame for it; with no compositor, this
//! does, once per frame and by the rule `console-core-screen-orientation`
//! walks every point by: what is across the picture is down the panel, and
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
//! way that memory wants it, a row at a time. The band is a stretch of the
//! picture, and the panel rows it lands on are asked of the same rule rather
//! than worked out here for the one way up this machine is mounted.

use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_screen_orientation::{Mounted, Transform};

const PIXEL: u32 = 4;

fn mounting(panel: Size<u32>) -> Result<Transform, Never> {
  let Ok(mounted) = Mounted::of(panel);

  mounted.transform()
}

pub fn drawn_at(panel: Size<u32>) -> Result<Size<u32>, Never> {
  let Ok(transform) = mounting(panel);

  transform.picture_size(panel)
}

pub fn on_the_picture(panel: Size<u32>, share: Point<f64>) -> Result<Point<f64>, Never> {
  let Ok(transform) = mounting(panel);
  let Ok(drawn) = transform.picture_size(panel);
  let Ok(picture) = transform.onto_picture(share);

  Ok(Point {
    x: picture.x * f64::from(drawn.width),
    y: picture.y * f64::from(drawn.height),
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rows {
  pub from: u32,
  pub to: u32,
}

pub fn changed(before: &[u8], now: &[u8], drawn: Size<u32>) -> Result<Option<Rows>, Never> {
  let every = Rows {
    from: 0,
    to: drawn.height,
  };

  match ::core::primitive::bool::then_some(before.len() == now.len(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(Some(every)),
  }

  let Ok(across) = index(drawn.width.saturating_mul(PIXEL));
  let pairs = || {
    before
      .chunks_exact(across.max(1))
      .zip(now.chunks_exact(across.max(1)))
  };
  let differing =
    |(row, (was, is)): (u32, (&[u8], &[u8]))| match ::core::primitive::bool::then_some(
      was == is,
      (),
    ) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => Some(row),
    };
  let first = (0_u32..).zip(pairs()).find_map(differing);
  let last = (0_u32..).zip(pairs()).filter_map(differing).last();

  Ok(match (first, last) {
    (Some(from), Some(last_2)) => Some(Rows {
      from,
      to: last_2.saturating_add(1),
    }),
    (None, _) | (_, None) => None,
  })
}

#[derive(Debug, Clone, Copy)]
struct Onto {
  transform: Transform,
  panel: Size<u32>,
  pitch: u32,
}

pub fn laid_into(
  picture: &[u8],
  panel: Size<u32>,
  rows: Rows,
  target: &mut [u8],
  pitch: u32,
) -> Result<(), Never> {
  let Ok(transform) = mounting(panel);

  laid_at(
    picture,
    Onto {
      transform,
      panel,
      pitch,
    },
    rows,
    target,
  )
}

fn laid_at(picture: &[u8], onto: Onto, rows: Rows, target: &mut [u8]) -> Result<(), Never> {
  match onto.transform {
    Transform::Normal => copied(picture, onto, rows, target),
    Transform::Degrees90 | Transform::Degrees180 | Transform::Degrees270 => {
      turned(picture, onto, rows, target)
    }
  }
}

fn copied(picture: &[u8], onto: Onto, rows: Rows, target: &mut [u8]) -> Result<(), Never> {
  let Ok(pitch) = index(onto.pitch);
  let Ok(across) = index(onto.panel.width.saturating_mul(PIXEL));
  let Ok(from) = index(rows.from);
  let Ok(many) = index(rows.to.saturating_sub(rows.from));

  for (row, laid) in picture
    .chunks_exact(across.max(1))
    .zip(target.chunks_mut(pitch.max(1)))
    .skip(from)
    .take(many)
  {
    match laid.get_mut(..across) {
      Some(visible) => {
        for (destination_byte, source_byte) in visible.iter_mut().zip(row) {
          *destination_byte = *source_byte;
        }
      }
      None => {}
    }
  }

  Ok(())
}

fn turned(picture: &[u8], onto: Onto, rows: Rows, target: &mut [u8]) -> Result<(), Never> {
  let Onto {
    transform,
    panel,
    pitch,
  } = onto;
  let Ok(drawn) = transform.picture_size(panel);
  let band = Rectangle {
    origin: Point { x: 0, y: rows.from },
    size: Size {
      width: drawn.width,
      height: rows.to.saturating_sub(rows.from),
    },
  };
  let Ok(covered) = transform.rectangle_on_panel(panel, band);
  let Ok(pitch_2) = index(pitch);
  let Ok(from) = index(covered.origin.y);
  let Ok(many) = index(covered.size.height);
  let along = covered.origin.x..covered.origin.x.saturating_add(covered.size.width);

  for (down, laid) in (0_u32..)
    .zip(target.chunks_mut(pitch_2.max(1)))
    .skip(from)
    .take(many)
  {
    for across in along.clone() {
      let Ok(under) = transform.pixel_on_picture(panel, Point { x: across, y: down });
      let Ok(taken) = index(
        under
          .y
          .saturating_mul(drawn.width)
          .saturating_add(under.x)
          .saturating_mul(PIXEL),
      );
      let Ok(put) = index(across.saturating_mul(PIXEL));
      let pixel = picture.get(taken..).and_then(<[u8]>::first_chunk::<4>);
      let spot = laid.get_mut(put..).and_then(<[u8]>::first_chunk_mut::<4>);

      match (pixel, spot) {
        (Some(pixel_2), Some(spot_2)) => *spot_2 = *pixel_2,
        (None, _) | (_, None) => {}
      }
    }
  }

  Ok(())
}

pub fn rotate(picture: &[u8], panel: Size<u32>) -> Result<Vec<u8>, Never> {
  let Ok(drawn) = drawn_at(panel);
  let mut laid = vec![0_u8; picture.len()];
  let Ok(()) = laid_into(
    picture,
    panel,
    Rows {
      from: 0,
      to: drawn.height,
    },
    &mut laid,
    panel.width.saturating_mul(PIXEL),
  );

  Ok(laid)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_wide_panel_is_drawn_on_as_it_is() {
    let panel = Size {
      width: 2,
      height: 1,
    };
    let picture = vec![1, 1, 1, 1, 2, 2, 2, 2];
    let Ok(laid) = rotate(&picture, panel);

    assert_eq!(laid, picture);
  }

  #[test]
  fn a_tall_panel_gets_the_picture_a_quarter_turn_round() {
    let panel = Size {
      width: 1,
      height: 2,
    };
    let Ok(drawn) = drawn_at(panel);
    let left_then_right = vec![1, 1, 1, 1, 2, 2, 2, 2];
    let Ok(laid) = rotate(&left_then_right, panel);

    assert_eq!(
      drawn,
      Size {
        width: 2,
        height: 1
      }
    );
    assert_eq!(laid, vec![2, 2, 2, 2, 1, 1, 1, 1]);
  }

  #[test]
  fn a_finger_on_a_turned_panel_lands_on_the_pixel_drawn_under_it() -> Result<(), &'static str> {
    let panel = Size {
      width: 3,
      height: 5,
    };
    let Ok(drawn) = drawn_at(panel);
    let Ok(from) = console_core_number_conversion::index(
      2_u32.saturating_mul(5).saturating_add(1).saturating_mul(4),
    );
    let mut picture = [0_u8; 60];

    for byte in picture.iter_mut().skip(from).take(4) {
      *byte = 9;
    }

    let Ok(laid) = rotate(&picture, panel);
    let (lit, _) = (0_u32..)
      .zip(laid.as_chunks::<4>().0)
      .find(|(_, pixel)| pixel.first() == Some(&9))
      .ok_or("the pixel went nowhere")?;
    let (column, row) = (lit.rem_euclid(3), lit.div_euclid(3));
    let share = Point {
      x: (f64::from(column) + 0.5) / 3.0,
      y: (f64::from(row) + 0.5) / 5.0,
    };
    let Ok(found) = on_the_picture(panel, share);

    assert_eq!(
      drawn,
      Size {
        width: 5,
        height: 3
      }
    );
    assert_eq!((found.x.floor(), found.y.floor()), (1.0, 2.0));

    Ok(())
  }

  #[test]
  fn only_the_rows_that_differ_are_a_change() {
    let drawn = Size {
      width: 2,
      height: 4,
    };
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
  fn a_band_laid_at_any_turn_is_the_picture_under_each_pixel_of_it_and_nothing_else() {
    let panel = Size {
      width: 3,
      height: 5,
    };
    let picture: Vec<u8> = (1_u8..=60).collect();
    let rows = Rows { from: 1, to: 3 };

    for transform in console_core_screen_orientation::EVERY {
      let Ok(drawn) = transform.picture_size(panel);
      let mut laid = vec![0_u8; 60];
      let Ok(()) = laid_at(
        &picture,
        Onto {
          transform,
          panel,
          pitch: 12,
        },
        rows,
        &mut laid,
      );

      for ((down, across), spot) in (0_u32..5)
        .flat_map(|down| (0_u32..3).map(move |across| (down, across)))
        .zip(laid.as_chunks::<4>().0)
      {
        let Ok(under) = transform.pixel_on_picture(panel, Point { x: across, y: down });
        let picked = under
          .y
          .saturating_mul(drawn.width)
          .saturating_add(under.x)
          .saturating_mul(PIXEL)
          .saturating_add(1);
        let Ok(first) = console_core_number_conversion::fitted::<u32, u8>(picked);

        match ::core::primitive::bool::then_some((rows.from..rows.to).contains(&under.y), ()) {
          ::core::option::Option::Some(()) => assert_eq!(
            spot.first(),
            Some(&first),
            "{transform:?} at {across},{down}"
          ),
          ::core::option::Option::None => assert_eq!(
            *spot,
            [0, 0, 0, 0],
            "{transform:?} drew outside the band at {across},{down}"
          ),
        }
      }
    }
  }

  #[test]
  fn a_zero_width_panel_leaves_the_row_untouched() {
    let mut target = [9_u8; 4];
    let Ok(()) = laid_at(
      &[1, 2, 3, 4],
      Onto {
        transform: Transform::Normal,
        panel: Size {
          width: 0,
          height: 1,
        },
        pitch: 4,
      },
      Rows { from: 0, to: 1 },
      &mut target,
    );

    assert_eq!(target, [9; 4]);
  }

  #[test]
  fn a_band_laid_on_a_turned_panel_is_the_whole_turn_there_and_nothing_elsewhere() {
    let panel = Size {
      width: 2,
      height: 3,
    };
    let picture: Vec<u8> = (1_u8..=24).collect();
    let Ok(whole) = rotate(&picture, panel);
    let mut laid = vec![0_u8; 24];
    let Ok(()) = laid_into(&picture, panel, Rows { from: 1, to: 2 }, &mut laid, 8);

    for ((whole_2, laid_2), column) in whole
      .as_chunks::<4>()
      .0
      .iter()
      .zip(laid.as_chunks::<4>().0.iter())
      .zip([0_u8, 1, 0, 1, 0, 1])
    {
      match column {
        1 => assert_eq!(whole_2, laid_2),
        _ => assert_eq!(*laid_2, [0, 0, 0, 0]),
      }
    }
  }
}

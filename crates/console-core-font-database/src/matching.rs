//! Which of one family's faces a CSS query chooses: the nearest stretch, then style, then weight.
//!
//! CSS fonts level 3 narrows a family's faces a property at a time. It keeps
//! the faces with the nearest stretch, then among those the most wanted style,
//! then the nearest weight, and the first face left is the answer. Every
//! narrowing keeps faces that are equal on that property, so the same choice is
//! the least of one rank per face read in that order -- the stretch's choice and
//! distance, the style's choice, the weight's choice and distance -- and the
//! first face holding the least rank is the face the narrowing leaves first.
//!
//! Nearest is the side the specification looks at first, then the other side,
//! with the distance deciding within a side. A stretch at or below normal looks
//! narrower first, and a weight at or below 500 looks lighter first; a weight
//! asked for from 400 to below 450 looks at 500 before either side, and one from
//! 450 to 500 looks at 400. The style looks for itself, then the other slant,
//! and then upright, and an upright query looks at oblique before italic. This
//! is the order the servo font-kit reader matched in, which this was read from.

use crate::{FaceInfo, Query, Stretch, Style};
use console_core_never::Never;
use core::cmp::Ordering;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Choice {
  First,
  Second,
  Third,
  Fourth,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Nearness {
  choice: Choice,
  apart: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct Rank {
  stretch: Nearness,
  style: Choice,
  weight: Nearness,
}

impl Rank {
  pub(crate) fn of(face: &FaceInfo, query: &Query) -> Result<Self, Never> {
    let Ok(stretch) = stretch_nearness(face, query);
    let Ok(style) = style_choice(face, query);
    let Ok(weight) = weight_nearness(face, query);

    Ok(Rank {
      stretch,
      style,
      weight,
    })
  }
}

fn stretch_nearness(face: &FaceInfo, query: &Query) -> Result<Nearness, Never> {
  let Ok(offered) = face.stretch.to_number();
  let Ok(wanted) = query.stretch.to_number();
  let choice = match (
    query.stretch.cmp(&Stretch::Normal),
    face.stretch.cmp(&query.stretch),
  ) {
    (_, Ordering::Equal) => Choice::First,
    (Ordering::Less | Ordering::Equal, Ordering::Less)
    | (Ordering::Greater, Ordering::Greater) => Choice::Second,
    (Ordering::Less | Ordering::Equal, Ordering::Greater)
    | (Ordering::Greater, Ordering::Less) => Choice::Third,
  };

  Ok(Nearness {
    choice,
    apart: offered.abs_diff(wanted),
  })
}

fn style_choice(face: &FaceInfo, query: &Query) -> Result<Choice, Never> {
  Ok(match (query.style, face.style) {
    (Style::Normal, Style::Normal)
    | (Style::Italic, Style::Italic)
    | (Style::Oblique, Style::Oblique) => Choice::First,
    (Style::Normal, Style::Oblique)
    | (Style::Italic, Style::Oblique)
    | (Style::Oblique, Style::Italic) => Choice::Second,
    (Style::Normal, Style::Italic)
    | (Style::Italic, Style::Normal)
    | (Style::Oblique, Style::Normal) => Choice::Third,
  })
}

fn weight_nearness(face: &FaceInfo, query: &Query) -> Result<Nearness, Never> {
  let offered = face.weight.0;
  let wanted = query.weight.0;
  let neighbour = match wanted {
    400..450 => Some(500),
    450..=500 => Some(400),
    0..400 | 501.. => None,
  };
  let choice = match (
    wanted,
    offered.cmp(&wanted),
    neighbour.filter(|&preferred| preferred == offered),
  ) {
    (_, Ordering::Equal, _) => Choice::First,
    (_, _, Some(_preferred)) => Choice::Second,
    (0..=500, Ordering::Less, None) | (501.., Ordering::Greater, None) => Choice::Third,
    (0..=500, Ordering::Greater, None) | (501.., Ordering::Less, None) => Choice::Fourth,
  };

  Ok(Nearness {
    choice,
    apart: offered.abs_diff(wanted),
  })
}

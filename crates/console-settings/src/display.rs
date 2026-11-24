//! Choices and geometry shared by the Display tab and its settings command.

use console_compositor::Monitor;
use console_compositor::setup::{Mode, ModeMatch, Output, OutputScale};
use console_core_never::Never;
use console_core_number_conversion::{Float, whole_i64};

pub(crate) mod rows;

#[must_use]
pub fn selected<'a>(
  outputs: &'a [Output],
  connector: Option<&str>,
) -> Result<Option<&'a Output>, Never> {
  Ok(match connector {
    Some(connector_2) => outputs
      .iter()
      .find(|output| output.connector == connector_2),
    None => outputs
      .iter()
      .find(|output| output.connector.starts_with("eDP-"))
      .or_else(|| outputs.first()),
  })
}

#[must_use]
pub fn screen(output: &Output) -> Result<Option<console_screen::Screen>, Never> {
  let Ok(screen) = console_screen::from_monitor(&Monitor {
    named: output.connector.clone(),
    size: Some(output.mode),
    refresh: Some(output.refresh.0),
    scale: Some(output.scale.0),
    transform: Some(output.transform),
  });
  Ok(screen)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
  Left,
  Right,
  Above,
  Below,
}

impl Side {
  pub const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Above, Self::Below];

  #[must_use]
  pub fn word(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Left => "Left",
      Self::Right => "Right",
      Self::Above => "Above",
      Self::Below => "Below",
    })
  }
}

#[must_use]
pub fn outputs_for_layout<'a>(
  display: &Output,
  outputs: &'a [Output],
) -> Result<impl Iterator<Item = &'a Output>, Never> {
  let id = display.id;
  Ok(
    outputs
      .iter()
      .filter(move |other| other.id != id && other.mirror.is_none()),
  )
}

#[must_use]
pub fn mirror_sources<'a>(
  display: &Output,
  outputs: &'a [Output],
) -> Result<impl Iterator<Item = &'a Output>, Never> {
  let can_mirror = !outputs.iter().any(|other| other.mirror == Some(display.id));
  let Ok(candidates) = outputs_for_layout(display, outputs);
  Ok(candidates.filter(move |_| can_mirror))
}

#[must_use]
fn extent(display: &Output) -> core::result::Result<(i64, i64), console_core_never::Never> {
  let (mut wide, mut tall) = display.mode;

  match {
    ::core::primitive::bool::then_some(
      ::core::primitive::i64::wrapping_rem(display.transform, 2) != 0,
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      std::mem::swap(&mut wide, &mut tall);
    }
    ::core::option::Option::None => {}
  }

  let Ok(wide_2) = wide.float();
  let Ok(tall_2) = tall.float();
  let Ok(wide_3) = whole_i64(wide_2 / display.scale.0);
  let Ok(tall_3) = whole_i64(tall_2 / display.scale.0);

  Ok((wide_3, tall_3))
}

#[must_use]
pub fn placed(display: &Output, anchor: &Output, side: Side) -> Result<(i64, i64), Never> {
  let (x, y) = anchor.position;
  let Ok((its_width, its_height)) = extent(anchor);
  let Ok((our_width, our_height)) = extent(display);

  Ok(match side {
    Side::Left => (x.saturating_sub(our_width), y),
    Side::Right => (x.saturating_add(its_width), y),
    Side::Above => (x, y.saturating_sub(our_height)),
    Side::Below => (x, y.saturating_add(its_height)),
  })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Placement {
  Available,
  Overlapping,
}

#[must_use]
pub fn placement(
  display: &Output,
  at: (i64, i64),
  outputs: &[Output],
) -> Result<Placement, Never> {
  let Ok((wide, tall)) = extent(display);
  let Ok(mut candidates) = outputs_for_layout(display, outputs);
  let overlap = candidates.any(|other| {
    let (x, y) = other.position;
    let Ok((its_width, its_height)) = extent(other);
    at.0 < x.saturating_add(its_width)
      && x < at.0.saturating_add(wide)
      && at.1 < y.saturating_add(its_height)
      && y < at.1.saturating_add(tall)
  });
  Ok(match overlap.then_some(()) {
    Some(()) => Placement::Overlapping,
    None => Placement::Available,
  })
}

#[must_use]
pub fn extend(display: &Output, outputs: &[Output]) -> Result<Option<(i64, i64)>, Never> {
  let Ok(candidates) = outputs_for_layout(display, outputs);
  let anchor = candidates.max_by_key(|other| {
    other.position.0.saturating_add(
      ({
        let Ok(explicit_value) = extent(other);
        explicit_value
      })
      .0,
    )
  });

  Ok(anchor.map(|other| {
    let Ok(position) = placed(display, other, Side::Right);
    position
  }))
}

#[must_use]
pub fn modes(display: &Output) -> Result<Vec<Mode>, Never> {
  let Ok(current) = display.current_mode();
  let mut modes = display.available_modes.clone();

  let listed = modes.iter().any(|mode| {
    let Ok(agreement) = current.matches(*mode);
    agreement == ModeMatch::Same
  });

  match (!listed).then_some(()) {
    ::core::option::Option::Some(()) => {
      modes.push(current);
    }
    ::core::option::Option::None => {}
  }

  modes.sort_by(|a, b| {
    b.size
      .cmp(&a.size)
      .then_with(|| b.refresh.0.total_cmp(&a.refresh.0))
  });
  modes.dedup();
  Ok(modes)
}

#[must_use]
fn mode_at_resolution(
  display: &Output,
  resolution: (i64, i64),
  modes: &[Mode],
) -> core::result::Result<Option<Mode>, console_core_never::Never> {
  Ok(
    modes
      .iter()
      .copied()
      .filter(|mode| mode.size == resolution)
      .min_by(|a, b| {
        (a.refresh.0 - display.refresh.0)
          .abs()
          .total_cmp(&(b.refresh.0 - display.refresh.0).abs())
      }),
  )
}

#[must_use]
pub fn pixel_aligned_scale(mode: (i64, i64), wanted: OutputScale) -> Result<OutputScale, Never> {
  let (wide, tall) = mode;
  let Ok(wide_2) = wide.float();
  let Ok(tall_2) = tall.float();

  match {
    ::core::primitive::bool::then_some(
      (wide_2 / wanted.0).fract().abs() < f64::EPSILON
        && (tall_2 / wanted.0).fract().abs() < f64::EPSILON,
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return Ok(wanted);
    }
    ::core::option::Option::None => {}
  }

  let at = (wanted.0 * 120.0).round();

  Ok(
    match (0..90)
      .flat_map(|step| [at + f64::from(step), at - f64::from(step)])
      .filter(|step| *step >= 120.0)
      .map(|step| step / 120.0)
      .find(|scale| {
        ::core::matches!(
          ::core::primitive::f64::classify((wide_2 / scale).fract()),
          ::core::num::FpCategory::Zero
        ) && ::core::matches!(
          ::core::primitive::f64::classify((tall_2 / scale).fract()),
          ::core::num::FpCategory::Zero
        )
      }) {
      ::core::option::Option::Some(present) => OutputScale(present),
      ::core::option::Option::None => OutputScale(1.0),
    },
  )
}

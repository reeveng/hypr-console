//! How wide each of the bar's words comes out, which is the text engine's to say.
//!
//! `showing` places against widths and never asks for one, so that where a
//! slab lands can be asserted with no font in the room. This is the one call
//! that needs a font, and it is a module of its own because two bars are drawn
//! out of `showing` -- the desktop's, and the one over the way in -- and a width
//! asked two ways is two bars that disagree about where a thumb lands.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_draw_painting::{CUT, Lines, Run, at_most, text};

use crate::showing::{Bar, Face, Fitting, Layout, Measured, MeasuredRun, Slot, Span};

pub fn sized(layout: &Layout, fitting: Fitting) -> Result<Bar, Never> {
  let Ok(left) = every(&layout.left, fitting);
  let Ok(middle) = every(&layout.middle, fitting);
  let Ok(right) = every(&layout.right, fitting);

  Ok(Bar {
    left,
    middle,
    right,
    filling: layout.filling,
  })
}

pub fn every(slots: &[Slot], fitting: Fitting) -> Result<Vec<Measured>, Never> {
  let mut measured = Vec::new();

  for slot in slots {
    let mut runs = Vec::new();

    for span in &slot.spans {
      let Ok(one) = measure(span.text.as_str(), span.face, fitting);

      runs.push(one);
    }

    measured.push(Measured {
      slot: slot.clone(),
      runs,
    });
  }

  Ok(measured)
}

pub fn compact(slots: &mut [Measured], fitting: Fitting, width: u32) -> Result<(), Never> {
  for measured in slots {
    let mut remaining = width;
    let mut kept = 0_u64;

    'inner_for_2: for (span, size) in measured.slot.spans.iter_mut().zip(&mut measured.runs) {
      let Ok(room) = keep_span(span, size, fitting, remaining);

      match room {
        Fit::Yes => {}
        Fit::No => break 'inner_for_2,
      }

      kept = kept.saturating_add(1);

      match span.face {
        Face::SideClock => {}
        Face::Icon | Face::Clock | Face::Reading | Face::Small => {
          remaining = remaining.saturating_sub(size.width.saturating_add(fitting.between));
        }
      }
    }

    let Ok(runs) = index(kept);
    measured.slot.spans.truncate(runs);
    measured.runs.truncate(runs);
  }

  Ok(())
}

enum Fit {
  Yes,
  No,
}

fn keep_span(
  span: &mut Span,
  size: &mut MeasuredRun,
  fitting: Fitting,
  width: u32,
) -> Result<Fit, Never> {
  match (size.width <= width).then_some(()) {
    Some(()) => return Ok(Fit::Yes),
    None => {}
  }

  match span.face {
    Face::Small => return Ok(Fit::No),
    Face::Icon | Face::Clock | Face::SideClock | Face::Reading => {}
  }

  let Ok((text, fits)) = shortened(span, fitting, width);

  match (fits.width <= width).then_some(()) {
    Some(()) => {
      span.text = text;
      *size = fits;

      Ok(Fit::Yes)
    }
    None => Ok(Fit::No),
  }
}

fn shortened(span: &Span, fitting: Fitting, width: u32) -> Result<(String, MeasuredRun), Never> {
  let Ok(font) = fitting.font(span.face);
  let Ok(weight) = span.face.weight();
  let Ok(text) = at_most(
    Run {
      said: &span.text,
      weight,
      width,
    },
    &font,
    Lines(1),
  );
  let Ok(size) = measure(&text, span.face, fitting);

  match (size.width <= width).then_some(()) {
    Some(()) => Ok((text, size)),
    None => {
      let ellipsis = CUT.to_string();
      let Ok(ellipsis_size) = measure(&ellipsis, span.face, fitting);

      Ok((ellipsis, ellipsis_size))
    }
  }
}

fn measure(said: &str, face: Face, fitting: Fitting) -> Result<MeasuredRun, Never> {
  let Ok(font) = fitting.font(face);
  let Ok(weight) = face.weight();
  let Ok(layout) = text::layout(said, &font, weight, u32::MAX);
  let Ok(size) = layout.measured();
  let ink = match layout.vertical_ink_bounds() {
    Ok(bounds) => bounds,
    Err(fault) => {
      console_core_logging::warn!("console-bar: measuring text ink: {fault}");
      None
    }
  };
  let Ok(line_height) = fitted::<u32, i32>(size.height);

  Ok(MeasuredRun {
    width: size.width,
    twice_ink_center: ink.map_or(line_height, |(top, bottom)| top.saturating_add(bottom)),
  })
}

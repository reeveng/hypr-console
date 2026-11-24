pub mod hairline;
pub mod hairline_aa;
pub mod path;
pub mod path_aa;

use crate::{FloatRectangle, IntRect};

use crate::blitter::Blitter;
use crate::geom::{IntRectExt, ScreenIntRect};

pub fn fill_rect(
  rect: &FloatRectangle,
  clip: &ScreenIntRect,
  blitter: &mut dyn Blitter,
) -> core::result::Result<(), console_core_never::Never> {
  let Ok(round) = rect.round();

  match round {
    Some(rect_2) => {
      let Ok(()) = fill_int_rect(&rect_2, clip, blitter);
    }
    None => {}
  }

  Ok(())
}

fn fill_int_rect(
  rect: &IntRect,
  clip: &ScreenIntRect,
  blitter: &mut dyn Blitter,
) -> core::result::Result<(), console_core_never::Never> {
  let Ok(int_rect) = clip.to_int_rect();
  let Ok(intersect) = rect.intersect(&int_rect);

  let rect_2 = match intersect {
    Some(v) => v,
    None => return Ok(()),
  };

  let rect_3 = match rect_2.to_screen_int_rect() {
    Some(v) => v,
    None => return Ok(()),
  };

  let Ok(()) = blitter.blit_rect(&rect_3);

  Ok(())
}

pub fn fill_rect_aa(
  rect: &FloatRectangle,
  clip: &ScreenIntRect,
  blitter: &mut dyn Blitter,
) -> core::result::Result<(), console_core_never::Never> {
  let Ok(()) = hairline_aa::fill_rect(rect, clip, blitter);

  Ok(())
}

pub fn fill_path(
  path: &crate::Path,
  rule: crate::FillRule,
  coverage: crate::CoverageMode,
  clip: &crate::geom::ScreenIntRect,
  blitter: &mut dyn crate::blitter::Blitter,
) -> Result<(), console_core_never::Never> {
  match coverage {
    crate::CoverageMode::Antialiased => path_aa::fill_path(path, rule, clip, blitter),
    crate::CoverageMode::Binary => path::fill_path(path, rule, clip, blitter),
  }
}

//! A content stream's own numbers turned into kurbo's: a transform, a line cap and a line join.
//!
//! A cap or a join the specification does not name is drawn as its first one, butt and miter,
//! which is what a reader does with a style it was never told about.

use console_core_never::Never;
use console_core_portable_document_syntax::content::ops::{LineCap, LineJoin, Transform};

pub(crate) fn convert_transform(transform: Transform) -> Result<kurbo::Affine, Never> {
  let Ok(xx) = transform.0.as_f64();
  let Ok(yx) = transform.1.as_f64();
  let Ok(xy) = transform.2.as_f64();
  let Ok(yy) = transform.3.as_f64();
  let Ok(dx) = transform.4.as_f64();
  let Ok(dy) = transform.5.as_f64();

  Ok(kurbo::Affine::new([xx, yx, xy, yy, dx, dy]))
}

pub(crate) fn convert_line_cap(cap: LineCap) -> Result<kurbo::Cap, Never> {
  let Ok(style) = cap.0.as_i64();

  Ok(match style {
    1 => kurbo::Cap::Round,
    2 => kurbo::Cap::Square,
    _ => kurbo::Cap::Butt,
  })
}

pub(crate) fn convert_line_join(join: LineJoin) -> Result<kurbo::Join, Never> {
  let Ok(style) = join.0.as_i64();

  Ok(match style {
    1 => kurbo::Join::Round,
    2 => kurbo::Join::Bevel,
    _ => kurbo::Join::Miter,
  })
}
